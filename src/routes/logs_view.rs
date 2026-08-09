use std::{cmp::Reverse, collections::BTreeMap};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{enterprise_ledger::OperationalLogQuery, error::AppError};

use super::{AppState, now_millis};

const DEFAULT_LOG_PAGE_SIZE: usize = 20;
const MAX_LOG_PAGE_SIZE: usize = 500;
const DEFAULT_LOG_WINDOW_MS: u64 = 24 * 60 * 60 * 1_000;
const MAX_LOG_WINDOW_MS: u64 = 31 * 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LogsQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    status: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
    api_key_id: Option<String>,
    date_from: Option<u64>,
    date_to: Option<u64>,
    search: Option<String>,
    // These fields keep the existing dashboard filters server-side as well.
    username: Option<String>,
    group: Option<String>,
    stream: Option<String>,
    tool_use: Option<String>,
    traffic_class: Option<String>,
}

impl LogsQuery {
    pub(super) fn validate(&self) -> Result<(), AppError> {
        for (name, value) in [
            ("status", self.status.as_deref()),
            ("provider", self.provider.as_deref()),
            ("model", self.model.as_deref()),
            ("userId", self.user_id.as_deref()),
            ("apiKeyId", self.api_key_id.as_deref()),
            ("username", self.username.as_deref()),
            ("group", self.group.as_deref()),
            ("stream", self.stream.as_deref()),
            ("toolUse", self.tool_use.as_deref()),
            ("trafficClass", self.traffic_class.as_deref()),
        ] {
            if value.is_some_and(|value| value.chars().count() > 256) {
                return Err(AppError::InvalidRequest(format!(
                    "{name} must be at most 256 characters"
                )));
            }
        }
        if self
            .search
            .as_deref()
            .is_some_and(|search| search.chars().count() > 512)
        {
            return Err(AppError::InvalidRequest(
                "search must be at most 512 characters".to_owned(),
            ));
        }
        if self
            .status
            .as_deref()
            .is_some_and(|status| !matches!(status, "success" | "error" | "timeout"))
        {
            return Err(AppError::InvalidRequest(
                "status must be success, error, or timeout".to_owned(),
            ));
        }
        if self
            .stream
            .as_deref()
            .is_some_and(|stream| !matches!(stream, "stream" | "non-stream"))
        {
            return Err(AppError::InvalidRequest(
                "stream must be stream or non-stream".to_owned(),
            ));
        }
        if self
            .tool_use
            .as_deref()
            .is_some_and(|tool_use| !matches!(tool_use, "requested" | "not-requested"))
        {
            return Err(AppError::InvalidRequest(
                "toolUse must be requested or not-requested".to_owned(),
            ));
        }
        if self.traffic_class.as_deref().is_some_and(|traffic_class| {
            !matches!(traffic_class, "business" | "synthetic" | "diagnostic")
        }) {
            return Err(AppError::InvalidRequest(
                "trafficClass must be business, synthetic, or diagnostic".to_owned(),
            ));
        }
        if self
            .date_from
            .zip(self.date_to)
            .is_some_and(|(from, to)| from > to)
        {
            return Err(AppError::InvalidRequest(
                "dateFrom must not be later than dateTo".to_owned(),
            ));
        }
        let effective_to = self.date_to.unwrap_or_else(now_millis);
        let effective_from = self
            .date_from
            .unwrap_or_else(|| effective_to.saturating_sub(DEFAULT_LOG_WINDOW_MS));
        if effective_to.saturating_sub(effective_from) > MAX_LOG_WINDOW_MS {
            return Err(AppError::InvalidRequest(
                "request log windows must not exceed 31 days".to_owned(),
            ));
        }
        Ok(())
    }

    fn with_effective_window(&self) -> Self {
        let mut query = self.clone();
        let effective_to = query.date_to.unwrap_or_else(now_millis);
        query.date_from = Some(
            query
                .date_from
                .unwrap_or_else(|| effective_to.saturating_sub(DEFAULT_LOG_WINDOW_MS)),
        );
        query
    }

    /// Applies the server-side principal boundary for request-log reads.
    ///
    /// This deliberately overwrites (rather than trusts) a caller supplied
    /// `userId`.  Keeping the ownership predicate in the query object means it
    /// is applied by both the PostgreSQL and in-memory ledger backends before
    /// pagination and summary aggregation.
    pub(super) fn scoped_to_user(&self, user_id: &str) -> Self {
        let mut query = self.clone();
        query.user_id = Some(user_id.to_owned());
        query
    }

    fn operational_query(&self) -> OperationalLogQuery {
        OperationalLogQuery {
            page: self.page.unwrap_or(1).max(1),
            page_size: self
                .page_size
                .unwrap_or(DEFAULT_LOG_PAGE_SIZE)
                .clamp(1, MAX_LOG_PAGE_SIZE),
            status: self.status.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            user_id: self.user_id.clone(),
            api_key_id: self.api_key_id.clone(),
            date_from: self.date_from,
            date_to: self.date_to,
            search: self.search.clone(),
            username: self.username.clone(),
            group: self.group.clone(),
            stream: self.stream.as_deref().map(|value| value == "stream"),
            tool_use_requested: self.tool_use.as_deref().map(|value| value == "requested"),
            traffic_class: self.traffic_class.clone(),
        }
    }
}

pub(super) async fn logs_body(state: &AppState, query: &LogsQuery) -> Result<Value, AppError> {
    let query = query.with_effective_window();
    if let Some(page) = state
        .ledger
        .operational_logs(&query.operational_query())
        .await?
    {
        return Ok(json!({
            "logs": page.logs,
            "total": page.total,
            "summary": page.summary,
        }));
    }
    Ok(logs_body_from_rows(
        state.ledger.usage_rows_since(query.date_from).await?,
        &query,
    ))
}

pub(super) async fn log_body(state: &AppState, id: &str) -> Result<Option<Value>, AppError> {
    state.ledger.usage_row(id).await
}

pub(super) fn log_belongs_to_user(row: &Value, user_id: &str) -> bool {
    row.get("userId").and_then(Value::as_str) == Some(user_id)
}

fn logs_body_from_rows(mut logs: Vec<Value>, query: &LogsQuery) -> Value {
    logs.retain(|row| log_matches(row, query));
    logs.sort_by_key(|row| Reverse(timestamp_millis(row)));
    let total = logs.len();
    let summary = summarize_logs(&logs);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_LOG_PAGE_SIZE)
        .clamp(1, MAX_LOG_PAGE_SIZE);
    let start = page.saturating_sub(1).saturating_mul(page_size);
    let logs = logs
        .into_iter()
        .skip(start)
        .take(page_size)
        .collect::<Vec<_>>();

    json!({
        "logs": logs,
        "total": total,
        "summary": summary,
    })
}

fn log_matches(row: &Value, query: &LogsQuery) -> bool {
    if query
        .status
        .as_deref()
        .is_some_and(|expected| !field_equals(row, "status", expected))
        || query
            .provider
            .as_deref()
            .is_some_and(|expected| !field_equals(row, "provider", expected))
        || query
            .user_id
            .as_deref()
            .is_some_and(|expected| !field_equals(row, "userId", expected))
        || query
            .api_key_id
            .as_deref()
            .is_some_and(|expected| !field_equals(row, "apiKeyId", expected))
        || query
            .stream
            .as_deref()
            .is_some_and(|expected| !field_equals(row, "stream", expected))
        || query.tool_use.as_deref().is_some_and(|expected| {
            row.get("toolUseRequested").and_then(Value::as_bool) != Some(expected == "requested")
        })
        || query.traffic_class.as_deref().is_some_and(|expected| {
            row.get("trafficClass")
                .and_then(Value::as_str)
                .unwrap_or("business")
                != expected
        })
    {
        return false;
    }

    if query
        .model
        .as_deref()
        .is_some_and(|expected| !any_field_contains(row, &["model", "resolvedModel"], expected))
        || query
            .username
            .as_deref()
            .is_some_and(|expected| !field_contains(row, "username", expected))
        || query
            .group
            .as_deref()
            .is_some_and(|expected| !field_contains(row, "apiKeyGroup", expected))
    {
        return false;
    }

    let timestamp = timestamp_millis(row);
    if query
        .date_from
        .is_some_and(|date_from| timestamp.is_none_or(|value| value < date_from))
        || query
            .date_to
            .is_some_and(|date_to| timestamp.is_none_or(|value| value > date_to))
    {
        return false;
    }

    query.search.as_deref().is_none_or(|search| {
        any_field_contains(
            row,
            &[
                "id",
                "requestId",
                "attemptId",
                "provider",
                "model",
                "resolvedModel",
                "userId",
                "username",
                "apiKeyId",
                "apiKeyName",
                "apiKeyGroup",
                "teamId",
                "teamName",
                "errorMessage",
                "terminalReason",
                "requestPath",
                "clientProtocol",
                "protocol",
                "trafficClass",
            ],
            search,
        )
    })
}

fn field_equals(row: &Value, field: &str, expected: &str) -> bool {
    row.get(field).and_then(Value::as_str) == Some(expected)
}

fn field_contains(row: &Value, field: &str, expected: &str) -> bool {
    let expected = expected.trim().to_lowercase();
    expected.is_empty() || field_contains_normalized(row, field, &expected)
}

fn any_field_contains(row: &Value, fields: &[&str], expected: &str) -> bool {
    let expected = expected.trim().to_lowercase();
    expected.is_empty()
        || fields
            .iter()
            .any(|field| field_contains_normalized(row, field, &expected))
}

fn field_contains_normalized(row: &Value, field: &str, expected: &str) -> bool {
    row.get(field)
        .and_then(Value::as_str)
        .is_some_and(|value| value.to_lowercase().contains(expected))
}

fn timestamp_millis(row: &Value) -> Option<u64> {
    row.get("timestamp").and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
    })
}

fn summarize_logs(logs: &[Value]) -> Value {
    let mut success_requests = 0usize;
    let mut tool_use_requests = 0usize;
    let mut tool_use_success_requests = 0usize;
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;
    let mut total_cache_write_tokens = 0u64;
    let mut total_cache_read_tokens = 0u64;
    let mut total_cost_estimate = 0.0f64;
    let mut latency_values = Vec::with_capacity(logs.len());
    let mut first_byte_latency_values = Vec::new();
    let mut first_timestamp = None::<u64>;
    let mut last_timestamp = None::<u64>;

    for log in logs {
        if log.get("status").and_then(Value::as_str) == Some("success") {
            success_requests += 1;
        }
        if log
            .get("toolUseRequested")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            tool_use_requests += 1;
            if log.get("status").and_then(Value::as_str) == Some("success") {
                tool_use_success_requests += 1;
            }
        }
        total_input_tokens = total_input_tokens.saturating_add(field_u64(log, "inputTokens"));
        total_output_tokens = total_output_tokens.saturating_add(field_u64(log, "outputTokens"));
        total_cache_write_tokens =
            total_cache_write_tokens.saturating_add(field_u64(log, "cacheWriteTokens"));
        total_cache_read_tokens =
            total_cache_read_tokens.saturating_add(field_u64(log, "cacheReadTokens"));
        total_cost_estimate += log
            .get("costEstimate")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if let Some(latency) = log.get("latencyMs").and_then(Value::as_u64) {
            latency_values.push(latency);
        }
        if let Some(latency) = log.get("firstByteLatencyMs").and_then(Value::as_u64) {
            first_byte_latency_values.push(latency);
        }
        if let Some(timestamp) = timestamp_millis(log) {
            first_timestamp = Some(first_timestamp.map_or(timestamp, |value| value.min(timestamp)));
            last_timestamp = Some(last_timestamp.map_or(timestamp, |value| value.max(timestamp)));
        }
    }

    let total_tokens = total_input_tokens
        .saturating_add(total_output_tokens)
        .saturating_add(total_cache_write_tokens)
        .saturating_add(total_cache_read_tokens);
    let minutes = match (first_timestamp, last_timestamp) {
        (Some(first), Some(last)) if last > first => ((last - first) as f64 / 60_000.0).max(1.0),
        _ => 1.0,
    };
    latency_values.sort_unstable();
    first_byte_latency_values.sort_unstable();

    json!({
        "totalRequests": logs.len(),
        "successRequests": success_requests,
        "toolUseRequests": tool_use_requests,
        "toolUseSuccessRequests": tool_use_success_requests,
        "totalInputTokens": total_input_tokens,
        "totalOutputTokens": total_output_tokens,
        "totalCacheWriteTokens": total_cache_write_tokens,
        "totalCacheReadTokens": total_cache_read_tokens,
        "totalTokens": total_tokens,
        "totalCostEstimate": total_cost_estimate,
        "latencyP95Ms": percentile(&latency_values, 95),
        "latencySampleCount": latency_values.len(),
        "firstByteLatencyP95Ms": percentile(&first_byte_latency_values, 95),
        "firstByteLatencySampleCount": first_byte_latency_values.len(),
        "rpm": logs.len() as f64 / minutes,
        "tpm": total_tokens as f64 / minutes,
    })
}

fn field_u64(row: &Value, field: &str) -> u64 {
    row.get(field).and_then(Value::as_u64).unwrap_or(0)
}

pub(super) async fn latency_body(state: &AppState) -> Result<Value, AppError> {
    let since = now_millis().saturating_sub(24 * 60 * 60 * 1_000);
    if let Some(stats) = state.ledger.latency_stats_since(since).await? {
        return Ok(stats);
    }
    Ok(latency_body_from_usage(
        &state.ledger.usage_rows_since(Some(since)).await?,
    ))
}

fn latency_body_from_usage(rows: &[Value]) -> Value {
    let mut all = Vec::with_capacity(rows.len());
    let mut by_model = BTreeMap::<String, Vec<u64>>::new();
    let mut by_provider = BTreeMap::<String, Vec<u64>>::new();

    for row in rows {
        let Some(latency) = row.get("latencyMs").and_then(Value::as_u64) else {
            continue;
        };
        all.push(latency);
        if let Some(model) = row.get("resolvedModel").and_then(Value::as_str) {
            by_model.entry(model.to_owned()).or_default().push(latency);
        }
        if let Some(provider) = row.get("provider").and_then(Value::as_str) {
            by_provider
                .entry(provider.to_owned())
                .or_default()
                .push(latency);
        }
    }

    let overall = latency_stats(&all);
    json!({
        "p50": overall["p50"],
        "p90": overall["p90"],
        "p95": overall["p95"],
        "p99": overall["p99"],
        "avg": overall["avg"],
        "max": overall["max"],
        "byModel": grouped_latency_stats(by_model),
        "byProvider": grouped_latency_stats(by_provider),
        "sampleCount": all.len(),
        "percentilesEstimated": false,
    })
}

fn grouped_latency_stats(groups: BTreeMap<String, Vec<u64>>) -> Value {
    Value::Object(
        groups
            .into_iter()
            .map(|(name, values)| (name, latency_stats(&values)))
            .collect::<Map<String, Value>>(),
    )
}

fn latency_stats(values: &[u64]) -> Value {
    if values.is_empty() {
        return json!({
            "p50": 0,
            "p90": 0,
            "p95": 0,
            "p99": 0,
            "avg": 0,
            "max": 0,
            "count": 0,
        });
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let total = sorted.iter().copied().fold(0u64, u64::saturating_add);
    json!({
        "p50": percentile(&sorted, 50),
        "p90": percentile(&sorted, 90),
        "p95": percentile(&sorted, 95),
        "p99": percentile(&sorted, 99),
        "avg": average(total, sorted.len() as u64),
        "max": sorted.last().copied().unwrap_or(0),
        "count": sorted.len(),
    })
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = percentile
        .saturating_mul(sorted.len())
        .div_ceil(100)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[rank]
}

fn average(total: u64, count: u64) -> u64 {
    total.checked_div(count).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_body_calculates_percentiles_from_persisted_usage() {
        let rows = vec![
            json!({ "latencyMs": 10, "resolvedModel": "model-a", "provider": "one" }),
            json!({ "latencyMs": 20, "resolvedModel": "model-a", "provider": "one" }),
            json!({ "latencyMs": 30, "resolvedModel": "model-b", "provider": "two" }),
            json!({ "latencyMs": 100, "resolvedModel": "model-b", "provider": "two" }),
        ];

        let body = latency_body_from_usage(&rows);

        assert_eq!(body["p50"], 20);
        assert_eq!(body["p90"], 100);
        assert_eq!(body["avg"], 40);
        assert_eq!(body["max"], 100);
        assert_eq!(body["sampleCount"], 4);
        assert_eq!(body["byModel"]["model-a"]["p95"], 20);
        assert_eq!(body["byProvider"]["two"]["avg"], 65);
        assert_eq!(body["percentilesEstimated"], false);
    }

    #[test]
    fn logs_query_filters_all_supported_dimensions_and_epoch_millis() {
        let mut rows = vec![
            test_log(
                "log-one",
                "req-one",
                1_000,
                "success",
                "provider-one",
                "model-one",
                "user-one",
                "key-one",
                None,
            ),
            test_log(
                "log-two",
                "req-two",
                2_000,
                "error",
                "provider-two",
                "model-two",
                "user-two",
                "key-two",
                Some("Upstream exploded"),
            ),
        ];
        rows[1]["toolUseRequested"] = json!(true);
        let query = LogsQuery {
            status: Some("error".to_owned()),
            provider: Some("provider-two".to_owned()),
            model: Some("MODEL-TWO".to_owned()),
            user_id: Some("user-two".to_owned()),
            api_key_id: Some("key-two".to_owned()),
            date_from: Some(2_000),
            date_to: Some(2_000),
            search: Some("UPSTREAM EXPLODED".to_owned()),
            tool_use: Some("requested".to_owned()),
            ..LogsQuery::default()
        };

        let body = logs_body_from_rows(rows.clone(), &query);

        assert_eq!(body["total"], 1);
        assert_eq!(body["logs"][0]["id"], "log-two");
        for search in [
            "LOG-TWO",
            "REQ-TWO",
            "provider-two",
            "model-two",
            "user-two",
            "key-two",
            "Upstream exploded",
        ] {
            assert!(log_matches(
                &rows[1],
                &LogsQuery {
                    search: Some(search.to_owned()),
                    ..LogsQuery::default()
                }
            ));
        }
    }

    #[test]
    fn logs_query_summarizes_filtered_rows_before_pagination() {
        let mut rows = vec![
            test_log(
                "log-one", "req-one", 0, "success", "provider", "model", "user", "key", None,
            ),
            test_log(
                "log-two",
                "req-two",
                60_000,
                "error",
                "provider",
                "model",
                "user",
                "key",
                Some("failed"),
            ),
            test_log(
                "log-three",
                "req-three",
                120_000,
                "success",
                "provider",
                "model",
                "user",
                "key",
                None,
            ),
        ];
        rows[0]["toolUseRequested"] = json!(true);
        rows[1]["toolUseRequested"] = json!(true);
        rows[0]["latencyMs"] = json!(100);
        rows[1]["latencyMs"] = json!(200);
        rows[2]["latencyMs"] = json!(1_000);
        rows[0]["firstByteLatencyMs"] = json!(25);
        rows[2]["firstByteLatencyMs"] = json!(250);
        let query = LogsQuery {
            page: Some(2),
            page_size: Some(1),
            ..LogsQuery::default()
        };

        let body = logs_body_from_rows(rows, &query);

        assert_eq!(body["logs"].as_array().unwrap().len(), 1);
        assert_eq!(body["logs"][0]["id"], "log-two");
        assert_eq!(body["total"], 3);
        assert_eq!(body["summary"]["totalRequests"], 3);
        assert_eq!(body["summary"]["successRequests"], 2);
        assert_eq!(body["summary"]["toolUseRequests"], 2);
        assert_eq!(body["summary"]["toolUseSuccessRequests"], 1);
        assert_eq!(body["summary"]["totalTokens"], 30);
        assert_eq!(body["summary"]["totalCostEstimate"], 0.75);
        assert_eq!(body["summary"]["latencyP95Ms"], 1_000);
        assert_eq!(body["summary"]["latencySampleCount"], 3);
        assert_eq!(body["summary"]["firstByteLatencyP95Ms"], 250);
        assert_eq!(body["summary"]["firstByteLatencySampleCount"], 2);
        assert_eq!(body["summary"]["rpm"], 1.5);
        assert_eq!(body["summary"]["tpm"], 15.0);
    }

    #[test]
    fn logs_query_clamps_page_size_to_server_limit() {
        let rows = (0..501)
            .map(|index| {
                test_log(
                    &format!("log-{index}"),
                    &format!("req-{index}"),
                    index,
                    "success",
                    "provider",
                    "model",
                    "user",
                    "key",
                    None,
                )
            })
            .collect();
        let query: LogsQuery = serde_json::from_value(json!({
            "page": 0,
            "pageSize": 999,
        }))
        .unwrap();

        let body = logs_body_from_rows(rows, &query);

        assert_eq!(body["logs"].as_array().unwrap().len(), MAX_LOG_PAGE_SIZE);
        assert_eq!(body["logs"][0]["id"], "log-500");
        assert_eq!(body["total"], 501);
    }

    #[test]
    fn logs_query_rejects_invalid_enums_and_reversed_date_range() {
        for query in [
            LogsQuery {
                status: Some("unknown".to_owned()),
                ..LogsQuery::default()
            },
            LogsQuery {
                stream: Some("sometimes".to_owned()),
                ..LogsQuery::default()
            },
            LogsQuery {
                tool_use: Some("sometimes".to_owned()),
                ..LogsQuery::default()
            },
            LogsQuery {
                traffic_class: Some("other".to_owned()),
                ..LogsQuery::default()
            },
            LogsQuery {
                date_from: Some(2),
                date_to: Some(1),
                ..LogsQuery::default()
            },
            LogsQuery {
                search: Some("x".repeat(513)),
                ..LogsQuery::default()
            },
        ] {
            assert!(matches!(query.validate(), Err(AppError::InvalidRequest(_))));
        }
    }

    #[test]
    fn logs_query_defaults_to_one_day_and_rejects_unbounded_windows() {
        let upper = 90 * 24 * 60 * 60 * 1_000;
        let defaulted = LogsQuery {
            date_to: Some(upper),
            ..LogsQuery::default()
        }
        .with_effective_window();
        assert_eq!(
            defaulted.date_from,
            Some(upper.saturating_sub(DEFAULT_LOG_WINDOW_MS))
        );
        assert!(
            LogsQuery {
                date_from: Some(1),
                date_to: Some(1 + MAX_LOG_WINDOW_MS + 1),
                ..LogsQuery::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn logs_query_filters_traffic_class() {
        let mut synthetic = test_log(
            "log-synthetic",
            "req-synthetic",
            2,
            "success",
            "provider",
            "model",
            "user",
            "key",
            None,
        );
        synthetic["trafficClass"] = json!("synthetic");
        let mut business = test_log(
            "log-business",
            "req-business",
            1,
            "success",
            "provider",
            "model",
            "user",
            "key",
            None,
        );
        business["trafficClass"] = json!("business");

        let synthetic_body = logs_body_from_rows(
            vec![business.clone(), synthetic.clone()],
            &LogsQuery {
                traffic_class: Some("synthetic".to_owned()),
                ..LogsQuery::default()
            },
        );
        assert_eq!(synthetic_body["total"], 1);
        assert_eq!(synthetic_body["logs"][0]["id"], "log-synthetic");

        let business_body = logs_body_from_rows(
            vec![business, synthetic],
            &LogsQuery {
                traffic_class: Some("business".to_owned()),
                ..LogsQuery::default()
            },
        );
        assert_eq!(business_body["total"], 1);
        assert_eq!(business_body["logs"][0]["id"], "log-business");
    }

    #[allow(clippy::too_many_arguments)]
    fn test_log(
        id: &str,
        request_id: &str,
        timestamp: u64,
        status: &str,
        provider: &str,
        model: &str,
        user_id: &str,
        api_key_id: &str,
        error_message: Option<&str>,
    ) -> Value {
        json!({
            "id": id,
            "requestId": request_id,
            "timestamp": timestamp.to_string(),
            "status": status,
            "provider": provider,
            "model": model,
            "resolvedModel": model,
            "userId": user_id,
            "username": format!("{user_id} name"),
            "apiKeyId": api_key_id,
            "apiKeyName": format!("{api_key_id} name"),
            "apiKeyGroup": "test-group",
            "trafficClass": "business",
            "stream": "non-stream",
            "inputTokens": 4,
            "outputTokens": 3,
            "cacheWriteTokens": 2,
            "cacheReadTokens": 1,
            "costEstimate": 0.25,
            "errorMessage": error_message,
        })
    }
}
