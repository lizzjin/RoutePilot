use std::{collections::BTreeSet, net::IpAddr};

use crate::error::AppError;

pub(crate) fn normalize_policy_list(values: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.len() > 160 {
            return Err(AppError::InvalidRequest(
                "policy entries must be 160 characters or shorter".to_owned(),
            ));
        }
        if seen.insert(value.to_owned()) {
            output.push(value.to_owned());
        }
    }
    Ok(output)
}

pub(crate) fn policy_references_provider(allowed_providers: &[String], provider_id: &str) -> bool {
    allowed_providers
        .iter()
        .any(|rule| policy_value_matches(rule, provider_id))
}

pub(crate) fn normalize_ip_rules(values: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut seen = BTreeSet::new();
    let mut rules = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        validate_ip_rule(value)?;
        if seen.insert(value.to_owned()) {
            rules.push(value.to_owned());
        }
    }
    Ok(rules)
}

pub(crate) fn enforce_model_policy(
    label: &str,
    allowed_models: &[String],
    requested_model: &str,
    resolved_model: &str,
) -> Result<(), AppError> {
    if allowed_models.is_empty()
        || allowed_models.iter().any(|rule| {
            policy_value_matches(rule, requested_model)
                || policy_value_matches(rule, resolved_model)
        })
    {
        return Ok(());
    }
    Err(AppError::Forbidden(format!(
        "{label} does not allow model {requested_model}"
    )))
}

pub(crate) fn enforce_provider_policy(
    label: &str,
    allowed_providers: &[String],
    provider_id: &str,
) -> Result<(), AppError> {
    if allowed_providers.is_empty()
        || allowed_providers
            .iter()
            .any(|rule| policy_value_matches(rule, provider_id))
    {
        return Ok(());
    }
    Err(AppError::Forbidden(format!(
        "{label} does not allow provider {provider_id}"
    )))
}

pub(crate) fn enforce_ip_policy(
    ip_restricted: bool,
    allowed_ips: &[String],
    client_ip: Option<&str>,
) -> Result<(), AppError> {
    if !ip_restricted {
        return Ok(());
    }
    if allowed_ips.is_empty() {
        return Err(AppError::Forbidden(
            "API key IP restriction has no allowed IPs configured".to_owned(),
        ));
    }
    let Some(client_ip) = client_ip else {
        return Err(AppError::Forbidden(
            "client IP is required for this API key".to_owned(),
        ));
    };
    let ip = parse_client_ip(client_ip)
        .ok_or_else(|| AppError::Forbidden("client IP is invalid for this API key".to_owned()))?;
    if allowed_ips.iter().any(|rule| ip_rule_matches(rule, ip)) {
        return Ok(());
    }

    Err(AppError::Forbidden(format!(
        "client IP {ip} is not allowed for this API key"
    )))
}

pub(crate) fn enforce_spend_limit(
    label: &str,
    limit: f64,
    used: f64,
    incoming: f64,
) -> Result<(), AppError> {
    if limit > 0.0 && used + incoming > limit {
        return Err(AppError::QuotaExceeded(format!(
            "API key {label} limit exceeded ({:.4} / {:.4} USD)",
            used + incoming,
            limit
        )));
    }
    Ok(())
}

fn validate_ip_rule(value: &str) -> Result<(), AppError> {
    if value.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    let Some((addr, prefix)) = value.split_once('/') else {
        return Err(AppError::InvalidRequest(format!(
            "invalid IP allowlist entry: {value}"
        )));
    };
    let addr = addr
        .parse::<IpAddr>()
        .map_err(|_| AppError::InvalidRequest(format!("invalid IP allowlist entry: {value}")))?;
    let prefix = prefix
        .parse::<u8>()
        .map_err(|_| AppError::InvalidRequest(format!("invalid IP allowlist entry: {value}")))?;
    let max_prefix = match addr {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if prefix > max_prefix {
        return Err(AppError::InvalidRequest(format!(
            "invalid IP allowlist entry: {value}"
        )));
    }
    Ok(())
}

fn policy_value_matches(rule: &str, value: &str) -> bool {
    let rule = rule.trim();
    if rule == "*" {
        return true;
    }
    if let Some(prefix) = rule.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    rule == value
}

fn parse_client_ip(value: &str) -> Option<IpAddr> {
    let value = value.trim();
    if let Ok(ip) = value.parse::<IpAddr>() {
        return Some(ip);
    }
    value
        .rsplit_once(':')
        .and_then(|(host, _)| host.parse::<IpAddr>().ok())
}

fn ip_rule_matches(rule: &str, ip: IpAddr) -> bool {
    if let Ok(exact) = rule.parse::<IpAddr>() {
        return exact == ip;
    }
    let Some((base, prefix)) = rule.split_once('/') else {
        return false;
    };
    let Ok(base) = base.parse::<IpAddr>() else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u8>() else {
        return false;
    };
    match (base, ip) {
        (IpAddr::V4(base), IpAddr::V4(ip)) if prefix <= 32 => {
            cidr_matches(u32::from(base).into(), u32::from(ip).into(), prefix, 32)
        }
        (IpAddr::V6(base), IpAddr::V6(ip)) if prefix <= 128 => {
            cidr_matches(u128::from(base), u128::from(ip), prefix, 128)
        }
        _ => false,
    }
}

fn cidr_matches(base: u128, ip: u128, prefix: u8, bits: u8) -> bool {
    if prefix == 0 {
        return true;
    }
    let shift = u32::from(bits - prefix);
    (base >> shift) == (ip >> shift)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn policy_list_trims_deduplicates_and_keeps_order() {
        assert_eq!(
            normalize_policy_list(vec![
                " deepseek-* ".to_owned(),
                "".to_owned(),
                "mimo".to_owned(),
                "deepseek-*".to_owned(),
            ])
            .unwrap(),
            vec!["deepseek-*", "mimo"]
        );
    }

    #[test]
    fn model_and_provider_policies_support_wildcards() {
        let allowed_models = vec!["deepseek-*".to_owned()];
        assert!(
            enforce_model_policy("API key", &allowed_models, "deepseek-v4-flash", "alias").is_ok()
        );
        assert!(
            enforce_model_policy("API key", &allowed_models, "alias", "deepseek-v4-flash").is_ok()
        );
        assert!(enforce_model_policy("API key", &allowed_models, "claude", "claude").is_err());

        let allowed_providers = vec!["deep*".to_owned()];
        assert!(enforce_provider_policy("team", &allowed_providers, "deepseek").is_ok());
        assert!(policy_references_provider(&allowed_providers, "deepseek"));
        assert!(enforce_provider_policy("team", &allowed_providers, "mimo").is_err());
    }

    #[test]
    fn ip_rules_validate_and_match_exact_and_cidr() {
        let rules = normalize_ip_rules(vec![
            " 127.0.0.1 ".to_owned(),
            "10.0.0.0/8".to_owned(),
            "::1".to_owned(),
        ])
        .unwrap();

        assert_eq!(rules, vec!["127.0.0.1", "10.0.0.0/8", "::1"]);
        assert!(ip_rule_matches(
            "10.0.0.0/8",
            IpAddr::V4(Ipv4Addr::new(10, 2, 3, 4))
        ));
        assert!(ip_rule_matches("::1", IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(normalize_ip_rules(vec!["10.0.0.0/33".to_owned()]).is_err());
    }

    #[test]
    fn ip_policy_accepts_forwarded_host_port_shape() {
        let rules = vec!["127.0.0.1".to_owned()];
        assert!(enforce_ip_policy(true, &rules, Some("127.0.0.1:43210")).is_ok());
        assert!(enforce_ip_policy(true, &rules, Some("127.0.0.2")).is_err());
        assert!(enforce_ip_policy(true, &[], Some("127.0.0.1")).is_err());
        assert!(enforce_ip_policy(true, &rules, None).is_err());
        assert!(enforce_ip_policy(false, &[], None).is_ok());
    }

    #[test]
    fn spend_limit_only_rejects_positive_exceeded_limits() {
        assert!(enforce_spend_limit("daily spend", 0.0, 100.0, 100.0).is_ok());
        assert!(enforce_spend_limit("daily spend", 10.0, 9.0, 1.0).is_ok());
        assert!(enforce_spend_limit("daily spend", 10.0, 9.0, 1.1).is_err());
    }
}
