use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    control::{ProviderUsageStats, UsageSummary},
    domain::TenantScope,
    enterprise_ledger::DashboardLedgerSnapshot,
    error::AppError,
};

use super::{AppState, effective_config, now_millis, provider_rows};

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

#[derive(Debug, Default)]
struct ModelRangeUsage {
    model: String,
    provider: String,
    requests: u64,
    tokens: u64,
    cost: f64,
}

#[derive(Debug)]
struct DashboardRangeUsage {
    matched_requests: u64,
    request_time_series: Vec<Value>,
    error_time_series: Vec<Value>,
    token_time_series: Vec<Value>,
    model_usage: Vec<Value>,
    summary: Value,
}

pub(super) async fn dashboard_body(
    state: &AppState,
    query: &DashboardQuery,
) -> Result<Value, AppError> {
    let trend_window = dashboard_trend_window(query)?;
    let uptime_seconds = state.metrics.uptime_seconds();
    let providers = provider_rows(state);
    let active_providers = providers
        .iter()
        .filter(|provider| provider.get("status").and_then(Value::as_str) == Some("active"))
        .count();
    let active_users = state.auth.active_user_count();
    let today_start = (now_millis() / DAY_MS) * DAY_MS;
    let relational_snapshot = state
        .ledger
        .dashboard_snapshot(
            trend_window.start_ms,
            trend_window.end_ms,
            trend_window.bucket_ms,
            today_start,
            state.control.api_key_counts(),
        )
        .await?;
    let (usage_summary, range_usage, persisted_provider_usage) =
        if let Some(DashboardLedgerSnapshot {
            usage_summary,
            provider_usage,
            matched_requests,
            request_time_series,
            error_time_series,
            token_time_series,
            model_usage,
            summary,
        }) = relational_snapshot
        {
            (
                usage_summary,
                DashboardRangeUsage {
                    matched_requests,
                    request_time_series,
                    error_time_series,
                    token_time_series,
                    model_usage,
                    summary,
                },
                provider_usage,
            )
        } else {
            let usage_rows = state
                .ledger
                .usage_rows_since(Some(trend_window.start_ms.min(today_start)))
                .await?;
            (
                dashboard_today_summary(&usage_rows, state.control.api_key_counts()),
                dashboard_range_usage(&usage_rows, &trend_window),
                provider_usage_today(&usage_rows),
            )
        };
    let total_requests = usage_summary.total_requests;
    let total_successes = usage_summary.total_successes;
    let mut persisted_top_models = range_usage
        .model_usage
        .iter()
        .map(|row| {
            json!({
                "model": row.get("model").cloned().unwrap_or(Value::Null),
                "provider": row.get("provider").cloned().unwrap_or(Value::Null),
                "requests": row.get("requests").cloned().unwrap_or_else(|| json!(0)),
            })
        })
        .collect::<Vec<_>>();
    sort_and_limit_top_models(&mut persisted_top_models);
    let (recent_activity, _) = state.ledger.audit_events(8).await?;
    let (has_request_ever, has_successful_request_ever) =
        state.ledger.onboarding_milestones().await?;
    let config = effective_config(state);

    Ok(json!({
        "uptimeSeconds": uptime_seconds,
        "totalRequests": total_requests,
        "successRate": percent(total_successes, total_requests),
        "activeProviders": active_providers,
        "totalProviders": providers.len(),
        "activeUsers": active_users,
        "totalModels": config.model_list().len(),
        "avgLatencyMs": usage_summary.average_latency_ms,
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
        "requestTimeSeries": range_usage.request_time_series,
        "errorTimeSeries": range_usage.error_time_series,
        "topModels": persisted_top_models,
        "modelUsage": range_usage.model_usage,
        "tokenTimeSeries": range_usage.token_time_series,
        "rangeSummary": range_usage.summary,
        "rangeDataSource": if range_usage.matched_requests > 0 { "relational-ledger" } else { "empty" },
        "rangeDataEstimated": false,
        "rangeDataAtRetentionLimit": false,
        "providerHealth": provider_health_rows(&providers, &persisted_provider_usage),
        "recentActivity": recent_activity,
        "onboardingMilestones": {
            "hasRequestEver": has_request_ever,
            "hasSuccessfulRequestEver": has_successful_request_ever,
            "hasDefaultProjectPolicy": state
                .governance
                .has_policy(&TenantScope::legacy_local()),
        },
    }))
}

fn sort_and_limit_top_models(rows: &mut Vec<Value>) {
    rows.sort_by(|left, right| {
        right
            .get("requests")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .cmp(&left.get("requests").and_then(Value::as_u64).unwrap_or(0))
    });
    rows.truncate(8);
}

fn dashboard_today_summary(rows: &[Value], api_keys: (u64, u64)) -> UsageSummary {
    let today_start = (now_millis() / DAY_MS) * DAY_MS;
    let mut summary = UsageSummary {
        api_keys_total: api_keys.0,
        api_keys_active: api_keys.1,
        ..UsageSummary::default()
    };
    let mut total_latency = 0u64;
    for row in rows.iter().filter(|row| {
        row.get("trafficClass").and_then(Value::as_str) == Some("business")
            && dashboard_usage_timestamp(row).is_some_and(|timestamp| timestamp >= today_start)
    }) {
        summary.total_requests = summary.total_requests.saturating_add(1);
        if row.get("status").and_then(Value::as_str) == Some("success") {
            summary.total_successes = summary.total_successes.saturating_add(1);
        }
        summary.total_input_tokens = summary
            .total_input_tokens
            .saturating_add(dashboard_usage_u64(row, "inputTokens"));
        summary.total_output_tokens = summary
            .total_output_tokens
            .saturating_add(dashboard_usage_u64(row, "outputTokens"));
        summary.total_cache_write_tokens = summary
            .total_cache_write_tokens
            .saturating_add(dashboard_usage_u64(row, "cacheWriteTokens"));
        summary.total_cache_read_tokens = summary
            .total_cache_read_tokens
            .saturating_add(dashboard_usage_u64(row, "cacheReadTokens"));
        summary.total_cost_estimate += row
            .get("costEstimate")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        total_latency = total_latency.saturating_add(dashboard_usage_u64(row, "latencyMs"));
    }
    summary.average_latency_ms = total_latency
        .checked_div(summary.total_requests)
        .unwrap_or(0);
    summary
}

fn provider_usage_today(rows: &[Value]) -> BTreeMap<String, ProviderUsageStats> {
    let today_start = (now_millis() / DAY_MS) * DAY_MS;
    let mut providers = BTreeMap::new();
    for row in rows.iter().filter(|row| {
        row.get("trafficClass").and_then(Value::as_str) == Some("business")
            && dashboard_usage_timestamp(row).is_some_and(|timestamp| timestamp >= today_start)
    }) {
        let provider = row
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("unrouted");
        let stats = providers
            .entry(provider.to_owned())
            .or_insert_with(ProviderUsageStats::default);
        stats.requests_total = stats.requests_total.saturating_add(1);
        if row.get("status").and_then(Value::as_str) == Some("success") {
            stats.successes_total = stats.successes_total.saturating_add(1);
        }
        stats.duration_ms_total = stats
            .duration_ms_total
            .saturating_add(dashboard_usage_u64(row, "latencyMs"));
        stats.input_tokens_total = stats
            .input_tokens_total
            .saturating_add(dashboard_usage_u64(row, "inputTokens"));
        stats.output_tokens_total = stats
            .output_tokens_total
            .saturating_add(dashboard_usage_u64(row, "outputTokens"));
        stats.cache_write_tokens_total = stats
            .cache_write_tokens_total
            .saturating_add(dashboard_usage_u64(row, "cacheWriteTokens"));
        stats.cache_read_tokens_total = stats
            .cache_read_tokens_total
            .saturating_add(dashboard_usage_u64(row, "cacheReadTokens"));
        stats.cost_estimate_usd_total += row
            .get("costEstimate")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
    }
    providers
}

fn provider_health_rows(
    providers: &[Value],
    persisted_provider_usage: &BTreeMap<String, ProviderUsageStats>,
) -> Vec<Value> {
    providers
        .iter()
        .map(|provider| {
            let id = provider.get("id").and_then(Value::as_str).unwrap_or("");
            let usage = persisted_provider_usage.get(id).cloned().unwrap_or_default();
            let success_rate = percent(usage.successes_total, usage.requests_total);
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
            } else if runtime_status == "degraded"
                || (usage.requests_total > 0 && success_rate < 99.0)
            {
                "degraded"
            } else {
                "healthy"
            };
            json!({
                "providerId": id,
                "displayName": provider.get("displayName").cloned().unwrap_or_else(|| json!(id)),
                "status": health_status,
                "requestsTotal": usage.requests_total,
                "successRate": success_rate,
                "avgLatencyMs": average(usage.duration_ms_total, usage.requests_total),
                "inputTokensTotal": usage.input_tokens_total,
                "outputTokensTotal": usage.output_tokens_total,
                "cacheWriteTokensTotal": usage.cache_write_tokens_total,
                "cacheReadTokensTotal": usage.cache_read_tokens_total,
                "costEstimateUsdTotal": usage.cost_estimate_usd_total,
                "accountIssue": provider_health.get("accountIssue").cloned().unwrap_or_else(|| json!("none")),
                "rechargeRequired": provider_health.get("rechargeRequired").and_then(Value::as_bool).unwrap_or(false),
                "rechargeBadge": provider_health.get("rechargeBadge").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn dashboard_range_usage(rows: &[Value], window: &DashboardTrendWindow) -> DashboardRangeUsage {
    let bucket_count = usize::try_from(bucket_count(
        window.start_ms,
        window.end_ms,
        window.bucket_ms,
    ))
    .unwrap_or(1)
    .max(1);
    let mut requests = vec![0u64; bucket_count];
    let mut errors = vec![0u64; bucket_count];
    let mut input_tokens = vec![0u64; bucket_count];
    let mut output_tokens = vec![0u64; bucket_count];
    let mut cache_write_tokens = vec![0u64; bucket_count];
    let mut cache_read_tokens = vec![0u64; bucket_count];
    let mut models = BTreeMap::<(String, String), ModelRangeUsage>::new();
    let mut matched_requests = 0u64;
    let mut success_requests = 0u64;
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;
    let mut total_cache_write_tokens = 0u64;
    let mut total_cache_read_tokens = 0u64;
    let mut total_cost_estimate = 0.0f64;

    for row in rows {
        if row
            .get("trafficClass")
            .and_then(Value::as_str)
            .is_some_and(|traffic_class| traffic_class != "business")
        {
            continue;
        }
        let Some(timestamp) = dashboard_usage_timestamp(row) else {
            continue;
        };
        if timestamp < window.start_ms || timestamp > window.end_ms {
            continue;
        }

        let index =
            usize::try_from(timestamp.saturating_sub(window.start_ms) / window.bucket_ms.max(1))
                .unwrap_or(bucket_count.saturating_sub(1))
                .min(bucket_count.saturating_sub(1));
        let row_input = dashboard_usage_u64(row, "inputTokens");
        let row_output = dashboard_usage_u64(row, "outputTokens");
        let row_cache_write = dashboard_usage_u64(row, "cacheWriteTokens");
        let row_cache_read = dashboard_usage_u64(row, "cacheReadTokens");
        let row_tokens = row_input
            .saturating_add(row_output)
            .saturating_add(row_cache_write)
            .saturating_add(row_cache_read);
        let row_cost = row
            .get("costEstimate")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);

        matched_requests = matched_requests.saturating_add(1);
        requests[index] = requests[index].saturating_add(1);
        if row.get("status").and_then(Value::as_str) == Some("success") {
            success_requests = success_requests.saturating_add(1);
        } else {
            errors[index] = errors[index].saturating_add(1);
        }
        input_tokens[index] = input_tokens[index].saturating_add(row_input);
        output_tokens[index] = output_tokens[index].saturating_add(row_output);
        cache_write_tokens[index] = cache_write_tokens[index].saturating_add(row_cache_write);
        cache_read_tokens[index] = cache_read_tokens[index].saturating_add(row_cache_read);
        total_input_tokens = total_input_tokens.saturating_add(row_input);
        total_output_tokens = total_output_tokens.saturating_add(row_output);
        total_cache_write_tokens = total_cache_write_tokens.saturating_add(row_cache_write);
        total_cache_read_tokens = total_cache_read_tokens.saturating_add(row_cache_read);
        total_cost_estimate += row_cost;

        let model = row
            .get("resolvedModel")
            .or_else(|| row.get("model"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let provider = row
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let model_usage = models
            .entry((model.clone(), provider.clone()))
            .or_insert_with(|| ModelRangeUsage {
                model,
                provider,
                ..ModelRangeUsage::default()
            });
        model_usage.requests = model_usage.requests.saturating_add(1);
        model_usage.tokens = model_usage.tokens.saturating_add(row_tokens);
        model_usage.cost += row_cost;
    }

    let request_time_series = dashboard_value_series(&requests, window);
    let error_time_series = dashboard_value_series(&errors, window);
    let token_time_series = (0..bucket_count)
        .map(|index| {
            let billed_input = input_tokens[index]
                .saturating_add(cache_write_tokens[index])
                .saturating_add(cache_read_tokens[index]);
            json!({
                "timestamp": dashboard_bucket_timestamp(window, index),
                "inputTokens": input_tokens[index],
                "outputTokens": output_tokens[index],
                "cacheWriteTokens": cache_write_tokens[index],
                "cacheReadTokens": cache_read_tokens[index],
                "cacheHitRate": if billed_input == 0 { 0.0 } else { (cache_read_tokens[index] as f64 / billed_input as f64) * 100.0 },
            })
        })
        .collect();
    let mut model_usage = models.into_values().collect::<Vec<_>>();
    model_usage.sort_by(|left, right| {
        right
            .tokens
            .cmp(&left.tokens)
            .then_with(|| right.requests.cmp(&left.requests))
            .then_with(|| left.model.cmp(&right.model))
    });
    let model_usage = model_usage
        .into_iter()
        .map(|row| {
            json!({
                "model": row.model,
                "provider": row.provider,
                "requests": row.requests,
                "tokens": row.tokens,
                "cost": row.cost,
            })
        })
        .collect();
    let total_tokens = total_input_tokens
        .saturating_add(total_output_tokens)
        .saturating_add(total_cache_write_tokens)
        .saturating_add(total_cache_read_tokens);
    let minutes = (window.end_ms.saturating_sub(window.start_ms) as f64 / 60_000.0).max(1.0);

    DashboardRangeUsage {
        matched_requests,
        request_time_series,
        error_time_series,
        token_time_series,
        model_usage,
        summary: json!({
            "totalRequests": matched_requests,
            "successRequests": success_requests,
            "totalInputTokens": total_input_tokens,
            "totalOutputTokens": total_output_tokens,
            "totalCacheWriteTokens": total_cache_write_tokens,
            "totalCacheReadTokens": total_cache_read_tokens,
            "totalTokens": total_tokens,
            "totalCostEstimate": total_cost_estimate,
            "rpm": matched_requests as f64 / minutes,
            "tpm": total_tokens as f64 / minutes,
        }),
    }
}

fn dashboard_usage_timestamp(row: &Value) -> Option<u64> {
    row.get("timestamp").and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
    })
}

fn dashboard_usage_u64(row: &Value, field: &str) -> u64 {
    row.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn dashboard_bucket_timestamp(window: &DashboardTrendWindow, index: usize) -> String {
    window
        .start_ms
        .saturating_add(
            u64::try_from(index)
                .unwrap_or(u64::MAX)
                .saturating_mul(window.bucket_ms),
        )
        .to_string()
}

fn dashboard_value_series(values: &[u64], window: &DashboardTrendWindow) -> Vec<Value> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            json!({
                "timestamp": dashboard_bucket_timestamp(window, index),
                "value": value,
            })
        })
        .collect()
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
                "rechargeBadge": "等待充值",
            },
        })];
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

        let rows = provider_health_rows(&providers, &persisted);
        let row = &rows[0];

        assert_eq!(row["requestsTotal"], 4);
        assert_eq!(row["successRate"], 75.0);
        assert_eq!(row["avgLatencyMs"], 30);
        assert_eq!(row["status"], "degraded");
        assert_eq!(row["inputTokensTotal"], 11);
        assert_eq!(row["rechargeRequired"], true);
        assert_eq!(row["rechargeBadge"], "等待充值");
    }

    #[test]
    fn range_usage_aggregates_every_matching_persisted_row() {
        let window = DashboardTrendWindow {
            range: "custom".to_owned(),
            start_ms: 1_000,
            end_ms: 4_000,
            bucket_ms: 1_000,
        };
        let rows = vec![
            json!({
                "timestamp": "1500",
                "status": "success",
                "resolvedModel": "model-a",
                "provider": "provider-a",
                "inputTokens": 10,
                "outputTokens": 20,
                "cacheWriteTokens": 2,
                "cacheReadTokens": 3,
                "costEstimate": 0.25,
            }),
            json!({
                "timestamp": "2500",
                "status": "error",
                "resolvedModel": "model-a",
                "provider": "provider-a",
                "inputTokens": 5,
                "outputTokens": 0,
                "cacheWriteTokens": 0,
                "cacheReadTokens": 0,
                "costEstimate": 0.05,
            }),
            json!({
                "timestamp": "9000",
                "status": "success",
                "resolvedModel": "outside",
                "provider": "provider-b",
                "inputTokens": 999,
                "outputTokens": 999,
                "costEstimate": 99.0,
            }),
        ];

        let usage = dashboard_range_usage(&rows, &window);

        assert_eq!(usage.matched_requests, 2);
        assert_eq!(usage.request_time_series[0]["value"], 1);
        assert_eq!(usage.request_time_series[1]["value"], 1);
        assert_eq!(usage.error_time_series[1]["value"], 1);
        assert_eq!(usage.model_usage.len(), 1);
        assert_eq!(usage.model_usage[0]["requests"], 2);
        assert_eq!(usage.model_usage[0]["tokens"], 40);
        assert_eq!(usage.summary["totalRequests"], 2);
        assert_eq!(usage.summary["successRequests"], 1);
        assert_eq!(usage.summary["totalTokens"], 40);
        assert_eq!(usage.summary["totalCostEstimate"], 0.3);
    }

    #[test]
    fn empty_historical_window_does_not_look_like_missing_persistence() {
        let window = DashboardTrendWindow {
            range: "custom".to_owned(),
            start_ms: 1_000,
            end_ms: 2_000,
            bucket_ms: 1_000,
        };
        let usage = dashboard_range_usage(
            &[json!({
                "timestamp": "9000",
                "status": "success",
                "resolvedModel": "outside",
                "provider": "provider-a",
            })],
            &window,
        );

        assert_eq!(usage.matched_requests, 0);
        assert_eq!(usage.summary["totalRequests"], 0);
    }

    #[test]
    fn range_usage_excludes_synthetic_and_diagnostic_traffic() {
        let window = DashboardTrendWindow {
            range: "custom".to_owned(),
            start_ms: 1_000,
            end_ms: 4_000,
            bucket_ms: 1_000,
        };
        let rows = vec![
            json!({
                "timestamp": "1500",
                "trafficClass": "business",
                "status": "success",
                "provider": "local",
                "resolvedModel": "model",
                "inputTokens": 10,
            }),
            json!({
                "timestamp": "1600",
                "trafficClass": "synthetic",
                "status": "error",
                "provider": "local",
                "resolvedModel": "model",
                "inputTokens": 999,
            }),
        ];

        let usage = dashboard_range_usage(&rows, &window);

        assert_eq!(usage.matched_requests, 1);
        assert_eq!(usage.summary["successRequests"], 1);
        assert_eq!(usage.summary["totalInputTokens"], 10);
    }
}
