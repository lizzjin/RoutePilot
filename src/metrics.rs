use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::control::UsageEstimate;

const MAX_MESSAGE_SERIES: usize = 512;
const TRAFFIC_CLASS_COUNT: usize = 3;
const OVERFLOW_PROVIDER_LABEL: &str = "__overflow__";
const OVERFLOW_MODEL_LABEL: &str = "__other__";
const OVERFLOW_TRAFFIC_LABEL: &str = "__overflow__";

#[derive(Debug)]
pub struct Metrics {
    started_at: Instant,
    inner: Mutex<MetricsInner>,
    max_message_series: usize,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub messages: Vec<MessageMetricsSnapshot>,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct MessageMetricsSnapshot {
    pub provider: String,
    pub model: String,
    pub traffic_class: String,
    pub stream: bool,
    pub requests_total: u64,
    pub successes_total: u64,
    pub failures_total: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct MessageMetricLabels<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub traffic_class: &'a str,
    pub stream: bool,
}

#[derive(Debug, Default)]
struct MetricsInner {
    routes: BTreeMap<String, CounterSet>,
    messages: BTreeMap<MessageKey, MessageCounterSet>,
    rejections: BTreeMap<RejectionKey, u64>,
}

#[derive(Debug, Default)]
struct CounterSet {
    requests_total: u64,
    successes_total: u64,
    failures_total: u64,
    duration_ms_total: u64,
}

#[derive(Debug, Default)]
struct MessageCounterSet {
    counters: CounterSet,
    usage: UsageCounterSet,
}

#[derive(Debug, Default)]
struct UsageCounterSet {
    input_tokens_total: u64,
    output_tokens_total: u64,
    cache_write_tokens_total: u64,
    cache_read_tokens_total: u64,
    cost_estimate_usd_total: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MessageKey {
    provider: String,
    model: String,
    traffic_class: String,
    stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RejectionKey {
    route: String,
    phase: String,
    reason: String,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            inner: Mutex::new(MetricsInner::default()),
            max_message_series: MAX_MESSAGE_SERIES,
        }
    }

    pub fn record_route(&self, route: &str, success: bool, duration: Duration) {
        let mut inner = self.inner.lock().expect("metrics lock poisoned");
        inner
            .routes
            .entry(route.to_owned())
            .or_default()
            .record(success, duration);
    }

    pub fn record_rejection(&self, route: &str, phase: &'static str, reason: &'static str) {
        debug_assert!(matches!(
            phase,
            "authentication"
                | "validation"
                | "identity"
                | "routing"
                | "rate_limit"
                | "admission"
                | "concurrency"
                | "ledger"
        ));
        let mut inner = self.inner.lock().expect("metrics lock poisoned");
        let count = inner
            .rejections
            .entry(RejectionKey {
                route: route.to_owned(),
                phase: phase.to_owned(),
                reason: reason.to_owned(),
            })
            .or_default();
        *count = count.saturating_add(1);
    }

    pub fn record_message(
        &self,
        labels: MessageMetricLabels<'_>,
        success: bool,
        duration: Duration,
        usage: UsageEstimate,
    ) {
        let mut inner = self.inner.lock().expect("metrics lock poisoned");
        let mut key = MessageKey {
            provider: labels.provider.to_owned(),
            model: labels.model.to_owned(),
            traffic_class: labels.traffic_class.to_owned(),
            stream: labels.stream,
        };
        let overflow_slots = self
            .max_message_series
            .saturating_sub(1)
            .min(TRAFFIC_CLASS_COUNT);
        let regular_series_limit = self.max_message_series.saturating_sub(overflow_slots);
        if !inner.messages.contains_key(&key) && inner.messages.len() >= regular_series_limit {
            key.provider = OVERFLOW_PROVIDER_LABEL.to_owned();
            key.model = OVERFLOW_MODEL_LABEL.to_owned();
            if overflow_slots < TRAFFIC_CLASS_COUNT {
                key.traffic_class = OVERFLOW_TRAFFIC_LABEL.to_owned();
            }
            key.stream = false;
        }
        inner
            .messages
            .entry(key)
            .or_default()
            .record(success, duration, usage);
    }

    pub fn render_prometheus(&self) -> String {
        let inner = self.inner.lock().expect("metrics lock poisoned");
        let mut output = String::new();

        output.push_str("# HELP modelport_uptime_seconds Seconds since ModelPort started.\n");
        output.push_str("# TYPE modelport_uptime_seconds gauge\n");
        output.push_str(&format!(
            "modelport_uptime_seconds {}\n\n",
            self.started_at.elapsed().as_secs()
        ));
        output.push_str("# HELP modelport_build_info Static release and source-build identity.\n");
        output.push_str("# TYPE modelport_build_info gauge\n");
        output.push_str(&format!(
            "modelport_build_info{{version=\"{}\",revision=\"{}\",source_state=\"{}\"}} 1\n\n",
            escape_label_value(crate::version::VERSION),
            escape_label_value(crate::version::REVISION),
            escape_label_value(crate::version::SOURCE_STATE),
        ));

        output.push_str(
            "# HELP modelport_route_requests_total Total route requests handled by ModelPort.\n",
        );
        output.push_str("# TYPE modelport_route_requests_total counter\n");
        output.push_str("# HELP modelport_route_successes_total Total successful route requests handled by ModelPort.\n");
        output.push_str("# TYPE modelport_route_successes_total counter\n");
        output.push_str("# HELP modelport_route_failures_total Total failed route requests handled by ModelPort.\n");
        output.push_str("# TYPE modelport_route_failures_total counter\n");
        output.push_str("# HELP modelport_route_duration_ms_total Total route handling duration in milliseconds.\n");
        output.push_str("# TYPE modelport_route_duration_ms_total counter\n");
        for (route, counters) in &inner.routes {
            let labels = format!("route=\"{}\"", escape_label_value(route));
            push_counter_set(&mut output, "modelport_route", &labels, counters);
        }
        output.push('\n');

        output.push_str(
            "# HELP modelport_inference_rejections_total Inference requests rejected before a Provider result, by bounded phase and reason.\n",
        );
        output.push_str("# TYPE modelport_inference_rejections_total counter\n");
        for (key, count) in &inner.rejections {
            let labels = format!(
                "route=\"{}\",phase=\"{}\",reason=\"{}\"",
                escape_label_value(&key.route),
                escape_label_value(&key.phase),
                escape_label_value(&key.reason),
            );
            output.push_str(&format!(
                "modelport_inference_rejections_total{{{labels}}} {count}\n"
            ));
        }
        output.push('\n');

        output.push_str("# HELP modelport_message_requests_total Total message requests by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_requests_total counter\n");
        output.push_str("# HELP modelport_message_successes_total Total successful message requests by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_successes_total counter\n");
        output.push_str("# HELP modelport_message_failures_total Total failed message requests by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_failures_total counter\n");
        output.push_str("# HELP modelport_message_duration_ms_total Total message request lifecycle duration in milliseconds by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_duration_ms_total counter\n");
        output.push_str("# HELP modelport_message_input_tokens_total Total input tokens by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_input_tokens_total counter\n");
        output.push_str("# HELP modelport_message_output_tokens_total Total output tokens by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_output_tokens_total counter\n");
        output.push_str("# HELP modelport_message_cache_write_tokens_total Total cache write tokens by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_cache_write_tokens_total counter\n");
        output.push_str("# HELP modelport_message_cache_read_tokens_total Total cache read tokens by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_cache_read_tokens_total counter\n");
        output.push_str("# HELP modelport_message_cost_estimate_usd_total Total estimated message cost in USD by provider/model/traffic class/stream.\n");
        output.push_str("# TYPE modelport_message_cost_estimate_usd_total counter\n");
        for (key, counters) in &inner.messages {
            let labels = format!(
                "provider=\"{}\",model=\"{}\",traffic_class=\"{}\",stream=\"{}\"",
                escape_label_value(&key.provider),
                escape_label_value(&key.model),
                escape_label_value(&key.traffic_class),
                key.stream
            );
            push_message_counter_set(&mut output, &labels, counters);
        }

        output
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    #[cfg(test)]
    pub fn snapshot(&self) -> MetricsSnapshot {
        let inner = self.inner.lock().expect("metrics lock poisoned");

        MetricsSnapshot {
            messages: inner
                .messages
                .iter()
                .map(|(key, message)| MessageMetricsSnapshot {
                    provider: key.provider.clone(),
                    model: key.model.clone(),
                    traffic_class: key.traffic_class.clone(),
                    stream: key.stream,
                    requests_total: message.counters.requests_total,
                    successes_total: message.counters.successes_total,
                    failures_total: message.counters.failures_total,
                })
                .collect(),
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl CounterSet {
    fn record(&mut self, success: bool, duration: Duration) {
        self.requests_total = self.requests_total.saturating_add(1);
        if success {
            self.successes_total = self.successes_total.saturating_add(1);
        } else {
            self.failures_total = self.failures_total.saturating_add(1);
        }
        self.duration_ms_total = self.duration_ms_total.saturating_add(duration_ms(duration));
    }
}

impl MessageCounterSet {
    fn record(&mut self, success: bool, duration: Duration, usage: UsageEstimate) {
        self.counters.record(success, duration);
        self.usage.record(usage);
    }
}

impl UsageCounterSet {
    fn record(&mut self, usage: UsageEstimate) {
        self.input_tokens_total = self.input_tokens_total.saturating_add(usage.input_tokens);
        self.output_tokens_total = self.output_tokens_total.saturating_add(usage.output_tokens);
        self.cache_write_tokens_total = self
            .cache_write_tokens_total
            .saturating_add(usage.cache_write_tokens);
        self.cache_read_tokens_total = self
            .cache_read_tokens_total
            .saturating_add(usage.cache_read_tokens);
        self.cost_estimate_usd_total += usage.cost_estimate.max(0.0);
    }
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn push_counter_set(output: &mut String, prefix: &str, labels: &str, counters: &CounterSet) {
    output.push_str(&format!(
        "{prefix}_requests_total{{{labels}}} {}\n",
        counters.requests_total
    ));
    output.push_str(&format!(
        "{prefix}_successes_total{{{labels}}} {}\n",
        counters.successes_total
    ));
    output.push_str(&format!(
        "{prefix}_failures_total{{{labels}}} {}\n",
        counters.failures_total
    ));
    output.push_str(&format!(
        "{prefix}_duration_ms_total{{{labels}}} {}\n",
        counters.duration_ms_total
    ));
}

fn push_message_counter_set(output: &mut String, labels: &str, message: &MessageCounterSet) {
    push_counter_set(output, "modelport_message", labels, &message.counters);
    output.push_str(&format!(
        "modelport_message_input_tokens_total{{{labels}}} {}\n",
        message.usage.input_tokens_total
    ));
    output.push_str(&format!(
        "modelport_message_output_tokens_total{{{labels}}} {}\n",
        message.usage.output_tokens_total
    ));
    output.push_str(&format!(
        "modelport_message_cache_write_tokens_total{{{labels}}} {}\n",
        message.usage.cache_write_tokens_total
    ));
    output.push_str(&format!(
        "modelport_message_cache_read_tokens_total{{{labels}}} {}\n",
        message.usage.cache_read_tokens_total
    ));
    output.push_str(&format!(
        "modelport_message_cost_estimate_usd_total{{{labels}}} {}\n",
        message.usage.cost_estimate_usd_total
    ));
}

fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_prometheus_metrics() {
        let metrics = Metrics::new();
        metrics.record_route("messages", true, Duration::from_millis(12));
        metrics.record_rejection("messages", "validation", "invalid_request");
        metrics.record_message(
            MessageMetricLabels {
                provider: "mimo",
                model: "mimo-v2.5-pro",
                traffic_class: "business",
                stream: false,
            },
            true,
            Duration::from_millis(12),
            UsageEstimate {
                input_tokens: 3,
                output_tokens: 4,
                cache_write_tokens: 5,
                cache_read_tokens: 6,
                cost_estimate: 0.000123,
            },
        );

        let rendered = metrics.render_prometheus();

        assert!(rendered.contains("modelport_uptime_seconds"));
        assert!(rendered.contains(&format!(
            r#"modelport_build_info{{version="{}""#,
            crate::version::VERSION
        )));
        assert!(rendered.contains(r#"modelport_route_requests_total{route="messages"} 1"#));
        assert!(rendered.contains(
            r#"modelport_message_requests_total{provider="mimo",model="mimo-v2.5-pro",traffic_class="business",stream="false"} 1"#
        ));
        assert!(rendered.contains(
            r#"modelport_message_input_tokens_total{provider="mimo",model="mimo-v2.5-pro",traffic_class="business",stream="false"} 3"#
        ));
        assert!(rendered.contains(
            r#"modelport_message_output_tokens_total{provider="mimo",model="mimo-v2.5-pro",traffic_class="business",stream="false"} 4"#
        ));
        assert!(rendered.contains(
            r#"modelport_message_cache_write_tokens_total{provider="mimo",model="mimo-v2.5-pro",traffic_class="business",stream="false"} 5"#
        ));
        assert!(rendered.contains(
            r#"modelport_message_cache_read_tokens_total{provider="mimo",model="mimo-v2.5-pro",traffic_class="business",stream="false"} 6"#
        ));
        assert!(rendered.contains(
            r#"modelport_message_cost_estimate_usd_total{provider="mimo",model="mimo-v2.5-pro",traffic_class="business",stream="false"} 0.000123"#
        ));
        assert!(rendered.contains(
            r#"modelport_inference_rejections_total{route="messages",phase="validation",reason="invalid_request"} 1"#
        ));
    }

    #[test]
    fn escapes_label_values() {
        assert_eq!(escape_label_value("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }

    #[test]
    fn bounds_user_controlled_model_series() {
        let metrics = Metrics {
            started_at: Instant::now(),
            inner: Mutex::new(MetricsInner::default()),
            max_message_series: 2,
        };
        for model in ["model-a", "model-b", "model-c", "model-d"] {
            metrics.record_message(
                MessageMetricLabels {
                    provider: "custom",
                    model,
                    traffic_class: "business",
                    stream: false,
                },
                true,
                Duration::from_millis(1),
                UsageEstimate::default(),
            );
        }

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.messages.len(), 2);
        assert!(
            snapshot
                .messages
                .iter()
                .any(|message| message.model == OVERFLOW_MODEL_LABEL && message.requests_total == 3)
        );
    }

    #[test]
    fn overflow_series_do_not_merge_business_and_synthetic_traffic() {
        let metrics = Metrics {
            started_at: Instant::now(),
            inner: Mutex::new(MetricsInner::default()),
            max_message_series: 5,
        };
        for (model, traffic_class) in [
            ("model-a", "business"),
            ("model-b", "business"),
            ("model-c", "business"),
            ("model-d", "synthetic"),
        ] {
            metrics.record_message(
                MessageMetricLabels {
                    provider: "custom",
                    model,
                    traffic_class,
                    stream: false,
                },
                true,
                Duration::from_millis(1),
                UsageEstimate::default(),
            );
        }

        let snapshot = metrics.snapshot();
        assert!(snapshot.messages.len() <= 5);
        assert!(snapshot.messages.iter().any(|message| {
            message.model == OVERFLOW_MODEL_LABEL && message.traffic_class == "business"
        }));
        assert!(snapshot.messages.iter().any(|message| {
            message.model == OVERFLOW_MODEL_LABEL && message.traffic_class == "synthetic"
        }));
    }
}
