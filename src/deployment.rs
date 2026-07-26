use crate::{
    AppError, database,
    enterprise_ledger::EnterpriseLedger,
    oidc::OidcService,
    routes::{TrustedProxyConfig, validate_allowed_origins_from_env},
};

pub(crate) fn validate_environment() -> Result<(), AppError> {
    database::validate_configuration()?;
    EnterpriseLedger::validate_configuration()?;
    OidcService::validate_configuration()?;
    TrustedProxyConfig::from_env()?;
    validate_allowed_origins_from_env()?;
    validate_finalization_drain_timeout()?;
    if database::enterprise_mode_enabled()? {
        validate_enterprise_security(EnterpriseSecurityValues {
            cookie_secure: std::env::var("MODELPORT_ADMIN_COOKIE_SECURE").ok(),
            require_control_api_keys: std::env::var("MODELPORT_REQUIRE_CONTROL_API_KEYS").ok(),
            disable_csrf: std::env::var("MODELPORT_DISABLE_CSRF").ok(),
            allowed_origins: std::env::var("MODELPORT_ALLOWED_ORIGINS").ok(),
            trusted_proxies: std::env::var("MODELPORT_TRUSTED_PROXIES").ok(),
        })?;
    }
    Ok(())
}

struct EnterpriseSecurityValues {
    cookie_secure: Option<String>,
    require_control_api_keys: Option<String>,
    disable_csrf: Option<String>,
    allowed_origins: Option<String>,
    trusted_proxies: Option<String>,
}

fn validate_enterprise_security(values: EnterpriseSecurityValues) -> Result<(), AppError> {
    if !flag_enabled(values.cookie_secure.as_deref()) {
        return Err(AppError::Config(
            "MODELPORT_ENTERPRISE_MODE requires MODELPORT_ADMIN_COOKIE_SECURE=1".to_owned(),
        ));
    }
    if !flag_enabled(values.require_control_api_keys.as_deref()) {
        return Err(AppError::Config(
            "MODELPORT_ENTERPRISE_MODE requires MODELPORT_REQUIRE_CONTROL_API_KEYS=1".to_owned(),
        ));
    }
    if flag_enabled(values.disable_csrf.as_deref()) {
        return Err(AppError::Config(
            "MODELPORT_DISABLE_CSRF is forbidden when MODELPORT_ENTERPRISE_MODE=1".to_owned(),
        ));
    }
    let allowed_origins = values
        .allowed_origins
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Config(
                "MODELPORT_ENTERPRISE_MODE requires explicit MODELPORT_ALLOWED_ORIGINS".to_owned(),
            )
        })?;
    if allowed_origins
        .split(',')
        .map(str::trim)
        .any(|origin| !origin.starts_with("https://"))
    {
        return Err(AppError::Config(
            "MODELPORT_ENTERPRISE_MODE requires HTTPS-only MODELPORT_ALLOWED_ORIGINS".to_owned(),
        ));
    }
    if values
        .trusted_proxies
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        return Err(AppError::Config(
            "MODELPORT_ENTERPRISE_MODE requires explicit MODELPORT_TRUSTED_PROXIES".to_owned(),
        ));
    }
    Ok(())
}

fn validate_finalization_drain_timeout() -> Result<(), AppError> {
    let Some(value) = std::env::var("MODELPORT_FINALIZATION_DRAIN_TIMEOUT_SECONDS")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(());
    };
    let seconds = value.parse::<u64>().map_err(|_| {
        AppError::Config(
            "MODELPORT_FINALIZATION_DRAIN_TIMEOUT_SECONDS must be an integer".to_owned(),
        )
    })?;
    if !(1..=300).contains(&seconds) {
        return Err(AppError::Config(
            "MODELPORT_FINALIZATION_DRAIN_TIMEOUT_SECONDS must be between 1 and 300".to_owned(),
        ));
    }
    Ok(())
}

fn flag_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim(),
            "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secure_values() -> EnterpriseSecurityValues {
        EnterpriseSecurityValues {
            cookie_secure: Some("1".to_owned()),
            require_control_api_keys: Some("1".to_owned()),
            disable_csrf: None,
            allowed_origins: Some("https://modelport.example.com".to_owned()),
            trusted_proxies: Some("10.0.0.0/8".to_owned()),
        }
    }

    #[test]
    fn enterprise_security_requires_fail_closed_browser_and_client_auth() {
        assert!(validate_enterprise_security(secure_values()).is_ok());

        let mut values = secure_values();
        values.cookie_secure = None;
        assert!(validate_enterprise_security(values).is_err());

        let mut values = secure_values();
        values.require_control_api_keys = None;
        assert!(validate_enterprise_security(values).is_err());

        let mut values = secure_values();
        values.disable_csrf = Some("1".to_owned());
        assert!(validate_enterprise_security(values).is_err());

        let mut values = secure_values();
        values.allowed_origins = Some("http://modelport.example.com".to_owned());
        assert!(validate_enterprise_security(values).is_err());

        let mut values = secure_values();
        values.trusted_proxies = None;
        assert!(validate_enterprise_security(values).is_err());
    }
}
