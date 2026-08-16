#[derive(Debug, Clone, Copy, Default)]
pub struct UsageEstimate {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_estimate: f64,
    pub actual_cost: Option<f64>,
    pub billable_cost: Option<f64>,
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

pub(crate) fn current_period(period: &str, now: u64) -> (u64, u64) {
    match period {
        "daily" => {
            let start = day_start(now);
            (start, start.saturating_add(DAY_MS))
        }
        "weekly" => {
            // UTC calendar week, Monday 00:00 through the next Monday.
            let days_since_epoch = now / DAY_MS;
            let weekday_from_monday = days_since_epoch.saturating_add(3) % 7;
            let start = days_since_epoch
                .saturating_sub(weekday_from_monday)
                .saturating_mul(DAY_MS);
            (start, start.saturating_add(DAY_MS * 7))
        }
        "monthly" => {
            // UTC calendar month; use civil-date arithmetic so leap years and
            // 28/29/30/31-day months reset on the first day as users expect.
            let days_since_epoch = i64::try_from(now / DAY_MS).unwrap_or(i64::MAX);
            let (year, month, _) = civil_from_days(days_since_epoch);
            let (next_year, next_month) = if month == 12 {
                (year.saturating_add(1), 1)
            } else {
                (year, month + 1)
            };
            let start = millis_from_civil(year, month, 1);
            let end = millis_from_civil(next_year, next_month, 1);
            (start, end)
        }
        _ => (now, now.saturating_add(DAY_MS)),
    }
}

fn millis_from_civil(year: i64, month: u32, day: u32) -> u64 {
    u64::try_from(days_from_civil(year, month, day))
        .unwrap_or(0)
        .saturating_mul(DAY_MS)
}

// Gregorian civil-date conversions adapted from Howard Hinnant's public-domain
// algorithms. Inputs and outputs are days relative to 1970-01-01 UTC.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch.saturating_add(719_468);
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month as u32, day as u32)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(crate) fn day_start(now: u64) -> u64 {
    (now / DAY_MS) * DAY_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_increment_matches_quota_type() {
        let estimate = UsageEstimate {
            input_tokens: 10,
            output_tokens: 20,
            cache_write_tokens: 30,
            cache_read_tokens: 40,
            cost_estimate: 0.125,
            actual_cost: None,
            billable_cost: None,
        };

        assert_eq!(quota_increment("requests", estimate), 1.0);
        assert_eq!(quota_increment("tokens", estimate), 100.0);
        assert_eq!(quota_increment("cost", estimate), 0.125);
        assert_eq!(quota_increment("unknown", estimate), 0.0);
    }

    #[test]
    fn quota_periods_use_utc_calendar_boundaries() {
        assert_eq!(
            current_period("weekly", 1_720_569_600_000),
            (1_720_396_800_000, 1_721_001_600_000)
        );
        assert_eq!(
            current_period("monthly", 1_707_955_200_000),
            (1_706_745_600_000, 1_709_251_200_000)
        );
    }

    #[test]
    fn period_windows_are_stable_and_monotonic() {
        let now = DAY_MS * 42 + 1234;

        assert_eq!(current_period("daily", now), (DAY_MS * 42, DAY_MS * 43));
        // 1970-02-12 is a Thursday: its UTC calendar week starts on
        // Monday 1970-02-09 (epoch day 39), and February spans days 31..59.
        assert_eq!(current_period("weekly", now), (DAY_MS * 39, DAY_MS * 46));
        assert_eq!(current_period("monthly", now), (DAY_MS * 31, DAY_MS * 59));
        assert_eq!(current_period("custom", now), (now, now + DAY_MS));
    }
}
