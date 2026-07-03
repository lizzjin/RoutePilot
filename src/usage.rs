use crate::pricing::{self, TokenUsageBreakdown};

#[derive(Debug, Clone, Copy, Default)]
pub struct UsageEstimate {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_estimate: f64,
}

pub(crate) const DAY_MS: u64 = 24 * 60 * 60 * 1_000;

pub(crate) fn quota_increment(quota_type: &str, estimate: UsageEstimate) -> f64 {
    match quota_type {
        "requests" => 1.0,
        "tokens" => estimate
            .input_tokens
            .saturating_add(estimate.output_tokens)
            .saturating_add(estimate.cache_write_tokens)
            .saturating_add(estimate.cache_read_tokens) as f64,
        "cost" => estimate.cost_estimate,
        _ => 0.0,
    }
}

pub(crate) trait UsageCostRecord {
    fn timestamp_ms(&self) -> u64;
    fn api_key_id(&self) -> Option<&str>;
    fn team_id(&self) -> Option<&str>;
    fn resolved_model(&self) -> &str;
    fn token_usage(&self) -> TokenUsageBreakdown;
    fn cost_estimate(&self) -> f64;
}

pub(crate) fn usage_cost_for_api_key<T: UsageCostRecord>(
    usage: &[T],
    api_key_id: &str,
    since: Option<u64>,
) -> f64 {
    usage
        .iter()
        .filter(|record| record.api_key_id() == Some(api_key_id))
        .filter(|record| since.is_none_or(|since| record.timestamp_ms() >= since))
        .map(usage_record_cost)
        .map(|cost| cost.max(0.0))
        .sum()
}

pub(crate) fn usage_cost_for_team<T: UsageCostRecord>(
    usage: &[T],
    team_id: &str,
    since: Option<u64>,
) -> f64 {
    usage
        .iter()
        .filter(|record| record.team_id() == Some(team_id))
        .filter(|record| since.is_none_or(|since| record.timestamp_ms() >= since))
        .map(usage_record_cost)
        .map(|cost| cost.max(0.0))
        .sum()
}

pub(crate) fn usage_record_cost(record: &impl UsageCostRecord) -> f64 {
    let usage = record.token_usage();
    let has_token_breakdown = usage
        .input_tokens
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.cache_write_tokens)
        .saturating_add(usage.cache_read_tokens)
        > 0;
    if !has_token_breakdown {
        return record.cost_estimate();
    }

    pricing::cost_for_model(record.resolved_model(), usage)
}

pub(crate) fn current_period(period: &str, now: u64) -> (u64, u64) {
    match period {
        "daily" => {
            let start = day_start(now);
            (start, start.saturating_add(DAY_MS))
        }
        "weekly" => {
            let start = (now / (DAY_MS * 7)) * (DAY_MS * 7);
            (start, start.saturating_add(DAY_MS * 7))
        }
        "monthly" => {
            let start = (now / (DAY_MS * 30)) * (DAY_MS * 30);
            (start, start.saturating_add(DAY_MS * 30))
        }
        _ => (now, now.saturating_add(DAY_MS)),
    }
}

pub(crate) fn day_start(now: u64) -> u64 {
    (now / DAY_MS) * DAY_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestUsageRecord {
        timestamp_ms: u64,
        api_key_id: Option<String>,
        team_id: Option<String>,
        resolved_model: String,
        token_usage: TokenUsageBreakdown,
        cost_estimate: f64,
    }

    impl UsageCostRecord for TestUsageRecord {
        fn timestamp_ms(&self) -> u64 {
            self.timestamp_ms
        }

        fn api_key_id(&self) -> Option<&str> {
            self.api_key_id.as_deref()
        }

        fn team_id(&self) -> Option<&str> {
            self.team_id.as_deref()
        }

        fn resolved_model(&self) -> &str {
            &self.resolved_model
        }

        fn token_usage(&self) -> TokenUsageBreakdown {
            self.token_usage
        }

        fn cost_estimate(&self) -> f64 {
            self.cost_estimate
        }
    }

    #[test]
    fn quota_increment_matches_quota_type() {
        let estimate = UsageEstimate {
            input_tokens: 10,
            output_tokens: 20,
            cache_write_tokens: 30,
            cache_read_tokens: 40,
            cost_estimate: 0.125,
        };

        assert_eq!(quota_increment("requests", estimate), 1.0);
        assert_eq!(quota_increment("tokens", estimate), 100.0);
        assert_eq!(quota_increment("cost", estimate), 0.125);
        assert_eq!(quota_increment("unknown", estimate), 0.0);
    }

    #[test]
    fn period_windows_are_stable_and_monotonic() {
        let now = DAY_MS * 42 + 1234;

        assert_eq!(current_period("daily", now), (DAY_MS * 42, DAY_MS * 43));
        assert_eq!(current_period("weekly", now), (DAY_MS * 42, DAY_MS * 49));
        assert_eq!(current_period("monthly", now), (DAY_MS * 30, DAY_MS * 60));
        assert_eq!(current_period("custom", now), (now, now + DAY_MS));
    }

    #[test]
    fn usage_record_cost_prefers_token_pricing_when_tokens_exist() {
        let record = TestUsageRecord {
            timestamp_ms: 1,
            api_key_id: Some("key_a".to_owned()),
            team_id: Some("team_a".to_owned()),
            resolved_model: "deepseek-v4-flash".to_owned(),
            token_usage: TokenUsageBreakdown {
                input_tokens: 1_000,
                output_tokens: 2_000,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
            },
            cost_estimate: 999.0,
        };

        assert_eq!(
            usage_record_cost(&record),
            pricing::cost_for_model("deepseek-v4-flash", record.token_usage)
        );
    }

    #[test]
    fn usage_record_cost_falls_back_to_stored_estimate_without_tokens() {
        let record = TestUsageRecord {
            timestamp_ms: 1,
            api_key_id: Some("key_a".to_owned()),
            team_id: Some("team_a".to_owned()),
            resolved_model: "deepseek-v4-flash".to_owned(),
            token_usage: TokenUsageBreakdown::default(),
            cost_estimate: 0.42,
        };

        assert_eq!(usage_record_cost(&record), 0.42);
    }

    #[test]
    fn usage_cost_aggregation_filters_by_owner_and_time() {
        let records = vec![
            TestUsageRecord {
                timestamp_ms: 100,
                api_key_id: Some("key_a".to_owned()),
                team_id: Some("team_a".to_owned()),
                resolved_model: "unknown".to_owned(),
                token_usage: TokenUsageBreakdown::default(),
                cost_estimate: 1.0,
            },
            TestUsageRecord {
                timestamp_ms: 200,
                api_key_id: Some("key_a".to_owned()),
                team_id: Some("team_b".to_owned()),
                resolved_model: "unknown".to_owned(),
                token_usage: TokenUsageBreakdown::default(),
                cost_estimate: 2.0,
            },
            TestUsageRecord {
                timestamp_ms: 300,
                api_key_id: Some("key_b".to_owned()),
                team_id: Some("team_a".to_owned()),
                resolved_model: "unknown".to_owned(),
                token_usage: TokenUsageBreakdown::default(),
                cost_estimate: -10.0,
            },
        ];

        assert_eq!(usage_cost_for_api_key(&records, "key_a", Some(150)), 2.0);
        assert_eq!(usage_cost_for_team(&records, "team_a", None), 1.0);
    }
}
