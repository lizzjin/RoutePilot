use crate::{config::validate_provider_base_url_for_request, error::AppError};

pub(crate) fn default_credential_status() -> String {
    "active".to_owned()
}

pub(crate) fn default_credential_pool_mode() -> String {
    "failover".to_owned()
}

pub(crate) fn validate_provider_credential_id(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 80 {
        return Err(AppError::InvalidRequest(
            "credential id must be 1-80 characters".to_owned(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(AppError::InvalidRequest(
            "credential id can only contain letters, numbers, '_' and '-'".to_owned(),
        ));
    }
    Ok(normalized)
}

pub(crate) fn validate_env_name(value: &str) -> Result<String, AppError> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > 120 {
        return Err(AppError::InvalidRequest(
            "apiKeyEnv must be 1-120 characters".to_owned(),
        ));
    }
    let mut chars = normalized.chars();
    let Some(first) = chars.next() else {
        return Err(AppError::InvalidRequest("apiKeyEnv is required".to_owned()));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(AppError::InvalidRequest(
            "apiKeyEnv must start with a letter or '_'".to_owned(),
        ));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(AppError::InvalidRequest(
            "apiKeyEnv can only contain letters, numbers and '_'".to_owned(),
        ));
    }
    Ok(normalized.to_owned())
}

pub(crate) fn validate_credential_status(value: &str) -> Result<String, AppError> {
    match value.trim() {
        "active" | "disabled" => Ok(value.trim().to_owned()),
        _ => Err(AppError::InvalidRequest(
            "credential status must be active or disabled".to_owned(),
        )),
    }
}

pub(crate) fn validate_credential_pool_mode(value: &str) -> Result<String, AppError> {
    match value.trim() {
        "manual" | "failover" | "round_robin" => Ok(value.trim().to_owned()),
        _ => Err(AppError::InvalidRequest(
            "credential pool mode must be manual, failover, or round_robin".to_owned(),
        )),
    }
}

pub(crate) fn validate_credential_base_url(
    provider_id: &str,
    value: Option<String>,
    allow_private_provider_urls: bool,
) -> Result<Option<String>, AppError> {
    let base_url = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let Some(base_url) = base_url else {
        return Ok(None);
    };

    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return Err(AppError::InvalidRequest(
            "baseUrl must start with http:// or https://".to_owned(),
        ));
    }
    validate_provider_base_url_for_request(provider_id, &base_url, allow_private_provider_urls)?;
    Ok(Some(base_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_id_is_normalized_and_restricted() {
        assert_eq!(
            validate_provider_credential_id(" Main_Key-01 ").unwrap(),
            "main_key-01"
        );
        assert!(validate_provider_credential_id("bad/key").is_err());
        assert!(validate_provider_credential_id("").is_err());
    }

    #[test]
    fn env_name_matches_shell_identifier_shape() {
        assert_eq!(
            validate_env_name(" DEEPSEEK_API_KEY ").unwrap(),
            "DEEPSEEK_API_KEY"
        );
        assert_eq!(validate_env_name("_TOKEN1").unwrap(), "_TOKEN1");
        assert!(validate_env_name("1TOKEN").is_err());
        assert!(validate_env_name("TOKEN-NAME").is_err());
    }

    #[test]
    fn status_and_pool_mode_are_explicit_enums() {
        assert_eq!(validate_credential_status("active").unwrap(), "active");
        assert_eq!(validate_credential_status("disabled").unwrap(), "disabled");
        assert!(validate_credential_status("paused").is_err());

        assert_eq!(
            validate_credential_pool_mode("failover").unwrap(),
            "failover"
        );
        assert_eq!(
            validate_credential_pool_mode("round_robin").unwrap(),
            "round_robin"
        );
        assert!(validate_credential_pool_mode("random").is_err());
    }

    #[test]
    fn base_url_is_trimmed_and_scheme_checked() {
        assert_eq!(
            validate_credential_base_url(
                "local-tool",
                Some(" http://127.0.0.1:8000/v1 ".to_owned()),
                true,
            )
            .unwrap()
            .as_deref(),
            Some("http://127.0.0.1:8000/v1")
        );
        assert!(
            validate_credential_base_url("deepseek", Some("ftp://example.com".to_owned()), false)
                .is_err()
        );
    }
}
