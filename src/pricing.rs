use axum::http::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;

pub const USAGE_HEADER: &str = "x-modelport-usage";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageBreakdown {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageCharge {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_estimate: f64,
    /// Provider-reported or exact-rate-card cost derived from Provider usage.
    /// `None` means the request has not produced invoice-grade cost evidence.
    pub actual_cost: Option<f64>,
    /// Amount eligible for governed chargeback. This is deliberately distinct
    /// from `cost_estimate`; estimates must never silently become charges.
    pub billable_cost: Option<f64>,
    pub pricing_evidence: Option<PricingEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricing {
    #[serde(alias = "input_per_million")]
    pub input_per_million: f64,
    #[serde(alias = "output_per_million")]
    pub output_per_million: f64,
    #[serde(alias = "cache_write_per_million")]
    pub cache_write_per_million: f64,
    #[serde(alias = "cache_read_per_million")]
    pub cache_read_per_million: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    ProviderPublished,
    ProviderContract,
    InternalChargeback,
    #[default]
    LegacyEstimate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingServiceTier {
    #[default]
    Standard,
    Batch,
    Flex,
    Priority,
    Custom,
}

/// An exact-model, versioned rate card. Legacy `[provider.pricing]` remains an
/// estimate-only compatibility surface; only this evidence-bearing shape can
/// turn Provider token usage into governed billable cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricingCard {
    #[serde(flatten)]
    pub rates: ModelPricing,
    pub version: String,
    #[serde(alias = "effective_at")]
    pub effective_at: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    pub source: PricingSource,
    #[serde(default, alias = "service_tier")]
    pub service_tier: PricingServiceTier,
    #[serde(default)]
    pub region: Option<String>,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingMethod {
    ProviderReported,
    ExactRateCard,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingEvidence {
    pub provider: String,
    pub model: String,
    pub method: PricingMethod,
    pub currency: String,
    pub version: String,
    pub effective_at: String,
    pub source: PricingSource,
    pub service_tier: PricingServiceTier,
    pub region: Option<String>,
    pub evidence: String,
    pub rates: Option<ModelPricing>,
}

fn default_currency() -> String {
    "USD".to_owned()
}

pub fn validate_model_pricing_card(
    model: &str,
    allowed_models: &[String],
    card: &ModelPricingCard,
) -> Result<(), String> {
    if model.trim().is_empty() || !allowed_models.iter().any(|allowed| allowed == model) {
        return Err(format!(
            "model_pricing requires an exact model from the provider models list; invalid key `{model}`"
        ));
    }
    for (field, value) in [
        ("input_per_million", card.rates.input_per_million),
        ("output_per_million", card.rates.output_per_million),
        (
            "cache_write_per_million",
            card.rates.cache_write_per_million,
        ),
        ("cache_read_per_million", card.rates.cache_read_per_million),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "model_pricing.{model}.{field} must be a finite non-negative number"
            ));
        }
    }
    if card.currency != "USD" {
        return Err(format!(
            "model_pricing.{model}.currency must be USD until currency conversion is implemented"
        ));
    }
    if card.source == PricingSource::LegacyEstimate {
        return Err(format!(
            "model_pricing.{model}.source cannot be legacy_estimate; use provider-wide pricing for estimates"
        ));
    }
    if card.service_tier != PricingServiceTier::Standard {
        return Err(format!(
            "model_pricing.{model}.service_tier must be standard until request-tier matching is implemented"
        ));
    }
    for (field, value, max_len) in [
        ("version", card.version.as_str(), 120usize),
        ("evidence", card.evidence.as_str(), 1000usize),
    ] {
        if value.trim().is_empty() || value.len() > max_len || value.chars().any(char::is_control) {
            return Err(format!(
                "model_pricing.{model}.{field} must be non-empty, bounded, and contain no control characters"
            ));
        }
    }
    if !valid_utc_rate_card_timestamp(&card.effective_at) {
        return Err(format!(
            "model_pricing.{model}.effective_at must be an RFC3339 UTC timestamp"
        ));
    }
    Ok(())
}

fn valid_utc_rate_card_timestamp(value: &str) -> bool {
    if !value.is_ascii() || value.len() < 20 || value.len() > 30 || !value.ends_with('Z') {
        return false;
    }
    let bytes = value.as_bytes();
    if bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return false;
    }
    let number = |start: usize, end: usize| value[start..end].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(0, 4),
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
    ) else {
        return false;
    };
    let leap_year =
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return false,
    };
    if !(1..=max_day).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return false;
    }
    let fractional = &value[19..value.len() - 1];
    fractional.is_empty()
        || (fractional.starts_with('.')
            && fractional.len() > 1
            && fractional[1..].bytes().all(|byte| byte.is_ascii_digit()))
}

pub fn charge_with_evidence(
    provider: &str,
    model: &str,
    usage: TokenUsageBreakdown,
    legacy_pricing: Option<ModelPricing>,
    pricing_card: Option<&ModelPricingCard>,
    provider_reported_cost: Option<f64>,
) -> UsageCharge {
    let applied_pricing = pricing_card
        .map(|card| card.rates)
        .or(legacy_pricing)
        .unwrap_or_else(|| pricing_for_model(model));
    let calculated_cost = cost_with_pricing(usage, applied_pricing);
    let provider_reported_cost =
        provider_reported_cost.filter(|value| value.is_finite() && *value >= 0.0);

    let (actual_cost, pricing_evidence) = if let Some(cost) = provider_reported_cost {
        (
            Some(cost),
            Some(PricingEvidence {
                provider: provider.to_owned(),
                model: model.to_owned(),
                method: PricingMethod::ProviderReported,
                currency: "USD".to_owned(),
                version: "provider-response-v1".to_owned(),
                effective_at: "response-time".to_owned(),
                source: PricingSource::ProviderContract,
                service_tier: PricingServiceTier::Standard,
                region: None,
                evidence: "upstream usage.cost".to_owned(),
                rates: None,
            }),
        )
    } else if let Some(card) = pricing_card {
        (
            Some(calculated_cost),
            Some(PricingEvidence {
                provider: provider.to_owned(),
                model: model.to_owned(),
                method: PricingMethod::ExactRateCard,
                currency: card.currency.clone(),
                version: card.version.clone(),
                effective_at: card.effective_at.clone(),
                source: card.source,
                service_tier: card.service_tier,
                region: card.region.clone(),
                evidence: card.evidence.clone(),
                rates: Some(card.rates),
            }),
        )
    } else {
        (None, None)
    };

    UsageCharge {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_write_tokens: usage.cache_write_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cost_estimate: calculated_cost,
        actual_cost,
        billable_cost: actual_cost,
        pricing_evidence,
    }
}

pub fn cost_for_model_with_pricing(
    model: &str,
    usage: TokenUsageBreakdown,
    configured_pricing: Option<ModelPricing>,
) -> f64 {
    let pricing = configured_pricing.unwrap_or_else(|| pricing_for_model(model));
    cost_with_pricing(usage, pricing)
}

pub fn cost_with_pricing(usage: TokenUsageBreakdown, pricing: ModelPricing) -> f64 {
    cost_component(usage.input_tokens, pricing.input_per_million)
        + cost_component(usage.output_tokens, pricing.output_per_million)
        + cost_component(usage.cache_write_tokens, pricing.cache_write_per_million)
        + cost_component(usage.cache_read_tokens, pricing.cache_read_per_million)
}

pub fn usage_header_value(
    provider: &str,
    model: &str,
    usage: TokenUsageBreakdown,
    legacy_pricing: Option<ModelPricing>,
    pricing_card: Option<&ModelPricingCard>,
    provider_reported_cost: Option<f64>,
) -> Result<HeaderValue, AppError> {
    let charge = charge_with_evidence(
        provider,
        model,
        usage,
        legacy_pricing,
        pricing_card,
        provider_reported_cost,
    );
    HeaderValue::from_str(&serde_json::to_string(&charge)?)
        .map_err(|err| AppError::Config(format!("invalid usage header: {err}")))
}

pub fn openai_reported_cost(response: &Value) -> Option<f64> {
    response
        .get("usage")?
        .get("cost")?
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub fn usage_from_headers(headers: &HeaderMap) -> Option<UsageCharge> {
    headers
        .get(USAGE_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| serde_json::from_str(value).ok())
}

pub fn openai_usage(response: &Value) -> TokenUsageBreakdown {
    let Some(usage) = response.get("usage") else {
        return TokenUsageBreakdown::default();
    };

    let prompt_tokens = get_u64(usage, &["prompt_tokens", "input_tokens"]);
    let output_tokens = get_u64(usage, &["completion_tokens", "output_tokens"]);
    let deepseek_cache_hit = get_u64(usage, &["prompt_cache_hit_tokens"]);
    let deepseek_cache_miss = get_u64(usage, &["prompt_cache_miss_tokens"]);
    let cached_tokens = get_nested_u64(
        usage,
        &[
            &["prompt_tokens_details", "cached_tokens"],
            &["input_tokens_details", "cached_tokens"],
        ],
    );
    let cache_read_tokens = deepseek_cache_hit.max(cached_tokens);
    let input_tokens = if deepseek_cache_miss > 0 {
        deepseek_cache_miss
    } else {
        prompt_tokens.saturating_sub(cache_read_tokens)
    };

    TokenUsageBreakdown {
        input_tokens,
        output_tokens,
        cache_write_tokens: get_u64(
            usage,
            &["cache_creation_input_tokens", "prompt_cache_write_tokens"],
        )
        .max(get_nested_u64(
            usage,
            &[
                &["prompt_tokens_details", "cache_write_tokens"],
                &["input_tokens_details", "cache_write_tokens"],
            ],
        )),
        cache_read_tokens,
    }
}

pub fn openai_usage_if_present(response: &Value) -> Option<TokenUsageBreakdown> {
    let usage = response.get("usage")?.as_object()?;
    let has_supported_field = [
        "prompt_tokens",
        "input_tokens",
        "completion_tokens",
        "output_tokens",
        "prompt_cache_hit_tokens",
        "prompt_cache_miss_tokens",
        "cache_creation_input_tokens",
        "prompt_cache_write_tokens",
        "prompt_tokens_details",
        "input_tokens_details",
    ]
    .iter()
    .any(|field| usage.contains_key(*field));
    has_supported_field.then(|| openai_usage(response))
}

pub fn anthropic_usage(response: &Value) -> TokenUsageBreakdown {
    let Some(usage) = response.get("usage") else {
        return TokenUsageBreakdown::default();
    };

    TokenUsageBreakdown {
        input_tokens: get_u64(usage, &["input_tokens"]),
        output_tokens: get_u64(usage, &["output_tokens"]),
        cache_write_tokens: get_u64(usage, &["cache_creation_input_tokens"]),
        cache_read_tokens: get_u64(usage, &["cache_read_input_tokens"]),
    }
}

pub fn anthropic_usage_if_present(response: &Value) -> Option<TokenUsageBreakdown> {
    let usage = response.get("usage")?.as_object()?;
    let has_supported_field = [
        "input_tokens",
        "output_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
    ]
    .iter()
    .any(|field| usage.contains_key(*field));
    has_supported_field.then(|| anthropic_usage(response))
}

pub fn pricing_for_model(model: &str) -> ModelPricing {
    let normalized = model.to_ascii_lowercase();
    if normalized.contains("deepseek-v4-pro") {
        return ModelPricing {
            input_per_million: 0.435,
            output_per_million: 0.87,
            cache_write_per_million: 0.435,
            cache_read_per_million: 0.003625,
        };
    }
    if normalized.contains("deepseek-v4-flash")
        || normalized.contains("deepseek-chat")
        || normalized.contains("deepseek-reasoner")
    {
        return ModelPricing {
            input_per_million: 0.14,
            output_per_million: 0.28,
            cache_write_per_million: 0.14,
            cache_read_per_million: 0.0028,
        };
    }
    if normalized.contains("claude-fable-5") || normalized.contains("claude-mythos-5") {
        return ModelPricing {
            input_per_million: 10.0,
            output_per_million: 50.0,
            cache_write_per_million: 12.5,
            cache_read_per_million: 1.0,
        };
    }
    if normalized.contains("claude-opus-4") {
        return ModelPricing {
            input_per_million: 5.0,
            output_per_million: 25.0,
            cache_write_per_million: 6.25,
            cache_read_per_million: 0.5,
        };
    }
    if normalized.contains("claude-sonnet-4") {
        return ModelPricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cache_write_per_million: 3.75,
            cache_read_per_million: 0.3,
        };
    }
    if normalized.contains("claude-haiku-4-5") {
        return ModelPricing {
            input_per_million: 1.0,
            output_per_million: 5.0,
            cache_write_per_million: 1.25,
            cache_read_per_million: 0.1,
        };
    }
    if normalized.contains("claude-3-5-haiku") {
        return ModelPricing {
            input_per_million: 0.8,
            output_per_million: 4.0,
            cache_write_per_million: 1.0,
            cache_read_per_million: 0.08,
        };
    }
    if normalized.contains("gpt-5.5-pro") || normalized.contains("gpt-5.4-pro") {
        return ModelPricing {
            input_per_million: 15.0,
            output_per_million: 90.0,
            cache_write_per_million: 15.0,
            cache_read_per_million: 15.0,
        };
    }
    if normalized.contains("gpt-5.5") {
        return ModelPricing {
            input_per_million: 2.5,
            output_per_million: 15.0,
            cache_write_per_million: 2.5,
            cache_read_per_million: 0.25,
        };
    }
    if normalized.contains("gpt-5.4-mini") {
        return ModelPricing {
            input_per_million: 0.75,
            output_per_million: 4.5,
            cache_write_per_million: 0.9375,
            cache_read_per_million: 0.075,
        };
    }
    if normalized.contains("gpt-5.4-nano") {
        return ModelPricing {
            input_per_million: 0.20,
            output_per_million: 1.25,
            cache_write_per_million: 0.25,
            cache_read_per_million: 0.02,
        };
    }
    if normalized.contains("gpt-5.4") {
        return ModelPricing {
            input_per_million: 2.5,
            output_per_million: 15.0,
            cache_write_per_million: 3.125,
            cache_read_per_million: 0.25,
        };
    }
    if normalized.contains("mimo-") {
        return ModelPricing {
            input_per_million: 0.14,
            output_per_million: 0.28,
            cache_write_per_million: 0.0,
            cache_read_per_million: 0.0028,
        };
    }
    if normalized.contains("gpt-4o-mini") {
        return ModelPricing {
            input_per_million: 0.15,
            output_per_million: 0.60,
            cache_write_per_million: 0.15,
            cache_read_per_million: 0.075,
        };
    }
    if normalized.contains("gpt-4o") {
        return ModelPricing {
            input_per_million: 2.5,
            output_per_million: 10.0,
            cache_write_per_million: 2.5,
            cache_read_per_million: 1.25,
        };
    }
    if normalized.contains("gpt-") || normalized.contains("openai/") {
        return ModelPricing {
            input_per_million: 1.25,
            output_per_million: 7.5,
            cache_write_per_million: 1.25,
            cache_read_per_million: 0.125,
        };
    }
    if normalized.contains("gemini-") {
        return ModelPricing {
            input_per_million: 1.25,
            output_per_million: 10.0,
            cache_write_per_million: 1.25,
            cache_read_per_million: 0.125,
        };
    }
    if normalized.contains("qwen-") || normalized.contains("kimi-") || normalized.contains("glm-") {
        return ModelPricing {
            input_per_million: 0.6,
            output_per_million: 2.4,
            cache_write_per_million: 0.6,
            cache_read_per_million: 0.06,
        };
    }

    ModelPricing {
        input_per_million: 1.0,
        output_per_million: 4.0,
        cache_write_per_million: 1.0,
        cache_read_per_million: 0.1,
    }
}

pub fn cost_component(tokens: u64, price_per_million: f64) -> f64 {
    (tokens as f64 / 1_000_000.0) * price_per_million
}

fn get_u64(value: &Value, fields: &[&str]) -> u64 {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn get_nested_u64(value: &Value, paths: &[&[&str]]) -> u64 {
    for path in paths {
        let mut current = value;
        for segment in *path {
            let Some(next) = current.get(*segment) else {
                current = &Value::Null;
                break;
            };
            current = next;
        }
        if let Some(result) = current.as_u64() {
            return result;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn deepseek_cache_hit_tokens_are_discounted() {
        let usage = openai_usage(&json!({
            "usage": {
                "prompt_cache_hit_tokens": 1_000_000_u64,
                "prompt_cache_miss_tokens": 1_000_000_u64,
                "completion_tokens": 1_000_000_u64
            }
        }));
        let charge = charge_with_evidence("deepseek", "deepseek-v4-flash", usage, None, None, None);

        assert_eq!(charge.input_tokens, 1_000_000);
        assert_eq!(charge.cache_read_tokens, 1_000_000);
        assert!((charge.cost_estimate - 0.4228).abs() < 0.000001);
    }

    #[test]
    fn anthropic_cache_write_and_read_are_separate() {
        let usage = anthropic_usage(&json!({
            "usage": {
                "input_tokens": 1_000_000_u64,
                "cache_creation_input_tokens": 1_000_000_u64,
                "cache_read_input_tokens": 1_000_000_u64,
                "output_tokens": 1_000_000_u64
            }
        }));
        let charge = charge_with_evidence(
            "anthropic",
            "claude-sonnet-4-20250514",
            usage,
            None,
            None,
            None,
        );

        assert!((charge.cost_estimate - 22.05).abs() < 0.000001);
    }

    #[test]
    fn configured_pricing_overrides_the_model_family_default() {
        let usage = TokenUsageBreakdown {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_write_tokens: 0,
            cache_read_tokens: 0,
        };
        let pricing = ModelPricing {
            input_per_million: 0.0,
            output_per_million: 0.0,
            cache_write_per_million: 0.0,
            cache_read_per_million: 0.0,
        };

        let charge =
            charge_with_evidence("local", "qwen3.5-9b-q5km", usage, Some(pricing), None, None);

        assert_eq!(charge.cost_estimate, 0.0);
        assert_eq!(charge.actual_cost, None);
        assert_eq!(charge.billable_cost, None);
    }

    #[test]
    fn exact_rate_card_turns_provider_usage_into_billable_cost() {
        let usage = TokenUsageBreakdown {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cache_write_tokens: 0,
            cache_read_tokens: 0,
        };
        let card = ModelPricingCard {
            rates: ModelPricing {
                input_per_million: 1.0,
                output_per_million: 4.0,
                cache_write_per_million: 1.25,
                cache_read_per_million: 0.1,
            },
            version: "contract-v7".to_owned(),
            effective_at: "2026-08-01T00:00:00Z".to_owned(),
            currency: "USD".to_owned(),
            source: PricingSource::ProviderContract,
            service_tier: PricingServiceTier::Standard,
            region: Some("global".to_owned()),
            evidence: "contract://provider/7".to_owned(),
        };

        let charge =
            charge_with_evidence("provider", "exact-model", usage, None, Some(&card), None);

        assert_eq!(charge.cost_estimate, 3.0);
        assert_eq!(charge.actual_cost, Some(3.0));
        assert_eq!(charge.billable_cost, Some(3.0));
        assert_eq!(
            charge.pricing_evidence.as_ref().map(|value| &value.method),
            Some(&PricingMethod::ExactRateCard)
        );
        assert_eq!(
            charge
                .pricing_evidence
                .as_ref()
                .map(|value| value.version.as_str()),
            Some("contract-v7")
        );
    }

    #[test]
    fn trusted_provider_reported_cost_wins_over_rate_card_calculation() {
        let usage = TokenUsageBreakdown {
            input_tokens: 1_000_000,
            ..TokenUsageBreakdown::default()
        };
        let card = ModelPricingCard {
            rates: ModelPricing {
                input_per_million: 10.0,
                output_per_million: 0.0,
                cache_write_per_million: 0.0,
                cache_read_per_million: 0.0,
            },
            version: "public-v1".to_owned(),
            effective_at: "2026-08-01T00:00:00Z".to_owned(),
            currency: "USD".to_owned(),
            source: PricingSource::ProviderPublished,
            service_tier: PricingServiceTier::Standard,
            region: None,
            evidence: "https://provider.example/pricing".to_owned(),
        };

        let charge = charge_with_evidence(
            "openrouter",
            "routed-model",
            usage,
            None,
            Some(&card),
            Some(1.75),
        );

        assert_eq!(charge.cost_estimate, 10.0);
        assert_eq!(charge.actual_cost, Some(1.75));
        assert_eq!(charge.billable_cost, Some(1.75));
        assert_eq!(
            charge.pricing_evidence.as_ref().map(|value| &value.method),
            Some(&PricingMethod::ProviderReported)
        );
    }

    #[test]
    fn openai_nested_cache_write_usage_is_preserved() {
        let usage = openai_usage(&json!({
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 20,
                "prompt_tokens_details": {
                    "cached_tokens": 30,
                    "cache_write_tokens": 10
                }
            }
        }));

        assert_eq!(usage.input_tokens, 70);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.cache_write_tokens, 10);
        assert_eq!(usage.cache_read_tokens, 30);
    }

    #[test]
    fn gpt_4o_mini_does_not_inherit_full_gpt_4o_rates() {
        assert_eq!(
            pricing_for_model("gpt-4o-mini-2024-07-18"),
            ModelPricing {
                input_per_million: 0.15,
                output_per_million: 0.60,
                cache_write_per_million: 0.15,
                cache_read_per_million: 0.075,
            }
        );
    }

    #[test]
    fn rate_card_timestamp_requires_a_real_utc_calendar_value() {
        assert!(valid_utc_rate_card_timestamp("2026-08-16T12:34:56Z"));
        assert!(valid_utc_rate_card_timestamp("2024-02-29T12:34:56.123456Z"));
        assert!(!valid_utc_rate_card_timestamp("2026-02-29T12:34:56Z"));
        assert!(!valid_utc_rate_card_timestamp("2026-08-16 12:34:56Z"));
        assert!(!valid_utc_rate_card_timestamp("2026-08-16T12:34:56+08:00"));
    }

    #[test]
    fn usage_metadata_must_contain_supported_token_fields() {
        assert!(openai_usage_if_present(&json!({ "id": "response" })).is_none());
        assert!(openai_usage_if_present(&json!({ "usage": {} })).is_none());
        assert!(anthropic_usage_if_present(&json!({ "usage": null })).is_none());
        assert!(
            openai_usage_if_present(&json!({
                "usage": { "prompt_tokens": 0, "completion_tokens": 0 }
            }))
            .is_some()
        );
    }
}
