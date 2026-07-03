use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    control::ProviderUsageStats,
    error::AppError,
    metrics::{MessageMetricsSnapshot, MetricsSnapshot},
};

use super::{AppState, effective_config, now_millis, now_millis_string, provider_rows};

const HOUR_MS: u64 = 60 * 60 * 1_000;
const DAY_MS: u64 = 24 * HOUR_MS;
const MAX_DASHBOARD_TREND_MS: u64 = 90 * DAY_MS;

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct DashboardQuery {
    range: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Clone)]
struct DashboardTrendWindow {
    range: String,
    start_ms: u64,
    end_ms: u64,
    bucket_ms: u64,
}

pub(super) fn dashboard_body(state: &AppState, query: &DashboardQuery) -> Result<Value, AppError> {
    let trend_window = dashboard_trend_window(query)?;
    let snapshot = state.metrics.snapshot();
    let total_requests = snapshot
        .messages
        .iter()
        .map(|message| message.requests_total)
        .sum::<u64>();
    let total_successes = snapshot
        .messages
        .iter()
        .map(|message| message.successes_total)
        .sum::<u64>();
    let total_failures = snapshot
        .messages
        .iter()
        .map(|message| message.failures_total)
        .sum::<u64>();
    let total_duration = snapshot
        .messages
        .iter()
        .map(|message| message.duration_ms_total)
        .sum::<u64>();
    let route_requests = snapshot
        .routes
        .iter()
        .map(|route| route.requests_total)
        .sum::<u64>();
    let route_successes = snapshot
        .routes
        .iter()
        .map(|route| route.successes_total)
        .sum::<u64>();
    let route_failures = snapshot
        .routes
        .iter()
        .map(|route| route.failures_total)
        .sum::<u64>();
    let route_duration = snapshot
        .routes
        .iter()
        .map(|route| route.duration_ms_total)
        .sum::<u64>();
    let busiest_route = snapshot
        .routes
        .iter()
        .max_by_key(|route| route.requests_total)
        .map(|route| route.route.as_str())
        .unwrap_or("none");

    let providers = provider_rows(state);
    let active_providers = providers
        .iter()
        .filter(|provider| provider.get("status").and_then(Value::as_str) == Some("active"))
        .count();
    let active_users = state.auth.active_user_count();
    let usage_summary = state.control.usage_summary_today();
    let (usage_request_series, usage_error_series) = state.control.usage_time_series(
        trend_window.start_ms,
        trend_window.end_ms,
        trend_window.bucket_ms,
    );
    let usage_series_has_data = usage_request_series
        .iter()
        .any(|point| point.get("value").and_then(Value::as_u64).unwrap_or(0) > 0);
    let metric_top_models = snapshot
        .messages
        .iter()
        .map(|message| {
            json!({
                "model": message.model,
                "provider": message.provider,
                "requests": message.requests_total,
            })
        })
        .collect::<Vec<_>>();
    let usage_top_models = state.control.usage_top_models_today(8);
    let persisted_provider_usage = state.control.provider_usage_today();
    let recent_activity = state.control.activity_rows(8);
    let config = effective_config(state);

    Ok(json!({
        "uptimeSeconds": snapshot.uptime_seconds,
        "totalRequests": total_requests,
        "successRate": percent(total_successes, total_requests),
        "activeProviders": active_providers,
        "totalProviders": providers.len(),
        "activeUsers": active_users,
        "totalModels": config.model_list().len(),
        "avgLatencyMs": average(total_duration, total_requests),
        "apiKeysTotal": usage_summary.api_keys_total,
        "apiKeysActive": usage_summary.api_keys_active,
        "todayRequests": usage_summary.total_requests,
        "todayInputTokens": usage_summary.total_input_tokens,
        "todayOutputTokens": usage_summary.total_output_tokens,
        "todayCacheWriteTokens": usage_summary.total_cache_write_tokens,
        "todayCacheReadTokens": usage_summary.total_cache_read_tokens,
        "todayCostEstimate": usage_summary.total_cost_estimate,
        "trendRange": {
            "range": trend_window.range,
            "from": trend_window.start_ms.to_string(),
            "to": trend_window.end_ms.to_string(),
            "bucketMs": trend_window.bucket_ms,
        },
        "requestTimeSeries": if usage_series_has_data { usage_request_series } else { time_series(total_requests, &trend_window) },
        "errorTimeSeries": if usage_series_has_data { usage_error_series } else { time_series(total_failures, &trend_window) },
        "topModels": if metric_top_models.is_empty() { usage_top_models } else { metric_top_models },
        "providerHealth": provider_health_rows(&providers, &snapshot, &persisted_provider_usage),
        "recentActivity": if recent_activity.is_empty() {
            fallback_activity(total_requests, total_failures, route_requests, route_successes, route_failures, route_duration, busiest_route)
        } else {
            recent_activity
        },
    }))
}

fn fallback_activity(
    total_requests: u64,
    total_failures: u64,
    route_requests: u64,
    route_successes: u64,
    route_failures: u64,
    route_duration: u64,
    busiest_route: &str,
) -> Vec<Value> {
    let now = now_millis_string();
    vec![
        json!({
            "id": "act_health",
            "timestamp": now.clone(),
            "type": "request",
            "message": format!("ModelPort gateway is healthy; busiest route: {busiest_route}"),
            "severity": "info",
        }),
        json!({
            "id": "act_messages",
            "timestamp": now_millis_string(),
            "type": if total_failures > 0 { "error" } else { "request" },
            "message": format!("{total_requests} model message request(s), {total_failures} failure(s) since startup"),
            "severity": if total_failures > 0 { "warning" } else { "info" },
        }),
        json!({
            "id": "act_routes",
            "timestamp": now_millis_string(),
            "type": if route_failures > 0 { "error" } else { "request" },
            "message": format!("{route_requests} route request(s), {route_successes} success(es), avg {} ms", average(route_duration, route_requests)),
            "severity": if route_failures > 0 { "warning" } else { "info" },
        }),
    ]
}

fn provider_health_rows(
    providers: &[Value],
    snapshot: &MetricsSnapshot,
    persisted_provider_usage: &BTreeMap<String, ProviderUsageStats>,
) -> Vec<Value> {
    providers
        .iter()
        .map(|provider| {
            let id = provider.get("id").and_then(Value::as_str).unwrap_or("");
            let provider_messages = snapshot
                .messages
                .iter()
                .filter(|message| message.provider == id)
                .collect::<Vec<_>>();
            let usage = provider_dashboard_usage(&provider_messages, persisted_provider_usage.get(id));
            let success_rate = percent(usage.successes, usage.requests);
            let provider_status = provider
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("inactive");
            let runtime_status = provider
                .get("runtimeStatus")
                .and_then(Value::as_str)
                .unwrap_or("healthy");
            let provider_health = provider.get("health").unwrap_or(&Value::Null);
            let health_status = if provider_status != "active" {
                "down"
            } else if runtime_status == "cooldown" {
                "cooldown"
            } else if runtime_status == "degraded" || (usage.requests > 0 && success_rate < 99.0) {
                "degraded"
            } else {
                "healthy"
            };
            json!({
                "providerId": id,
                "displayName": provider.get("displayName").cloned().unwrap_or_else(|| json!(id)),
                "status": health_status,
                "requestsTotal": usage.requests,
                "successRate": success_rate,
                "avgLatencyMs": average(usage.duration_ms, usage.requests),
                "inputTokensTotal": usage.input_tokens,
                "outputTokensTotal": usage.output_tokens,
                "cacheWriteTokensTotal": usage.cache_write_tokens,
                "cacheReadTokensTotal": usage.cache_read_tokens,
                "costEstimateUsdTotal": usage.cost_estimate,
                "accountIssue": provider_health.get("accountIssue").cloned().unwrap_or_else(|| json!("none")),
                "rechargeRequired": provider_health.get("rechargeRequired").and_then(Value::as_bool).unwrap_or(false),
                "rechargeBadge": provider_health.get("rechargeBadge").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default)]
struct DashboardProviderUsage {
    requests: u64,
    successes: u64,
    duration_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
    cache_write_tokens: u64,
    cache_read_tokens: u64,
    cost_estimate: f64,
}

fn provider_dashboard_usage(
    metric_messages: &[&MessageMetricsSnapshot],
    persisted_usage: Option<&ProviderUsageStats>,
) -> DashboardProviderUsage {
    if let Some(stats) = persisted_usage
        && stats.requests_total > 0
    {
        return DashboardProviderUsage {
            requests: stats.requests_total,
            successes: stats.successes_total,
            duration_ms: stats.duration_ms_total,
            input_tokens: stats.input_tokens_total,
            output_tokens: stats.output_tokens_total,
            cache_write_tokens: stats.cache_write_tokens_total,
            cache_read_tokens: stats.cache_read_tokens_total,
            cost_estimate: stats.cost_estimate_usd_total,
        };
    }

    DashboardProviderUsage {
        requests: metric_messages
            .iter()
            .map(|message| message.requests_total)
            .sum(),
        successes: metric_messages
            .iter()
            .map(|message| message.successes_total)
            .sum(),
        duration_ms: metric_messages
            .iter()
            .map(|message| message.duration_ms_total)
            .sum(),
        input_tokens: metric_messages
            .iter()
            .map(|message| message.input_tokens_total)
            .sum(),
        output_tokens: metric_messages
            .iter()
            .map(|message| message.output_tokens_total)
            .sum(),
        cache_write_tokens: metric_messages
            .iter()
            .map(|message| message.cache_write_tokens_total)
            .sum(),
        cache_read_tokens: metric_messages
            .iter()
            .map(|message| message.cache_read_tokens_total)
            .sum(),
        cost_estimate: metric_messages
            .iter()
            .map(|message| message.cost_estimate_usd_total)
            .sum(),
    }
}

fn dashboard_trend_window(query: &DashboardQuery) -> Result<DashboardTrendWindow, AppError> {
    let now = now_millis();
    let range = query.range.as_deref().unwrap_or("1d");
    let (range, start_ms, end_ms) = match range {
        "custom" => {
            let start_ms = query
                .from
                .as_deref()
                .and_then(parse_dashboard_time)
                .ok_or_else(|| {
                    AppError::InvalidRequest("custom dashboard range requires from".to_owned())
                })?;
            let end_ms = query
                .to
                .as_deref()
                .and_then(parse_dashboard_time)
                .ok_or_else(|| {
                    AppError::InvalidRequest("custom dashboard range requires to".to_owned())
                })?;
            if start_ms >= end_ms {
                return Err(AppError::InvalidRequest(
                    "custom dashboard range requires from before to".to_owned(),
                ));
            }
            ("custom".to_owned(), start_ms, end_ms.min(now))
        }
        "3d" => ("3d".to_owned(), now.saturating_sub(3 * DAY_MS), now),
        "7d" => ("7d".to_owned(), now.saturating_sub(7 * DAY_MS), now),
        _ => ("1d".to_owned(), now.saturating_sub(DAY_MS), now),
    };
    let duration_ms = end_ms.saturating_sub(start_ms).max(HOUR_MS);
    if duration_ms > MAX_DASHBOARD_TREND_MS {
        return Err(AppError::InvalidRequest(
            "dashboard range cannot exceed 90 days".to_owned(),
        ));
    }

    Ok(DashboardTrendWindow {
        range,
        start_ms,
        end_ms,
        bucket_ms: dashboard_bucket_ms(duration_ms),
    })
}

fn parse_dashboard_time(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

fn dashboard_bucket_ms(duration_ms: u64) -> u64 {
    if duration_ms <= DAY_MS {
        HOUR_MS
    } else if duration_ms <= 3 * DAY_MS {
        3 * HOUR_MS
    } else if duration_ms <= 7 * DAY_MS {
        6 * HOUR_MS
    } else if duration_ms <= 31 * DAY_MS {
        DAY_MS
    } else {
        7 * DAY_MS
    }
}

fn time_series(value: u64, window: &DashboardTrendWindow) -> Vec<Value> {
    let bucket_count = bucket_count(window.start_ms, window.end_ms, window.bucket_ms);
    (0..bucket_count)
        .map(|offset| {
            let timestamp = window
                .start_ms
                .saturating_add(offset.saturating_mul(window.bucket_ms));
            json!({
                "timestamp": timestamp.to_string(),
                "value": if offset + 1 == bucket_count { value } else { 0 },
            })
        })
        .collect()
}

fn bucket_count(start_ms: u64, end_ms: u64, bucket_ms: u64) -> u64 {
    if bucket_ms == 0 || end_ms <= start_ms {
        return 1;
    }
    end_ms.saturating_sub(start_ms) / bucket_ms + 1
}

fn percent(successes: u64, total: u64) -> f64 {
    if total == 0 {
        100.0
    } else {
        (successes as f64 / total as f64) * 100.0
    }
}

fn average(total: u64, count: u64) -> u64 {
    total.checked_div(count).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricsSnapshot, RouteMetricsSnapshot};

    #[test]
    fn time_series_places_fallback_value_in_latest_bucket() {
        let window = DashboardTrendWindow {
            range: "custom".to_owned(),
            start_ms: 1_000,
            end_ms: 4_000,
            bucket_ms: 1_000,
        };

        let series = time_series(42, &window);

        assert_eq!(series.len(), 4);
        assert_eq!(series[0]["value"], 0);
        assert_eq!(series[3]["timestamp"], "4000");
        assert_eq!(series[3]["value"], 42);
    }

    #[test]
    fn provider_health_prefers_persisted_usage_and_keeps_recharge_badge() {
        let providers = vec![json!({
            "id": "deepseek",
            "displayName": "DeepSeek",
            "status": "active",
            "runtimeStatus": "healthy",
            "health": {
                "accountIssue": "insufficient_balance",
                "rechargeRequired": true,
                "rechargeBadge": "代充值",
            },
        })];
        let snapshot = MetricsSnapshot {
            uptime_seconds: 1,
            routes: vec![RouteMetricsSnapshot {
                route: "messages".to_owned(),
                requests_total: 1,
                successes_total: 1,
                failures_total: 0,
                duration_ms_total: 10,
            }],
            messages: vec![MessageMetricsSnapshot {
                provider: "deepseek".to_owned(),
                model: "deepseek-v4-flash".to_owned(),
                stream: false,
                requests_total: 1,
                successes_total: 1,
                failures_total: 0,
                duration_ms_total: 10,
                input_tokens_total: 1,
                output_tokens_total: 1,
                cache_write_tokens_total: 0,
                cache_read_tokens_total: 0,
                cost_estimate_usd_total: 0.01,
            }],
        };
        let mut persisted = BTreeMap::new();
        persisted.insert(
            "deepseek".to_owned(),
            ProviderUsageStats {
                requests_total: 4,
                successes_total: 3,
                duration_ms_total: 120,
                input_tokens_total: 11,
                output_tokens_total: 22,
                cache_write_tokens_total: 33,
                cache_read_tokens_total: 44,
                cost_estimate_usd_total: 0.5,
            },
        );

        let rows = provider_health_rows(&providers, &snapshot, &persisted);
        let row = &rows[0];

        assert_eq!(row["requestsTotal"], 4);
        assert_eq!(row["successRate"], 75.0);
        assert_eq!(row["avgLatencyMs"], 30);
        assert_eq!(row["status"], "degraded");
        assert_eq!(row["inputTokensTotal"], 11);
        assert_eq!(row["rechargeRequired"], true);
        assert_eq!(row["rechargeBadge"], "代充值");
    }
}
