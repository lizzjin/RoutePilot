use serde_json::{Value, json};

use crate::metrics::{MessageMetricsSnapshot, MetricsSnapshot};

use super::{AppState, effective_config, now_millis_string, provider_protocol_value};

pub(super) fn logs_body(state: &AppState) -> Value {
    let mut logs = state.control.usage_rows();
    if logs.is_empty() {
        logs = fallback_log_rows(state);
    }
    json!({
        "logs": logs,
        "total": logs.len(),
    })
}

pub(super) fn latency_body(state: &AppState) -> Value {
    latency_body_from_snapshot(&state.metrics.snapshot())
}

fn latency_body_from_snapshot(snapshot: &MetricsSnapshot) -> Value {
    let total_requests = snapshot
        .messages
        .iter()
        .map(|message| message.requests_total)
        .sum::<u64>();
    let total_duration = snapshot
        .messages
        .iter()
        .map(|message| message.duration_ms_total)
        .sum::<u64>();
    let avg = average(total_duration, total_requests);

    json!({
        "p50": avg,
        "p90": avg,
        "p95": avg,
        "p99": avg,
        "avg": avg,
        "max": avg,
        "byModel": {},
        "byProvider": {},
    })
}

fn fallback_log_rows(state: &AppState) -> Vec<Value> {
    let config = effective_config(state);
    let snapshot = state.metrics.snapshot();
    snapshot
        .messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let protocol = config
                .providers
                .get(&message.provider)
                .map(|provider| provider_protocol_value(provider.protocol))
                .unwrap_or("openai-compat");
            fallback_log_row(message, index, protocol, now_millis_string())
        })
        .collect()
}

fn fallback_log_row(
    message: &MessageMetricsSnapshot,
    index: usize,
    protocol: &str,
    timestamp: String,
) -> Value {
    let requests = message.requests_total.max(1);
    json!({
        "id": format!("log_{}_{}_{}", message.provider, message.model.replace('/', "_"), if message.stream { "stream" } else { "nonstream" }),
        "timestamp": timestamp,
        "userId": "usr_local_admin",
        "username": "local-admin",
        "apiKeyId": null,
        "apiKeyName": "MODELPORT_AUTH_TOKEN",
        "apiKeyGroup": "legacy",
        "tokenName": "MODELPORT_AUTH_TOKEN",
        "group": "legacy",
        "channelId": message.provider,
        "channelName": message.provider,
        "model": message.model,
        "resolvedModel": message.model,
        "provider": message.provider,
        "protocol": protocol,
        "requestType": if message.failures_total > 0 { "error" } else { "consume" },
        "stream": if message.stream { "stream" } else { "non-stream" },
        "status": if message.failures_total > 0 { "error" } else { "success" },
        "statusCode": if message.failures_total > 0 { 502 } else { 200 },
        "inputTokens": 0,
        "outputTokens": 0,
        "cacheWriteTokens": 0,
        "cacheReadTokens": 0,
        "billedInputTokens": 0,
        "totalTokens": 0,
        "cacheHitRate": 0.0,
        "costEstimate": 0.0,
        "costBreakdown": {
            "inputCost": 0.0,
            "outputCost": 0.0,
            "cacheWriteCost": 0.0,
            "cacheReadCost": 0.0,
            "totalCost": 0.0,
        },
        "latencyMs": average(message.duration_ms_total, requests),
        "firstByteLatencyMs": average(message.duration_ms_total, requests),
        "retryCount": 0,
        "clientIp": null,
        "requestPath": "/v1/messages",
        "billingMode": "metrics-fallback",
        "detail": format!("进程内指标回退日志 · provider={} · model={}", message.provider, message.model),
        "errorMessage": if message.failures_total > 0 { Some(format!("{} failure(s) recorded", message.failures_total)) } else { None },
        "sortIndex": index,
    })
}

fn average(total: u64, count: u64) -> u64 {
    total.checked_div(count).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_body_uses_average_duration_across_messages() {
        let snapshot = MetricsSnapshot {
            uptime_seconds: 1,
            routes: vec![],
            messages: vec![
                test_message("deepseek", "deepseek-v4-flash", 2, 80, false, 0),
                test_message("mimo", "mimo-v2.5-pro", 2, 120, false, 0),
            ],
        };

        let body = latency_body_from_snapshot(&snapshot);

        assert_eq!(body["avg"], 50);
        assert_eq!(body["p95"], 50);
        assert_eq!(body["byModel"], json!({}));
    }

    #[test]
    fn fallback_log_row_marks_failures_and_sanitizes_model_for_id() {
        let row = fallback_log_row(
            &test_message("openai", "foo/bar", 3, 90, true, 2),
            7,
            "openai-compat",
            "12345".to_owned(),
        );

        assert_eq!(row["id"], "log_openai_foo_bar_stream");
        assert_eq!(row["timestamp"], "12345");
        assert_eq!(row["status"], "error");
        assert_eq!(row["statusCode"], 502);
        assert_eq!(row["latencyMs"], 30);
        assert_eq!(row["errorMessage"], "2 failure(s) recorded");
        assert_eq!(row["sortIndex"], 7);
    }

    fn test_message(
        provider: &str,
        model: &str,
        requests_total: u64,
        duration_ms_total: u64,
        stream: bool,
        failures_total: u64,
    ) -> MessageMetricsSnapshot {
        MessageMetricsSnapshot {
            provider: provider.to_owned(),
            model: model.to_owned(),
            stream,
            requests_total,
            successes_total: requests_total.saturating_sub(failures_total),
            failures_total,
            duration_ms_total,
            input_tokens_total: 0,
            output_tokens_total: 0,
            cache_write_tokens_total: 0,
            cache_read_tokens_total: 0,
            cost_estimate_usd_total: 0.0,
        }
    }
}
