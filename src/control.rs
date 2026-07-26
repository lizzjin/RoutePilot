#[cfg(test)]
use std::path::PathBuf;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::http::HeaderMap;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    config::{ProviderConfig, ToolUseConfig},
    control_view::{
        ApiKeyViewRecord, ProviderCredentialHealthViewRecord, ProviderCredentialViewRecord,
        ProviderHealthViewRecord, QuotaViewRecord, TeamViewRecord, provider_credential_health_row,
        provider_health_row, public_api_key, public_quota, public_team,
    },
    domain::{TenantScope, valid_tenant_identifier},
    error::{AppError, audit_safe_persisted_error},
    policy::{
        enforce_ip_policy, enforce_model_policy, enforce_provider_policy, normalize_ip_rules,
        normalize_policy_list, policy_references_provider,
    },
    pricing,
    provider_credentials::{
        default_credential_pool_mode, validate_credential_base_url, validate_credential_pool_mode,
        validate_credential_status, validate_env_name, validate_provider_credential_id,
    },
    provider_status::{
        cooldown_seconds, credential_cooldown_seconds, provider_failure_guidance,
        should_rotate_provider_credential,
    },
    storage::JsonStore,
    usage::current_period,
};

pub use crate::usage::UsageEstimate;

fn default_organization_id() -> String {
    "org_local".to_owned()
}

fn default_project_id() -> String {
    "prj_default".to_owned()
}

fn default_environment_id() -> String {
    "env_default".to_owned()
}

fn normalized_tenant_id(field: &str, value: String) -> Result<String, AppError> {
    let value = value.trim();
    if !valid_tenant_identifier(value) {
        return Err(AppError::InvalidRequest(format!(
            "API key {field} must contain 1-128 safe scope characters"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_tenant_scope(
    organization_id: Option<String>,
    project_id: Option<String>,
    environment_id: Option<String>,
) -> Result<(String, String, String), AppError> {
    match (organization_id, project_id, environment_id) {
        (None, None, None) => Ok((
            default_organization_id(),
            default_project_id(),
            default_environment_id(),
        )),
        (Some(organization_id), Some(project_id), Some(environment_id)) => Ok((
            normalized_tenant_id("organizationId", organization_id)?,
            normalized_tenant_id("projectId", project_id)?,
            normalized_tenant_id("environmentId", environment_id)?,
        )),
        _ => Err(AppError::InvalidRequest(
            "API key organizationId, projectId, and environmentId must be supplied together"
                .to_owned(),
        )),
    }
}

pub(crate) fn validate_backup_document(value: &serde_json::Value) -> Result<(), AppError> {
    serde_json::from_value::<ControlFile>(value.clone())
        .map(|_| ())
        .map_err(|error| {
            AppError::InvalidRequest(format!("backup control document is invalid: {error}"))
        })
}

pub struct ControlStore {
    store: Option<JsonStore>,
    inner: Mutex<ControlInner>,
    persistence_degraded: AtomicBool,
}

impl std::fmt::Debug for ControlStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ControlStore")
            .field("data_path", &self.data_path())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default)]
struct ControlInner {
    teams: BTreeMap<String, TeamRecord>,
    api_keys: BTreeMap<String, ApiKeyRecord>,
    api_key_hash_index: HashMap<String, String>,
    quotas: BTreeMap<String, QuotaRecord>,
    route_config: RouteConfigRecord,
    provider_tests: BTreeMap<String, ProviderTestRecord>,
    provider_health: BTreeMap<String, ProviderHealthRecord>,
    provider_overrides: BTreeMap<String, ProviderOverrideRecord>,
    disabled_providers: BTreeSet<String>,
    deleted_providers: BTreeSet<String>,
    provider_model_overrides: BTreeMap<String, BTreeMap<String, ProviderModelOverrideRecord>>,
    provider_credentials: BTreeMap<String, BTreeMap<String, ProviderCredentialRecord>>,
    active_provider_credentials: BTreeMap<String, String>,
    provider_credential_pool_modes: BTreeMap<String, String>,
    provider_credential_health: BTreeMap<String, BTreeMap<String, ProviderCredentialHealthRecord>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ControlFile {
    #[serde(default)]
    teams: Vec<TeamRecord>,
    #[serde(default)]
    api_keys: Vec<ApiKeyRecord>,
    #[serde(default)]
    quotas: Vec<QuotaRecord>,
    #[serde(default)]
    route_config: RouteConfigRecord,
    #[serde(default)]
    provider_tests: Vec<ProviderTestRecord>,
    #[serde(default)]
    provider_health: Vec<ProviderHealthRecord>,
    #[serde(default)]
    provider_overrides: Vec<ProviderOverrideRecord>,
    #[serde(default)]
    disabled_providers: BTreeSet<String>,
    #[serde(default)]
    deleted_providers: BTreeSet<String>,
    #[serde(default)]
    provider_model_overrides: Vec<ProviderModelOverrideRecord>,
    #[serde(default)]
    provider_credentials: Vec<ProviderCredentialRecord>,
    #[serde(default)]
    active_provider_credentials: BTreeMap<String, String>,
    #[serde(default)]
    provider_credential_pool_modes: BTreeMap<String, String>,
    #[serde(default)]
    provider_credential_health: Vec<ProviderCredentialHealthRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RouteConfigRecord {
    #[serde(default)]
    aliases: BTreeMap<String, String>,
    #[serde(default)]
    deleted_aliases: BTreeSet<String>,
    default_provider: Option<String>,
    provider_order: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderTestRecord {
    provider_id: String,
    tested_at_ms: u64,
    success: bool,
    message: String,
    #[serde(default)]
    discovered_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOverrideRecord {
    pub id: String,
    pub display_name: String,
    pub protocol: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    #[serde(default = "default_true")]
    pub api_key_required: bool,
    pub default_model: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub model_prefixes: Vec<String>,
    #[serde(default)]
    pub passthrough_unknown_models: bool,
    #[serde(default = "default_max_tokens_field")]
    pub max_tokens_field: String,
    #[serde(default)]
    pub deduplicate_stream_text: bool,
    #[serde(default)]
    pub buffer_stream_text: bool,
    #[serde(default = "default_fidelity_mode")]
    pub fidelity_mode: String,
    #[serde(default)]
    pub tool_use: ToolUseConfig,
    #[serde(default)]
    pub pricing: Option<pricing::ModelPricing>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelOverrideRecord {
    pub provider_id: String,
    pub model: String,
    #[serde(default = "default_model_status")]
    pub status: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub context_window: Option<u64>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialRecord {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub api_key_env: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "crate::provider_credentials::default_credential_status")]
    pub status: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl ProviderCredentialViewRecord for ProviderCredentialRecord {
    fn id(&self) -> &str {
        &self.id
    }

    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn api_key_env(&self) -> &str {
        &self.api_key_env
    }

    fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    fn status(&self) -> &str {
        &self.status
    }

    fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    fn updated_at_ms(&self) -> u64 {
        self.updated_at_ms
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialHealthRecord {
    pub provider_id: String,
    pub credential_id: String,
    #[serde(default)]
    pub requests_total: u64,
    #[serde(default)]
    pub successes_total: u64,
    #[serde(default)]
    pub failures_total: u64,
    #[serde(default)]
    pub consecutive_failures: u32,
    #[serde(default)]
    pub last_success_at_ms: Option<u64>,
    #[serde(default)]
    pub last_failure_at_ms: Option<u64>,
    #[serde(default)]
    pub last_used_at_ms: Option<u64>,
    #[serde(default)]
    pub cooldown_until_ms: Option<u64>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_status_code: Option<u16>,
}

impl ProviderHealthViewRecord for ProviderCredentialHealthRecord {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn requests_total(&self) -> u64 {
        self.requests_total
    }

    fn successes_total(&self) -> u64 {
        self.successes_total
    }

    fn failures_total(&self) -> u64 {
        self.failures_total
    }

    fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    fn last_success_at_ms(&self) -> Option<u64> {
        self.last_success_at_ms
    }

    fn last_failure_at_ms(&self) -> Option<u64> {
        self.last_failure_at_ms
    }

    fn cooldown_until_ms(&self) -> Option<u64> {
        self.cooldown_until_ms
    }

    fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn last_status_code(&self) -> Option<u16> {
        self.last_status_code
    }
}

impl ProviderCredentialHealthViewRecord for ProviderCredentialHealthRecord {
    fn credential_id(&self) -> &str {
        &self.credential_id
    }

    fn last_used_at_ms(&self) -> Option<u64> {
        self.last_used_at_ms
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamRecord {
    id: String,
    name: String,
    slug: String,
    description: Option<String>,
    status: String,
    #[serde(default)]
    daily_limit_usd: f64,
    #[serde(default)]
    monthly_limit_usd: f64,
    #[serde(default)]
    allowed_models: Vec<String>,
    #[serde(default)]
    allowed_providers: Vec<String>,
    created_at_ms: u64,
    updated_at_ms: u64,
}

impl TeamViewRecord for TeamRecord {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn slug(&self) -> &str {
        &self.slug
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn status(&self) -> &str {
        &self.status
    }

    fn daily_limit_usd(&self) -> f64 {
        self.daily_limit_usd
    }

    fn monthly_limit_usd(&self) -> f64 {
        self.monthly_limit_usd
    }

    fn allowed_models(&self) -> &[String] {
        &self.allowed_models
    }

    fn allowed_providers(&self) -> &[String] {
        &self.allowed_providers
    }

    fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    fn updated_at_ms(&self) -> u64 {
        self.updated_at_ms
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderHealthRecord {
    provider_id: String,
    #[serde(default)]
    requests_total: u64,
    #[serde(default)]
    successes_total: u64,
    #[serde(default)]
    failures_total: u64,
    #[serde(default)]
    consecutive_failures: u32,
    #[serde(default)]
    last_success_at_ms: Option<u64>,
    #[serde(default)]
    last_failure_at_ms: Option<u64>,
    #[serde(default)]
    cooldown_until_ms: Option<u64>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    last_status_code: Option<u16>,
}

impl ProviderHealthViewRecord for ProviderHealthRecord {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn requests_total(&self) -> u64 {
        self.requests_total
    }

    fn successes_total(&self) -> u64 {
        self.successes_total
    }

    fn failures_total(&self) -> u64 {
        self.failures_total
    }

    fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    fn last_success_at_ms(&self) -> Option<u64> {
        self.last_success_at_ms
    }

    fn last_failure_at_ms(&self) -> Option<u64> {
        self.last_failure_at_ms
    }

    fn cooldown_until_ms(&self) -> Option<u64> {
        self.cooldown_until_ms
    }

    fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn last_status_code(&self) -> Option<u16> {
        self.last_status_code
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertTeamInput {
    pub id: Option<String>,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub daily_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
    pub allowed_models: Option<Vec<String>>,
    pub allowed_providers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyRecord {
    id: String,
    user_id: String,
    username: String,
    name: String,
    key_hash: String,
    key_prefix: String,
    key_preview: String,
    group: Option<String>,
    #[serde(default)]
    team_id: Option<String>,
    #[serde(default)]
    team_name: Option<String>,
    #[serde(default)]
    allowed_models: Vec<String>,
    #[serde(default)]
    allowed_providers: Vec<String>,
    #[serde(default = "default_organization_id")]
    organization_id: String,
    #[serde(default = "default_project_id")]
    project_id: String,
    #[serde(default = "default_environment_id")]
    environment_id: String,
    created_at_ms: u64,
    last_used_at_ms: Option<u64>,
    expires_at_ms: Option<u64>,
    status: String,
    #[serde(default)]
    ip_restricted: bool,
    #[serde(default)]
    allowed_ips: Vec<String>,
    #[serde(default)]
    spend_limit_usd: f64,
    #[serde(default)]
    rate_limited: bool,
    #[serde(default)]
    five_hour_limit_usd: f64,
    #[serde(default)]
    daily_limit_usd: f64,
    #[serde(default)]
    weekly_limit_usd: f64,
    #[serde(default)]
    monthly_limit_usd: f64,
}

impl ApiKeyViewRecord for ApiKeyRecord {
    fn id(&self) -> &str {
        &self.id
    }

    fn user_id(&self) -> &str {
        &self.user_id
    }

    fn username(&self) -> &str {
        &self.username
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn key_prefix(&self) -> &str {
        &self.key_prefix
    }

    fn key_preview(&self) -> &str {
        &self.key_preview
    }

    fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    fn team_id(&self) -> Option<&str> {
        self.team_id.as_deref()
    }

    fn team_name(&self) -> Option<&str> {
        self.team_name.as_deref()
    }

    fn allowed_models(&self) -> &[String] {
        &self.allowed_models
    }

    fn allowed_providers(&self) -> &[String] {
        &self.allowed_providers
    }

    fn organization_id(&self) -> &str {
        &self.organization_id
    }

    fn project_id(&self) -> &str {
        &self.project_id
    }

    fn environment_id(&self) -> &str {
        &self.environment_id
    }

    fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    fn last_used_at_ms(&self) -> Option<u64> {
        self.last_used_at_ms
    }

    fn expires_at_ms(&self) -> Option<u64> {
        self.expires_at_ms
    }

    fn status(&self) -> &str {
        &self.status
    }

    fn ip_restricted(&self) -> bool {
        self.ip_restricted
    }

    fn allowed_ips(&self) -> &[String] {
        &self.allowed_ips
    }

    fn spend_limit_usd(&self) -> f64 {
        self.spend_limit_usd
    }

    fn rate_limited(&self) -> bool {
        self.rate_limited
    }

    fn five_hour_limit_usd(&self) -> f64 {
        self.five_hour_limit_usd
    }

    fn daily_limit_usd(&self) -> f64 {
        self.daily_limit_usd
    }

    fn weekly_limit_usd(&self) -> f64 {
        self.weekly_limit_usd
    }

    fn monthly_limit_usd(&self) -> f64 {
        self.monthly_limit_usd
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaRecord {
    id: String,
    user_id: String,
    username: String,
    quota_type: String,
    limit: f64,
    period: String,
    period_start_ms: u64,
    period_end_ms: u64,
    reset_at_ms: u64,
}

impl QuotaViewRecord for QuotaRecord {
    fn id(&self) -> &str {
        &self.id
    }

    fn user_id(&self) -> &str {
        &self.user_id
    }

    fn username(&self) -> &str {
        &self.username
    }

    fn quota_type(&self) -> &str {
        &self.quota_type
    }

    fn limit(&self) -> f64 {
        self.limit
    }

    fn period(&self) -> &str {
        &self.period
    }

    fn period_start_ms(&self) -> u64 {
        self.period_start_ms
    }

    fn period_end_ms(&self) -> u64 {
        self.period_end_ms
    }

    fn reset_at_ms(&self) -> u64 {
        self.reset_at_ms
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicApiKey {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub name: String,
    pub key_prefix: String,
    pub key_preview: String,
    pub group: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
    pub allowed_models: Vec<String>,
    pub allowed_providers: Vec<String>,
    pub organization_id: String,
    pub project_id: String,
    pub environment_id: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub status: String,
    pub requests_today: u64,
    pub tokens_today: u64,
    pub ip_restricted: bool,
    pub allowed_ips: Vec<String>,
    pub spend_limit_usd: f64,
    pub rate_limited: bool,
    pub five_hour_limit_usd: f64,
    pub daily_limit_usd: f64,
    pub weekly_limit_usd: f64,
    pub monthly_limit_usd: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedApiKey {
    #[serde(flatten)]
    pub public: PublicApiKey,
    pub key: String,
}

impl std::fmt::Debug for CreatedApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CreatedApiKey")
            .field("public", &self.public)
            .field("key", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyInput {
    pub user_id: String,
    pub username: Option<String>,
    pub name: String,
    pub group: Option<String>,
    pub team_id: Option<String>,
    pub allowed_models: Option<Vec<String>>,
    pub allowed_providers: Option<Vec<String>>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiKeyInput {
    pub name: Option<String>,
    pub group: Option<String>,
    pub team_id: Option<String>,
    pub allowed_models: Option<Vec<String>>,
    pub allowed_providers: Option<Vec<String>>,
    pub expires_at: Option<String>,
    pub status: Option<String>,
    pub ip_restricted: Option<bool>,
    pub allowed_ips: Option<Vec<String>>,
    pub spend_limit_usd: Option<f64>,
    pub rate_limited: Option<bool>,
    pub five_hour_limit_usd: Option<f64>,
    pub daily_limit_usd: Option<f64>,
    pub weekly_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindApiKeyScopeInput {
    pub organization_id: String,
    pub project_id: String,
    pub environment_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertQuotaInput {
    pub id: Option<String>,
    pub user_id: String,
    pub username: String,
    pub quota_type: String,
    pub limit: f64,
    pub period: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicQuota {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub quota_type: String,
    pub limit: f64,
    pub used: f64,
    pub period: String,
    pub period_start: String,
    pub period_end: String,
    pub reset_at: String,
}

#[derive(Debug, Clone)]
pub struct ClientIdentity {
    pub user_id: String,
    pub username: String,
    pub api_key_id: Option<String>,
    pub api_key_name: Option<String>,
    pub api_key_group: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
    pub enforce_quotas: bool,
    pub api_key_policy: ApiKeyPolicy,
}

#[derive(Debug, Clone, Default)]
pub struct ApiKeyPolicy {
    pub team_id: Option<String>,
    pub ip_restricted: bool,
    pub allowed_ips: Vec<String>,
    pub allowed_models: Vec<String>,
    pub allowed_providers: Vec<String>,
    pub team_allowed_models: Vec<String>,
    pub team_allowed_providers: Vec<String>,
    pub team_daily_limit_usd: f64,
    pub team_monthly_limit_usd: f64,
    pub spend_limit_usd: f64,
    pub rate_limited: bool,
    pub five_hour_limit_usd: f64,
    pub daily_limit_usd: f64,
    pub weekly_limit_usd: f64,
    pub monthly_limit_usd: f64,
}

#[derive(Debug, Clone)]
pub struct UsageEventInput {
    pub request_id: Option<String>,
    pub attempt_id: Option<String>,
    pub resolved_model: String,
    pub provider: String,
    pub protocol: String,
    /// Whether the request declares tools, selects a tool, or continues an
    /// existing tool call/result exchange. No tool arguments are persisted.
    pub tool_use_requested: bool,
    /// Aggregate-only Tool Use outcome. Never contains tool names or arguments.
    pub tool_outcome: String,
    /// Caller-supplied bounded traffic classification used to keep synthetic
    /// acceptance traffic out of business SLOs.
    pub traffic_class: String,
    pub tool_repair_attempted: bool,
    pub tool_repair_recovered: bool,
    pub success: bool,
    pub timed_out: bool,
    pub status_code: u16,
    pub terminal_reason: String,
    pub estimate: UsageEstimate,
    pub model_pricing: Option<pricing::ModelPricing>,
    pub billing_mode: String,
    /// Whether this request reached an upstream provider and can therefore
    /// consume quota or spend. Locally rejected requests are still logged,
    /// but must never move billing counters.
    pub chargeable: bool,
    pub latency: Duration,
    pub first_byte_latency: Option<Duration>,
    pub retry_count: u32,
    pub fallback_from_provider: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct UsageQuotaLimit {
    pub(crate) id: String,
    pub(crate) user_id: String,
    pub(crate) quota_type: String,
    pub(crate) limit: f64,
    pub(crate) period_start_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UsagePolicySnapshot {
    pub(crate) user_id: String,
    pub(crate) username: String,
    pub(crate) api_key_id: Option<String>,
    pub(crate) team_id: Option<String>,
    pub(crate) api_key_policy: ApiKeyPolicy,
    pub(crate) quotas: Vec<UsageQuotaLimit>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub total_requests: u64,
    pub total_successes: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_write_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cost_estimate: f64,
    pub api_keys_total: u64,
    pub api_keys_active: u64,
    pub average_latency_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderUsageStats {
    pub requests_total: u64,
    pub successes_total: u64,
    pub duration_ms_total: u64,
    pub input_tokens_total: u64,
    pub output_tokens_total: u64,
    pub cache_write_tokens_total: u64,
    pub cache_read_tokens_total: u64,
    pub cost_estimate_usd_total: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProviderRoutingSignal {
    pub(crate) requests_total: u64,
    pub(crate) successes_total: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RoutingConfigSnapshot {
    pub default_provider: Option<String>,
    pub provider_order: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderControlSnapshot {
    pub provider_overrides: BTreeMap<String, ProviderOverrideRecord>,
    pub disabled_providers: BTreeSet<String>,
    pub deleted_providers: BTreeSet<String>,
    pub provider_model_overrides: BTreeMap<String, BTreeMap<String, ProviderModelOverrideRecord>>,
    pub provider_credentials: BTreeMap<String, BTreeMap<String, ProviderCredentialRecord>>,
    pub active_provider_credentials: BTreeMap<String, String>,
    pub provider_credential_pool_modes: BTreeMap<String, String>,
}

impl ControlStore {
    pub fn load() -> Result<Self, AppError> {
        let store = JsonStore::open("control")?;
        let mut file: ControlFile = store.read_or_default(json!({
            "teams": [],
            "apiKeys": [],
            "quotas": [],
            "routeConfig": {},
            "providerTests": [],
            "providerHealth": [],
            "providerOverrides": [],
            "disabledProviders": [],
            "deletedProviders": [],
            "providerModelOverrides": [],
            "providerCredentials": [],
            "activeProviderCredentials": {},
            "providerCredentialPoolModes": {},
            "providerCredentialHealth": [],
        }))?;
        if redact_historical_control_errors(&mut file) {
            store.write_json(&file)?;
        }
        let mut provider_model_overrides: BTreeMap<
            String,
            BTreeMap<String, ProviderModelOverrideRecord>,
        > = BTreeMap::new();
        for record in file.provider_model_overrides {
            provider_model_overrides
                .entry(record.provider_id.clone())
                .or_default()
                .insert(record.model.clone(), record);
        }
        let mut provider_credentials: BTreeMap<String, BTreeMap<String, ProviderCredentialRecord>> =
            BTreeMap::new();
        for record in file.provider_credentials {
            provider_credentials
                .entry(record.provider_id.clone())
                .or_default()
                .insert(record.id.clone(), record);
        }
        let mut provider_credential_health: BTreeMap<
            String,
            BTreeMap<String, ProviderCredentialHealthRecord>,
        > = BTreeMap::new();
        for record in file.provider_credential_health {
            provider_credential_health
                .entry(record.provider_id.clone())
                .or_default()
                .insert(record.credential_id.clone(), record);
        }
        let api_keys = file
            .api_keys
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let api_key_hash_index = api_keys
            .iter()
            .map(|(id, record)| (record.key_hash.clone(), id.clone()))
            .collect();
        Ok(Self {
            store: Some(store),
            inner: Mutex::new(ControlInner {
                teams: file
                    .teams
                    .into_iter()
                    .map(|record| (record.id.clone(), record))
                    .collect(),
                api_keys,
                api_key_hash_index,
                quotas: file
                    .quotas
                    .into_iter()
                    .map(|record| (record.id.clone(), record))
                    .collect(),
                route_config: file.route_config,
                provider_tests: file
                    .provider_tests
                    .into_iter()
                    .map(|record| (record.provider_id.clone(), record))
                    .collect(),
                provider_health: file
                    .provider_health
                    .into_iter()
                    .map(|record| (record.provider_id.clone(), record))
                    .collect(),
                provider_overrides: file
                    .provider_overrides
                    .into_iter()
                    .map(|record| (record.id.clone(), record))
                    .collect(),
                disabled_providers: file.disabled_providers,
                deleted_providers: file.deleted_providers,
                provider_model_overrides,
                provider_credentials,
                active_provider_credentials: file.active_provider_credentials,
                provider_credential_pool_modes: file.provider_credential_pool_modes,
                provider_credential_health,
            }),
            persistence_degraded: AtomicBool::new(false),
        })
    }

    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            store: None,
            inner: Mutex::new(ControlInner::default()),
            persistence_degraded: AtomicBool::new(false),
        }
    }

    pub fn routing_config(&self) -> RoutingConfigSnapshot {
        let inner = self.inner.lock().expect("control lock poisoned");
        RoutingConfigSnapshot {
            default_provider: inner.route_config.default_provider.clone(),
            provider_order: inner.route_config.provider_order.clone(),
        }
    }

    pub fn provider_control_snapshot(&self) -> ProviderControlSnapshot {
        let inner = self.inner.lock().expect("control lock poisoned");
        ProviderControlSnapshot {
            provider_overrides: inner.provider_overrides.clone(),
            disabled_providers: inner.disabled_providers.clone(),
            deleted_providers: inner.deleted_providers.clone(),
            provider_model_overrides: inner.provider_model_overrides.clone(),
            provider_credentials: inner.provider_credentials.clone(),
            active_provider_credentials: inner.active_provider_credentials.clone(),
            provider_credential_pool_modes: inner.provider_credential_pool_modes.clone(),
        }
    }

    pub fn effective_aliases(
        &self,
        base_aliases: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let inner = self.inner.lock().expect("control lock poisoned");
        effective_aliases_locked(base_aliases, &inner.route_config)
    }

    pub fn upsert_alias(&self, alias: String, target: String) -> Result<(), AppError> {
        let alias = alias.trim();
        let target = target.trim();
        if alias.is_empty() || alias.len() > 120 {
            return Err(AppError::InvalidRequest(
                "alias must be 1-120 characters".to_owned(),
            ));
        }
        if alias.contains(':') {
            return Err(AppError::InvalidRequest(
                "alias cannot contain provider selector ':'".to_owned(),
            ));
        }
        if target.is_empty() || target.len() > 240 {
            return Err(AppError::InvalidRequest(
                "alias target must be 1-240 characters".to_owned(),
            ));
        }

        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        inner
            .route_config
            .aliases
            .insert(alias.to_owned(), target.to_owned());
        inner.route_config.deleted_aliases.remove(alias);
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn delete_alias(&self, alias: &str, tombstone: bool) -> Result<(), AppError> {
        let alias = alias.trim();
        if alias.is_empty() {
            return Err(AppError::InvalidRequest("alias is required".to_owned()));
        }

        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        inner.route_config.aliases.remove(alias);
        if tombstone {
            inner.route_config.deleted_aliases.insert(alias.to_owned());
        } else {
            inner.route_config.deleted_aliases.remove(alias);
        }
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn set_default_provider(&self, provider_id: String) -> Result<(), AppError> {
        let provider_id = provider_id.trim();
        if provider_id.is_empty() {
            return Err(AppError::InvalidRequest(
                "default provider is required".to_owned(),
            ));
        }
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        inner.route_config.default_provider = Some(provider_id.to_owned());
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn set_provider_order(&self, provider_order: Vec<String>) -> Result<(), AppError> {
        if provider_order.is_empty() {
            return Err(AppError::InvalidRequest(
                "provider order cannot be empty".to_owned(),
            ));
        }
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        inner.route_config.provider_order = Some(provider_order);
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn upsert_provider_override(
        &self,
        mut record: ProviderOverrideRecord,
    ) -> Result<ProviderOverrideRecord, AppError> {
        let id = validate_provider_id(&record.id)?;
        record.id = id.clone();
        record.display_name = validate_non_empty("displayName", &record.display_name, 120)?;
        record.base_url = validate_non_empty("baseUrl", &record.base_url, 512)?;
        crate::config::validate_provider_base_url_for_request(
            &id,
            &record.base_url,
            env_flag("MODELPORT_ALLOW_PRIVATE_PROVIDER_URLS"),
        )?;
        record.default_model = validate_non_empty("defaultModel", &record.default_model, 240)?;
        record.models = normalize_policy_list(record.models)?;
        if !record.models.contains(&record.default_model) {
            record.models.insert(0, record.default_model.clone());
        }
        record.model_prefixes = normalize_policy_list(record.model_prefixes)?;
        record.api_key_env = record
            .api_key_env
            .map(|value| validate_env_name(&value))
            .transpose()?;
        let now = now_millis();

        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        let created_at_ms = inner
            .provider_overrides
            .get(&id)
            .map(|existing| existing.created_at_ms)
            .unwrap_or(now);
        record.created_at_ms = created_at_ms;
        record.updated_at_ms = now;
        inner.provider_overrides.insert(id.clone(), record.clone());
        inner.deleted_providers.remove(&id);
        if let Some(order) = &mut inner.route_config.provider_order
            && !order.contains(&id)
        {
            order.push(id.clone());
        }
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(record)
    }

    pub fn set_provider_disabled(&self, provider_id: &str, disabled: bool) -> Result<(), AppError> {
        let provider_id = validate_provider_id(provider_id)?;
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        if disabled {
            inner.disabled_providers.insert(provider_id);
        } else {
            inner.disabled_providers.remove(&provider_id);
        }
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn delete_provider(&self, provider_id: &str, tombstone: bool) -> Result<(), AppError> {
        let provider_id = validate_provider_id(provider_id)?;
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        inner.provider_overrides.remove(&provider_id);
        inner.disabled_providers.remove(&provider_id);
        inner.provider_model_overrides.remove(&provider_id);
        inner.provider_credentials.remove(&provider_id);
        inner.active_provider_credentials.remove(&provider_id);
        inner.provider_credential_pool_modes.remove(&provider_id);
        inner.provider_credential_health.remove(&provider_id);
        inner.provider_tests.remove(&provider_id);
        inner.provider_health.remove(&provider_id);
        if tombstone {
            inner.deleted_providers.insert(provider_id.clone());
        } else {
            inner.deleted_providers.remove(&provider_id);
        }
        if let Some(order) = &mut inner.route_config.provider_order {
            order.retain(|value| value != &provider_id);
        }
        if inner.route_config.default_provider.as_deref() == Some(provider_id.as_str()) {
            inner.route_config.default_provider = None;
        }
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn upsert_provider_model_override(
        &self,
        mut record: ProviderModelOverrideRecord,
    ) -> Result<ProviderModelOverrideRecord, AppError> {
        record.provider_id = validate_provider_id(&record.provider_id)?;
        record.model = validate_non_empty("model", &record.model, 240)?;
        record.status = validate_model_status(&record.status)?;
        record.display_name = record
            .display_name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        record.family = record
            .family
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let now = now_millis();

        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        let models = inner
            .provider_model_overrides
            .entry(record.provider_id.clone())
            .or_default();
        let created_at_ms = models
            .get(&record.model)
            .map(|existing| existing.created_at_ms)
            .unwrap_or(now);
        record.created_at_ms = created_at_ms;
        record.updated_at_ms = now;
        models.insert(record.model.clone(), record.clone());
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(record)
    }

    pub fn delete_provider_model_override(
        &self,
        provider_id: &str,
        model: &str,
    ) -> Result<ProviderModelOverrideRecord, AppError> {
        let provider_id = validate_provider_id(provider_id)?;
        let model = validate_non_empty("model", model, 240)?;
        let now = now_millis();
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        let models = inner
            .provider_model_overrides
            .entry(provider_id.clone())
            .or_default();
        let created_at_ms = models
            .get(&model)
            .map(|existing| existing.created_at_ms)
            .unwrap_or(now);
        let record = ProviderModelOverrideRecord {
            provider_id,
            model: model.clone(),
            status: "disabled".to_owned(),
            display_name: None,
            family: None,
            context_window: None,
            created_at_ms,
            updated_at_ms: now,
        };
        models.insert(model, record.clone());
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(record)
    }

    pub fn upsert_provider_credential(
        &self,
        mut record: ProviderCredentialRecord,
    ) -> Result<ProviderCredentialRecord, AppError> {
        record.provider_id = validate_provider_id(&record.provider_id)?;
        record.id = validate_provider_credential_id(&record.id)?;
        record.name = validate_non_empty("name", &record.name, 120)?;
        record.api_key_env = validate_env_name(&record.api_key_env)?;
        record.base_url = validate_credential_base_url(
            &record.provider_id,
            record.base_url,
            env_flag("MODELPORT_ALLOW_PRIVATE_PROVIDER_URLS"),
        )?;
        record.status = validate_credential_status(&record.status)?;
        let now = now_millis();

        let provider_id = record.provider_id.clone();
        let credential_id = record.id.clone();
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        let active_id = inner.active_provider_credentials.get(&provider_id).cloned();
        let next_active_id = {
            let credentials = inner
                .provider_credentials
                .entry(provider_id.clone())
                .or_default();
            let created_at_ms = credentials
                .get(&credential_id)
                .map(|existing| existing.created_at_ms)
                .unwrap_or(now);
            record.created_at_ms = created_at_ms;
            record.updated_at_ms = now;
            credentials.insert(credential_id.clone(), record.clone());
            let active_id_exists = active_id
                .as_deref()
                .is_some_and(|active_id| credentials.contains_key(active_id));
            if record.status == "disabled" && active_id.as_deref() == Some(credential_id.as_str()) {
                next_enabled_provider_credential_id(credentials, Some(credential_id.as_str()))
            } else if !active_id_exists {
                next_enabled_provider_credential_id(credentials, None)
            } else {
                active_id
            }
        };
        if let Some(next_active_id) = next_active_id {
            inner
                .active_provider_credentials
                .insert(provider_id, next_active_id);
        } else {
            inner.active_provider_credentials.remove(&provider_id);
        }
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(record)
    }

    pub fn set_provider_credential_pool_mode(
        &self,
        provider_id: &str,
        mode: &str,
    ) -> Result<String, AppError> {
        let provider_id = validate_provider_id(provider_id)?;
        let mode = validate_credential_pool_mode(mode)?;
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        if mode == default_credential_pool_mode() {
            inner.provider_credential_pool_modes.remove(&provider_id);
        } else {
            inner
                .provider_credential_pool_modes
                .insert(provider_id, mode.clone());
        }
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(mode)
    }

    pub fn set_active_provider_credential(
        &self,
        provider_id: &str,
        credential_id: &str,
    ) -> Result<ProviderCredentialRecord, AppError> {
        let provider_id = validate_provider_id(provider_id)?;
        let credential_id = validate_provider_credential_id(credential_id)?;
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let record = inner
            .provider_credentials
            .get(&provider_id)
            .and_then(|credentials| credentials.get(&credential_id))
            .cloned()
            .ok_or_else(|| {
                AppError::InvalidRequest(format!(
                    "credential {credential_id} does not exist for provider {provider_id}"
                ))
            })?;
        if record.status == "disabled" {
            return Err(AppError::InvalidRequest(
                "disabled credential cannot be selected".to_owned(),
            ));
        }
        let previous = inner.clone();
        inner
            .active_provider_credentials
            .insert(provider_id.clone(), credential_id);
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(record)
    }

    pub fn delete_provider_credential(
        &self,
        provider_id: &str,
        credential_id: &str,
    ) -> Result<ProviderCredentialRecord, AppError> {
        let provider_id = validate_provider_id(provider_id)?;
        let credential_id = validate_provider_credential_id(credential_id)?;
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        let was_active =
            inner.active_provider_credentials.get(&provider_id) == Some(&credential_id);
        let (record, next_id, is_empty) = {
            let Some(credentials) = inner.provider_credentials.get_mut(&provider_id) else {
                return Err(AppError::InvalidRequest(format!(
                    "credential {credential_id} does not exist for provider {provider_id}"
                )));
            };
            let Some(record) = credentials.remove(&credential_id) else {
                return Err(AppError::InvalidRequest(format!(
                    "credential {credential_id} does not exist for provider {provider_id}"
                )));
            };
            let next_id = if was_active {
                credentials
                    .values()
                    .find(|credential| credential.status != "disabled")
                    .map(|credential| credential.id.clone())
            } else {
                None
            };
            (record, next_id, credentials.is_empty())
        };
        if was_active {
            if let Some(next_id) = next_id {
                inner
                    .active_provider_credentials
                    .insert(provider_id.clone(), next_id);
            } else {
                inner.active_provider_credentials.remove(&provider_id);
            }
        }
        if is_empty {
            inner.provider_credentials.remove(&provider_id);
            inner.provider_credential_pool_modes.remove(&provider_id);
        }
        if let Some(health) = inner.provider_credential_health.get_mut(&provider_id) {
            health.remove(&credential_id);
            if health.is_empty() {
                inner.provider_credential_health.remove(&provider_id);
            }
        }
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(record)
    }

    pub fn provider_policy_references(&self, provider_id: &str) -> Vec<serde_json::Value> {
        let inner = self.inner.lock().expect("control lock poisoned");
        let mut references = Vec::new();
        references.extend(
            inner
                .api_keys
                .values()
                .filter(|record| policy_references_provider(&record.allowed_providers, provider_id))
                .map(|record| {
                    json!({
                        "type": "apiKey",
                        "id": record.id,
                        "name": record.name,
                        "field": "allowedProviders",
                    })
                }),
        );
        references.extend(
            inner
                .teams
                .values()
                .filter(|record| policy_references_provider(&record.allowed_providers, provider_id))
                .map(|record| {
                    json!({
                        "type": "team",
                        "id": record.id,
                        "name": record.name,
                        "field": "allowedProviders",
                    })
                }),
        );
        references
    }

    pub fn data_path(&self) -> Option<String> {
        self.store.as_ref().map(JsonStore::location)
    }

    pub fn health_check(&self) -> Result<(), AppError> {
        if self.persistence_degraded.load(Ordering::Acquire) {
            return Err(AppError::NotReady(
                "control persistence is degraded after a failed write".to_owned(),
            ));
        }
        self.store
            .as_ref()
            .map(JsonStore::read_value)
            .transpose()
            .map(|_| ())
    }

    pub fn export_snapshot(&self) -> serde_json::Value {
        let inner = self.inner.lock().expect("control lock poisoned");
        json!({
            "teams": inner
                .teams
                .values()
                .map(|record| public_team(record, &inner.api_keys))
                .collect::<Vec<_>>(),
            "apiKeys": inner
                .api_keys
                .values()
                .map(public_api_key)
                .collect::<Vec<_>>(),
            "quotas": inner.quotas.values().map(public_quota).collect::<Vec<_>>(),
            "routeConfig": &inner.route_config,
            "providerTests": inner.provider_tests.values().collect::<Vec<_>>(),
            "providerHealth": inner.provider_health.values().collect::<Vec<_>>(),
            "providerCredentials": inner
                .provider_credentials
                .values()
                .flat_map(|credentials| credentials.values())
                .collect::<Vec<_>>(),
            "activeProviderCredentials": &inner.active_provider_credentials,
        })
    }

    pub fn record_provider_test(
        &self,
        provider_id: String,
        success: bool,
        message: String,
        discovered_models: Vec<String>,
    ) -> Result<u64, AppError> {
        let tested_at_ms = now_millis();
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        inner.provider_tests.insert(
            provider_id.clone(),
            ProviderTestRecord {
                provider_id,
                tested_at_ms,
                success,
                message,
                discovered_models,
            },
        );
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(tested_at_ms)
    }

    pub fn provider_test_rows(&self) -> BTreeMap<String, serde_json::Value> {
        let inner = self.inner.lock().expect("control lock poisoned");
        inner
            .provider_tests
            .iter()
            .map(|(provider_id, record)| {
                (
                    provider_id.clone(),
                    json!({
                        "testedAt": record.tested_at_ms.to_string(),
                        "success": record.success,
                        "message": record.message,
                        "models": record.discovered_models,
                        "modelCount": record.discovered_models.len(),
                    }),
                )
            })
            .collect()
    }

    pub fn provider_discovered_models(&self) -> BTreeMap<String, Vec<String>> {
        let inner = self.inner.lock().expect("control lock poisoned");
        inner
            .provider_tests
            .iter()
            .filter(|(_, record)| record.success && !record.discovered_models.is_empty())
            .map(|(provider_id, record)| (provider_id.clone(), record.discovered_models.clone()))
            .collect()
    }

    pub fn list_teams(&self) -> Vec<serde_json::Value> {
        let inner = self.inner.lock().expect("control lock poisoned");
        inner
            .teams
            .values()
            .map(|team| public_team(team, &inner.api_keys))
            .collect()
    }

    pub fn upsert_team(&self, input: UpsertTeamInput) -> Result<serde_json::Value, AppError> {
        let name = validate_team_name(&input.name)?;
        let slug = input
            .slug
            .as_deref()
            .map(validate_team_slug)
            .transpose()?
            .unwrap_or_else(|| slug_from_name(&name));
        let status = input
            .status
            .as_deref()
            .map(validate_team_status)
            .transpose()?
            .unwrap_or_else(|| "active".to_owned());
        let daily_limit_usd = input
            .daily_limit_usd
            .map(|value| validate_usd_limit("dailyLimitUsd", value))
            .transpose()?
            .unwrap_or(0.0);
        let monthly_limit_usd = input
            .monthly_limit_usd
            .map(|value| validate_usd_limit("monthlyLimitUsd", value))
            .transpose()?
            .unwrap_or(0.0);
        let allowed_models = normalize_policy_list(input.allowed_models.unwrap_or_default())?;
        let allowed_providers = normalize_policy_list(input.allowed_providers.unwrap_or_default())?;
        let description = input
            .description
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let now = now_millis();
        let mut inner = self.inner.lock().expect("control lock poisoned");
        if inner
            .teams
            .values()
            .any(|team| team.slug == slug && input.id.as_deref() != Some(team.id.as_str()))
        {
            return Err(AppError::InvalidRequest(
                "team slug already exists".to_owned(),
            ));
        }
        let id = input
            .id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("team_{}", Uuid::new_v4().simple()));
        let created_at_ms = inner
            .teams
            .get(&id)
            .map(|team| team.created_at_ms)
            .unwrap_or(now);
        let team = TeamRecord {
            id: id.clone(),
            name,
            slug,
            description,
            status,
            daily_limit_usd,
            monthly_limit_usd,
            allowed_models,
            allowed_providers,
            created_at_ms,
            updated_at_ms: now,
        };
        let previous = inner.clone();
        inner.teams.insert(id.clone(), team.clone());
        for key in inner.api_keys.values_mut() {
            if key.team_id.as_deref() == Some(&id) {
                key.team_name = Some(team.name.clone());
            }
        }
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(public_team(&team, &inner.api_keys))
    }

    pub fn delete_team(&self, team_id: &str) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let referencing_keys = inner
            .api_keys
            .values()
            .filter(|key| key.team_id.as_deref() == Some(team_id))
            .count();
        if referencing_keys > 0 {
            return Err(AppError::InvalidRequest(format!(
                "team is still referenced by {referencing_keys} API key(s); reassign or delete those keys first"
            )));
        }
        if !inner.teams.contains_key(team_id) {
            return Ok(());
        }
        let previous = inner.clone();
        inner.teams.remove(team_id);
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn provider_health_rows(&self) -> BTreeMap<String, serde_json::Value> {
        let inner = self.inner.lock().expect("control lock poisoned");
        let now = now_millis();
        inner
            .provider_health
            .iter()
            .map(|(provider_id, health)| (provider_id.clone(), provider_health_row(health, now)))
            .collect()
    }

    pub(crate) fn provider_routing_signals(&self) -> BTreeMap<String, ProviderRoutingSignal> {
        self.inner
            .lock()
            .expect("control lock poisoned")
            .provider_health
            .iter()
            .map(|(provider_id, health)| {
                (
                    provider_id.clone(),
                    ProviderRoutingSignal {
                        requests_total: health.requests_total,
                        successes_total: health.successes_total,
                    },
                )
            })
            .collect()
    }

    pub(crate) fn provider_credential_route_available(&self, provider_id: &str) -> Option<bool> {
        let inner = self.inner.lock().expect("control lock poisoned");
        let credentials = inner.provider_credentials.get(provider_id)?;
        if credentials.is_empty() {
            return None;
        }
        let now = now_millis();
        if provider_credential_pool_mode_locked(&inner, provider_id) != "manual" {
            return Some(has_usable_provider_credential_locked(
                &inner,
                provider_id,
                now,
            ));
        }
        let selected = inner
            .active_provider_credentials
            .get(provider_id)
            .and_then(|id| credentials.get(id))
            .filter(|credential| credential.status != "disabled")
            .or_else(|| {
                credentials
                    .values()
                    .find(|credential| credential.status != "disabled")
            });
        Some(selected.is_some_and(|credential| {
            env::var(&credential.api_key_env)
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        }))
    }

    pub fn provider_in_cooldown(&self, provider_id: &str) -> bool {
        let inner = self.inner.lock().expect("control lock poisoned");
        let now = now_millis();
        let Some(health) = inner.provider_health.get(provider_id) else {
            return false;
        };
        if health.cooldown_until_ms.is_none_or(|until| until <= now) {
            return false;
        }
        let mode = provider_credential_pool_mode_locked(&inner, provider_id);
        let (failure_kind, _) =
            provider_failure_guidance(health.last_status_code, health.last_error.as_deref());
        let can_use_pool = mode != "manual"
            && should_rotate_provider_credential(failure_kind)
            && has_usable_provider_credential_locked(&inner, provider_id, now);
        !can_use_pool
    }

    pub fn apply_selected_provider_credential_for_request(
        &self,
        provider_id: &str,
        provider: &mut ProviderConfig,
    ) -> Result<Option<String>, AppError> {
        let (record, has_pool, pool_mode) = {
            let mut inner = self.inner.lock().expect("control lock poisoned");
            let has_pool = inner
                .provider_credentials
                .get(provider_id)
                .is_some_and(|credentials| !credentials.is_empty());
            let pool_mode = provider_credential_pool_mode_locked(&inner, provider_id);
            let record = select_provider_credential_locked(&mut inner, provider_id, now_millis());
            (record, has_pool, pool_mode)
        };
        let Some(record) = record else {
            if has_pool && pool_mode != "manual" {
                return Err(AppError::NotReady(format!(
                    "provider {provider_id} has no usable credential in {pool_mode} pool mode"
                )));
            }
            return Ok(None);
        };
        provider.api_key_env = Some(record.api_key_env.clone());
        provider.api_key = env::var(&record.api_key_env)
            .ok()
            .filter(|value| !value.trim().is_empty());
        if let Some(base_url) = record.base_url.clone() {
            provider.base_url = base_url;
        }
        Ok(Some(record.id))
    }

    #[cfg(test)]
    pub fn select_provider_credential_for_request(
        &self,
        provider_id: &str,
    ) -> Option<ProviderCredentialRecord> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        select_provider_credential_locked(&mut inner, provider_id, now_millis())
    }

    pub fn record_provider_outcome_for_credential(
        &self,
        provider_id: &str,
        credential_id: Option<&str>,
        success: bool,
        status_code: u16,
        error_message: Option<&str>,
        persist_immediately: bool,
    ) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = persist_immediately.then(|| inner.clone());
        let now = now_millis();
        let failure_kind = record_provider_health_locked(
            &mut inner,
            provider_id,
            success,
            status_code,
            error_message,
            now,
        );
        if let Some(credential_id) = credential_id {
            record_provider_credential_health_locked(
                &mut inner,
                provider_id,
                credential_id,
                ProviderHealthUpdate {
                    success,
                    status_code,
                    error_message,
                    failure_kind,
                    now,
                },
            );
        }
        if !success {
            let mode = provider_credential_pool_mode_locked(&inner, provider_id);
            if mode != "manual"
                && should_rotate_provider_credential(failure_kind)
                && rotate_provider_credential_locked(&mut inner, provider_id, now).is_some()
                && let Some(health) = inner.provider_health.get_mut(provider_id)
            {
                health.cooldown_until_ms = None;
            }
        }
        previous.map_or(Ok(()), |previous| {
            self.save_or_restore_locked(&mut inner, previous)
        })
    }

    pub fn provider_credential_health_rows(
        &self,
    ) -> BTreeMap<String, BTreeMap<String, serde_json::Value>> {
        let inner = self.inner.lock().expect("control lock poisoned");
        let now = now_millis();
        inner
            .provider_credential_health
            .iter()
            .map(|(provider_id, health)| {
                (
                    provider_id.clone(),
                    health
                        .iter()
                        .map(|(credential_id, record)| {
                            (
                                credential_id.clone(),
                                provider_credential_health_row(record, now),
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    }

    pub fn authenticate_headers(
        &self,
        headers: &HeaderMap,
    ) -> Result<Option<ClientIdentity>, AppError> {
        let Some(token) = client_token(headers) else {
            return Ok(None);
        };
        let token_hash = hash_secret(token);
        let now = now_millis();
        let mut inner = self.inner.lock().expect("control lock poisoned");
        reset_expired_quotas_locked(&mut inner, now);

        let Some(api_key_id) = inner.api_key_hash_index.get(&token_hash).cloned() else {
            return Ok(None);
        };
        let Some(record_snapshot) = inner.api_keys.get(&api_key_id).cloned() else {
            return Ok(None);
        };

        if record_snapshot.status != "active" {
            return Err(AppError::Auth);
        }
        if record_snapshot
            .expires_at_ms
            .is_some_and(|expires| expires <= now)
        {
            if let Some(record) = inner.api_keys.get_mut(&api_key_id) {
                record.status = "revoked".to_owned();
            }
            self.save_locked(&inner)?;
            return Err(AppError::Auth);
        }

        let team = record_snapshot
            .team_id
            .as_deref()
            .and_then(|team_id| inner.teams.get(team_id).cloned());
        if record_snapshot.team_id.is_some()
            && team
                .as_ref()
                .is_none_or(|team| team.status.as_str() != "active")
        {
            return Err(AppError::Forbidden("API key team is not active".to_owned()));
        }

        if let Some(record) = inner.api_keys.get_mut(&api_key_id) {
            record.last_used_at_ms = Some(now);
        }
        let identity = ClientIdentity {
            user_id: record_snapshot.user_id.clone(),
            username: record_snapshot.username.clone(),
            api_key_id: Some(record_snapshot.id.clone()),
            api_key_name: Some(record_snapshot.name.clone()),
            api_key_group: record_snapshot.group.clone(),
            team_id: record_snapshot.team_id.clone(),
            team_name: record_snapshot.team_name.clone(),
            enforce_quotas: true,
            api_key_policy: ApiKeyPolicy {
                team_id: record_snapshot.team_id.clone(),
                ip_restricted: record_snapshot.ip_restricted,
                allowed_ips: record_snapshot.allowed_ips.clone(),
                allowed_models: record_snapshot.allowed_models.clone(),
                allowed_providers: record_snapshot.allowed_providers.clone(),
                team_allowed_models: team
                    .as_ref()
                    .map(|team| team.allowed_models.clone())
                    .unwrap_or_default(),
                team_allowed_providers: team
                    .as_ref()
                    .map(|team| team.allowed_providers.clone())
                    .unwrap_or_default(),
                team_daily_limit_usd: team
                    .as_ref()
                    .map(|team| team.daily_limit_usd)
                    .unwrap_or(0.0),
                team_monthly_limit_usd: team
                    .as_ref()
                    .map(|team| team.monthly_limit_usd)
                    .unwrap_or(0.0),
                spend_limit_usd: record_snapshot.spend_limit_usd,
                rate_limited: record_snapshot.rate_limited,
                five_hour_limit_usd: record_snapshot.five_hour_limit_usd,
                daily_limit_usd: record_snapshot.daily_limit_usd,
                weekly_limit_usd: record_snapshot.weekly_limit_usd,
                monthly_limit_usd: record_snapshot.monthly_limit_usd,
            },
        };
        Ok(Some(identity))
    }

    pub fn legacy_identity() -> ClientIdentity {
        ClientIdentity {
            user_id: "usr_local_admin".to_owned(),
            username: "local-admin".to_owned(),
            api_key_id: None,
            api_key_name: Some("MODELPORT_AUTH_TOKEN".to_owned()),
            api_key_group: Some("legacy".to_owned()),
            team_id: None,
            team_name: None,
            enforce_quotas: false,
            api_key_policy: ApiKeyPolicy::default(),
        }
    }

    pub fn list_api_keys(&self) -> Vec<PublicApiKey> {
        let inner = self.inner.lock().expect("control lock poisoned");
        inner.api_keys.values().map(public_api_key).collect()
    }

    pub fn tenant_scope(&self, identity: &ClientIdentity) -> Result<TenantScope, AppError> {
        let Some(key_id) = identity.api_key_id.as_deref() else {
            return Ok(TenantScope::legacy_local());
        };
        let inner = self.inner.lock().expect("control lock poisoned");
        let record = inner.api_keys.get(key_id).ok_or(AppError::Auth)?;
        if record.status != "active" || record.user_id != identity.user_id {
            return Err(AppError::Auth);
        }
        Ok(TenantScope::from_strings(
            record.organization_id.clone(),
            record.project_id.clone(),
            record.environment_id.clone(),
        ))
    }

    pub fn list_user_api_keys(&self, user_id: &str) -> Vec<PublicApiKey> {
        self.list_api_keys()
            .into_iter()
            .filter(|record| record.user_id == user_id)
            .collect()
    }

    pub fn active_api_key_count(&self, user_id: &str) -> u32 {
        let inner = self.inner.lock().expect("control lock poisoned");
        inner
            .api_keys
            .values()
            .filter(|record| record.user_id == user_id && record.status == "active")
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    }

    pub fn api_key_counts(&self) -> (u64, u64) {
        let inner = self.inner.lock().expect("control lock poisoned");
        (
            inner.api_keys.len().try_into().unwrap_or(u64::MAX),
            inner
                .api_keys
                .values()
                .filter(|record| record.status == "active")
                .count()
                .try_into()
                .unwrap_or(u64::MAX),
        )
    }

    pub fn api_key_user_id(&self, key_id: &str) -> Result<String, AppError> {
        let inner = self.inner.lock().expect("control lock poisoned");
        inner
            .api_keys
            .get(key_id)
            .map(|record| record.user_id.clone())
            .ok_or_else(|| AppError::InvalidRequest("API key not found".to_owned()))
    }

    pub fn create_api_key(&self, input: CreateApiKeyInput) -> Result<CreatedApiKey, AppError> {
        let name = input.name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(AppError::InvalidRequest(
                "API key name must be 1-80 characters".to_owned(),
            ));
        }
        let user_id = input.user_id.trim();
        if user_id.is_empty() {
            return Err(AppError::InvalidRequest("userId is required".to_owned()));
        }
        let username = input.username.unwrap_or_else(|| user_id.to_owned());
        let allowed_models = normalize_policy_list(input.allowed_models.unwrap_or_default())?;
        let allowed_providers = normalize_policy_list(input.allowed_providers.unwrap_or_default())?;
        let (organization_id, project_id, environment_id) =
            normalize_tenant_scope(None, None, None)?;
        let now = now_millis();
        let expires_at_ms = input
            .expires_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                value.parse::<u64>().map_err(|_| {
                    AppError::InvalidRequest("expiresAt must be a millisecond timestamp".to_owned())
                })
            })
            .transpose()?;
        if expires_at_ms.is_some_and(|expires_at| expires_at <= now) {
            return Err(AppError::InvalidRequest(
                "cannot create an expired API key".to_owned(),
            ));
        }
        let key = new_api_key();
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let (team_id, team_name) = resolve_team_ref(&inner, input.team_id)?;
        let record = ApiKeyRecord {
            id: format!("key_{}", Uuid::new_v4().simple()),
            user_id: user_id.to_owned(),
            username,
            name: name.to_owned(),
            key_hash: hash_secret(&key),
            key_prefix: key.chars().take(12).collect(),
            key_preview: preview_secret(&key),
            group: input.group.filter(|value| !value.trim().is_empty()),
            team_id,
            team_name,
            allowed_models,
            allowed_providers,
            organization_id,
            project_id,
            environment_id,
            created_at_ms: now,
            last_used_at_ms: None,
            expires_at_ms,
            status: "active".to_owned(),
            ip_restricted: false,
            allowed_ips: Vec::new(),
            spend_limit_usd: 0.0,
            rate_limited: false,
            five_hour_limit_usd: 0.0,
            daily_limit_usd: 0.0,
            weekly_limit_usd: 0.0,
            monthly_limit_usd: 0.0,
        };

        let previous = inner.clone();
        inner.api_keys.insert(record.id.clone(), record.clone());
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(CreatedApiKey {
            public: public_api_key(&record),
            key,
        })
    }

    pub fn revoke_api_key(&self, key_id: &str) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        if !inner.api_keys.contains_key(key_id) {
            return Err(AppError::InvalidRequest("API key not found".to_owned()));
        }
        let previous = inner.clone();
        inner
            .api_keys
            .get_mut(key_id)
            .expect("API key existence checked above")
            .status = "revoked".to_owned();
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn update_api_key(
        &self,
        key_id: &str,
        input: UpdateApiKeyInput,
    ) -> Result<PublicApiKey, AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let Some(record) = inner.api_keys.get(key_id).cloned() else {
            return Err(AppError::InvalidRequest("API key not found".to_owned()));
        };

        let mut updated = record;
        if let Some(name) = input.name {
            let name = name.trim();
            if name.is_empty() || name.len() > 80 {
                return Err(AppError::InvalidRequest(
                    "API key name must be 1-80 characters".to_owned(),
                ));
            }
            updated.name = name.to_owned();
        }
        if let Some(group) = input.group {
            let group = group.trim();
            updated.group = if group.is_empty() {
                None
            } else {
                Some(group.to_owned())
            };
        }
        if let Some(team_id) = input.team_id {
            let (team_id, team_name) = resolve_team_ref(&inner, Some(team_id))?;
            updated.team_id = team_id;
            updated.team_name = team_name;
        }
        if let Some(allowed_models) = input.allowed_models {
            updated.allowed_models = normalize_policy_list(allowed_models)?;
        }
        if let Some(allowed_providers) = input.allowed_providers {
            updated.allowed_providers = normalize_policy_list(allowed_providers)?;
        }
        if let Some(expires_at) = input.expires_at {
            let expires_at = expires_at.trim();
            updated.expires_at_ms = if expires_at.is_empty() {
                None
            } else {
                Some(expires_at.parse::<u64>().map_err(|_| {
                    AppError::InvalidRequest("expiresAt must be a millisecond timestamp".to_owned())
                })?)
            };
        }
        if let Some(status) = input.status {
            let status = status.trim();
            if !matches!(status, "active" | "revoked") {
                return Err(AppError::InvalidRequest(
                    "invalid API key status".to_owned(),
                ));
            }
            updated.status = status.to_owned();
        }
        if let Some(ip_restricted) = input.ip_restricted {
            updated.ip_restricted = ip_restricted;
        }
        if let Some(allowed_ips) = input.allowed_ips {
            updated.allowed_ips = normalize_ip_rules(allowed_ips)?;
        }
        if let Some(spend_limit_usd) = input.spend_limit_usd {
            updated.spend_limit_usd = validate_usd_limit("spendLimitUsd", spend_limit_usd)?;
        }
        if let Some(rate_limited) = input.rate_limited {
            updated.rate_limited = rate_limited;
        }
        if let Some(five_hour_limit_usd) = input.five_hour_limit_usd {
            updated.five_hour_limit_usd =
                validate_usd_limit("fiveHourLimitUsd", five_hour_limit_usd)?;
        }
        if let Some(daily_limit_usd) = input.daily_limit_usd {
            updated.daily_limit_usd = validate_usd_limit("dailyLimitUsd", daily_limit_usd)?;
        }
        if let Some(weekly_limit_usd) = input.weekly_limit_usd {
            updated.weekly_limit_usd = validate_usd_limit("weeklyLimitUsd", weekly_limit_usd)?;
        }
        if let Some(monthly_limit_usd) = input.monthly_limit_usd {
            updated.monthly_limit_usd = validate_usd_limit("monthlyLimitUsd", monthly_limit_usd)?;
        }

        if updated.status == "active"
            && updated
                .expires_at_ms
                .is_some_and(|expires_at| expires_at <= now_millis())
        {
            return Err(AppError::InvalidRequest(
                "cannot activate an expired API key".to_owned(),
            ));
        }

        let previous = inner.clone();
        inner.api_keys.insert(updated.id.clone(), updated.clone());
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(public_api_key(&updated))
    }

    pub fn bind_api_key_scope(
        &self,
        key_id: &str,
        input: BindApiKeyScopeInput,
    ) -> Result<PublicApiKey, AppError> {
        let (organization_id, project_id, environment_id) = normalize_tenant_scope(
            Some(input.organization_id),
            Some(input.project_id),
            Some(input.environment_id),
        )?;
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let Some(mut updated) = inner.api_keys.get(key_id).cloned() else {
            return Err(AppError::InvalidRequest("API key not found".to_owned()));
        };
        updated.organization_id = organization_id;
        updated.project_id = project_id;
        updated.environment_id = environment_id;
        let previous = inner.clone();
        inner.api_keys.insert(updated.id.clone(), updated.clone());
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(public_api_key(&updated))
    }

    pub fn delete_api_key(&self, key_id: &str) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        if !inner.api_keys.contains_key(key_id) {
            return Err(AppError::InvalidRequest("API key not found".to_owned()));
        }
        let previous = inner.clone();
        inner.api_keys.remove(key_id);
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn delete_user_resources(&self, user_id: &str) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        for record in inner.api_keys.values_mut() {
            if record.user_id == user_id {
                record.status = "revoked".to_owned();
            }
        }
        inner.quotas.retain(|_, quota| quota.user_id != user_id);
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn list_quotas(&self) -> Result<Vec<PublicQuota>, AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        reset_expired_quotas_locked(&mut inner, now_millis());
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(inner.quotas.values().map(public_quota).collect())
    }

    pub(crate) fn usage_quota_limits(&self) -> Vec<UsageQuotaLimit> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        reset_expired_quotas_locked(&mut inner, now_millis());
        inner
            .quotas
            .values()
            .map(|quota| UsageQuotaLimit {
                id: quota.id.clone(),
                user_id: quota.user_id.clone(),
                quota_type: quota.quota_type.clone(),
                limit: quota.limit,
                period_start_ms: quota.period_start_ms,
            })
            .collect()
    }

    pub fn upsert_quota(&self, input: UpsertQuotaInput) -> Result<PublicQuota, AppError> {
        if input.user_id.trim().is_empty() || input.username.trim().is_empty() {
            return Err(AppError::InvalidRequest(
                "userId and username are required".to_owned(),
            ));
        }
        if input.limit < 0.0 {
            return Err(AppError::InvalidRequest(
                "quota limit must be zero or greater".to_owned(),
            ));
        }
        if !matches!(input.quota_type.as_str(), "tokens" | "requests" | "cost") {
            return Err(AppError::InvalidRequest("invalid quota type".to_owned()));
        }
        if !matches!(input.period.as_str(), "daily" | "weekly" | "monthly") {
            return Err(AppError::InvalidRequest("invalid quota period".to_owned()));
        }

        let now = now_millis();
        let (period_start_ms, period_end_ms) = current_period(&input.period, now);
        let id = input
            .id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("quota_{}", Uuid::new_v4().simple()));
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let quota = QuotaRecord {
            id: id.clone(),
            user_id: input.user_id,
            username: input.username,
            quota_type: input.quota_type,
            limit: input.limit,
            period: input.period,
            period_start_ms,
            period_end_ms,
            reset_at_ms: period_end_ms,
        };
        let previous = inner.clone();
        inner.quotas.insert(id, quota.clone());
        self.save_or_restore_locked(&mut inner, previous)?;
        Ok(public_quota(&quota))
    }

    pub fn delete_quota(&self, quota_id: &str) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let previous = inner.clone();
        inner.quotas.remove(quota_id);
        self.save_or_restore_locked(&mut inner, previous)
    }

    pub fn check_quotas(
        &self,
        identity: &ClientIdentity,
        client_ip: Option<&str>,
        requested_model: &str,
        resolved_model: &str,
        provider_id: &str,
    ) -> Result<UsagePolicySnapshot, AppError> {
        if !identity.enforce_quotas {
            return Ok(UsagePolicySnapshot::default());
        }
        let mut inner = self.inner.lock().expect("control lock poisoned");
        let now = now_millis();
        reset_expired_quotas_locked(&mut inner, now);
        if identity.api_key_id.is_some() {
            enforce_api_key_policy(ApiKeyPolicyCheck {
                policy: &identity.api_key_policy,
                client_ip,
                requested_model,
                resolved_model,
                provider_id,
            })?;
        }
        Ok(UsagePolicySnapshot {
            user_id: identity.user_id.clone(),
            username: identity.username.clone(),
            api_key_id: identity.api_key_id.clone(),
            team_id: identity
                .team_id
                .clone()
                .or_else(|| identity.api_key_policy.team_id.clone()),
            api_key_policy: identity.api_key_policy.clone(),
            quotas: inner
                .quotas
                .values()
                .filter(|quota| quota.user_id == identity.user_id)
                .map(|quota| UsageQuotaLimit {
                    id: quota.id.clone(),
                    user_id: quota.user_id.clone(),
                    quota_type: quota.quota_type.clone(),
                    limit: quota.limit,
                    period_start_ms: quota.period_start_ms,
                })
                .collect(),
        })
    }

    fn save_or_restore_locked(
        &self,
        inner: &mut ControlInner,
        previous: ControlInner,
    ) -> Result<(), AppError> {
        rebuild_api_key_hash_index(inner);
        if let Err(error) = self.save_locked(inner) {
            *inner = previous;
            return Err(error);
        }
        Ok(())
    }

    fn save_locked(&self, inner: &ControlInner) -> Result<(), AppError> {
        let result = if let Some(store) = &self.store {
            self.write_locked(store, inner)
        } else {
            Ok(())
        };
        self.persistence_degraded
            .store(result.is_err(), Ordering::Release);
        result
    }

    fn write_locked(&self, store: &JsonStore, inner: &ControlInner) -> Result<(), AppError> {
        let file = ControlFile {
            teams: inner.teams.values().cloned().collect(),
            api_keys: inner.api_keys.values().cloned().collect(),
            quotas: inner.quotas.values().cloned().collect(),
            route_config: inner.route_config.clone(),
            provider_tests: inner.provider_tests.values().cloned().collect(),
            provider_health: inner.provider_health.values().cloned().collect(),
            provider_overrides: inner.provider_overrides.values().cloned().collect(),
            disabled_providers: inner.disabled_providers.clone(),
            deleted_providers: inner.deleted_providers.clone(),
            provider_model_overrides: inner
                .provider_model_overrides
                .values()
                .flat_map(|models| models.values().cloned())
                .collect(),
            provider_credentials: inner
                .provider_credentials
                .values()
                .flat_map(|credentials| credentials.values().cloned())
                .collect(),
            active_provider_credentials: inner.active_provider_credentials.clone(),
            provider_credential_pool_modes: inner.provider_credential_pool_modes.clone(),
            provider_credential_health: inner
                .provider_credential_health
                .values()
                .flat_map(|health| health.values().cloned())
                .collect(),
        };
        store.write_json(&file)
    }
}

fn redact_historical_control_errors(file: &mut ControlFile) -> bool {
    let mut changed = false;
    for record in &mut file.provider_health {
        changed |= redact_optional_error(&mut record.last_error);
    }
    for record in &mut file.provider_credential_health {
        changed |= redact_optional_error(&mut record.last_error);
    }
    for record in &mut file.provider_tests {
        if !record.success && record.message != "provider test failed [details redacted]" {
            record.message = "provider test failed [details redacted]".to_owned();
            changed = true;
        }
    }
    changed
}

fn redact_optional_error(error: &mut Option<String>) -> bool {
    let Some(current) = error.as_deref() else {
        return false;
    };
    let redacted = audit_safe_persisted_error(current);
    if redacted == current {
        return false;
    }
    *error = Some(redacted);
    true
}

fn record_provider_health_locked(
    inner: &mut ControlInner,
    provider_id: &str,
    success: bool,
    status_code: u16,
    error_message: Option<&str>,
    now: u64,
) -> &'static str {
    let failure_kind = provider_failure_guidance(Some(status_code), error_message).0;
    let health = inner
        .provider_health
        .entry(provider_id.to_owned())
        .or_insert_with(|| ProviderHealthRecord {
            provider_id: provider_id.to_owned(),
            ..ProviderHealthRecord::default()
        });
    health.requests_total = health.requests_total.saturating_add(1);
    if success {
        health.successes_total = health.successes_total.saturating_add(1);
        health.consecutive_failures = 0;
        health.last_success_at_ms = Some(now);
        health.cooldown_until_ms = None;
        health.last_error = None;
        health.last_status_code = Some(status_code);
    } else {
        health.failures_total = health.failures_total.saturating_add(1);
        if provider_failure_can_trigger_cooldown(failure_kind) {
            health.consecutive_failures = health.consecutive_failures.saturating_add(1);
        } else {
            health.consecutive_failures = 0;
        }
        health.last_failure_at_ms = Some(now);
        health.last_status_code = Some(status_code);
        health.last_error = truncated_error(error_message, status_code);
        if provider_failure_can_trigger_cooldown(failure_kind)
            && (health.consecutive_failures >= 3 || status_code == 429 || status_code >= 500)
        {
            let seconds = cooldown_seconds(health.consecutive_failures);
            health.cooldown_until_ms = Some(now.saturating_add(seconds.saturating_mul(1_000)));
        }
    }
    failure_kind
}

struct ProviderHealthUpdate<'a> {
    success: bool,
    status_code: u16,
    error_message: Option<&'a str>,
    failure_kind: &'a str,
    now: u64,
}

fn record_provider_credential_health_locked(
    inner: &mut ControlInner,
    provider_id: &str,
    credential_id: &str,
    update: ProviderHealthUpdate<'_>,
) {
    let health = inner
        .provider_credential_health
        .entry(provider_id.to_owned())
        .or_default()
        .entry(credential_id.to_owned())
        .or_insert_with(|| ProviderCredentialHealthRecord {
            provider_id: provider_id.to_owned(),
            credential_id: credential_id.to_owned(),
            ..ProviderCredentialHealthRecord::default()
        });
    health.requests_total = health.requests_total.saturating_add(1);
    health.last_used_at_ms = Some(update.now);
    if update.success {
        health.successes_total = health.successes_total.saturating_add(1);
        health.consecutive_failures = 0;
        health.last_success_at_ms = Some(update.now);
        health.cooldown_until_ms = None;
        health.last_error = None;
        health.last_status_code = Some(update.status_code);
    } else {
        health.failures_total = health.failures_total.saturating_add(1);
        if provider_failure_can_trigger_cooldown(update.failure_kind) {
            health.consecutive_failures = health.consecutive_failures.saturating_add(1);
        } else {
            health.consecutive_failures = 0;
        }
        health.last_failure_at_ms = Some(update.now);
        health.last_status_code = Some(update.status_code);
        health.last_error = truncated_error(update.error_message, update.status_code);
        if provider_failure_can_trigger_cooldown(update.failure_kind)
            && (should_rotate_provider_credential(update.failure_kind)
                || health.consecutive_failures >= 3)
        {
            let seconds =
                credential_cooldown_seconds(update.failure_kind, health.consecutive_failures);
            health.cooldown_until_ms =
                Some(update.now.saturating_add(seconds.saturating_mul(1_000)));
        }
    }
}

fn truncated_error(error_message: Option<&str>, status_code: u16) -> Option<String> {
    error_message
        .map(|value| value.chars().take(240).collect())
        .or_else(|| Some(format!("HTTP {status_code}")))
}

fn select_provider_credential_locked(
    inner: &mut ControlInner,
    provider_id: &str,
    now: u64,
) -> Option<ProviderCredentialRecord> {
    let mode = provider_credential_pool_mode_locked(inner, provider_id);
    let active_id = inner.active_provider_credentials.get(provider_id).cloned();
    let (available, fallback) = {
        let credentials = inner.provider_credentials.get(provider_id)?;
        let health = inner.provider_credential_health.get(provider_id);
        let available = credentials
            .values()
            .filter(|credential| provider_credential_is_usable(credential, health, now))
            .cloned()
            .collect::<Vec<_>>();
        let fallback = active_id
            .as_deref()
            .and_then(|id| credentials.get(id))
            .filter(|credential| credential.status != "disabled")
            .or_else(|| {
                credentials
                    .values()
                    .find(|credential| credential.status != "disabled")
            })
            .cloned();
        (available, fallback)
    };

    let selected = match mode.as_str() {
        "manual" => fallback,
        "round_robin" => round_robin_provider_credential(&available, active_id.as_deref())
            .or_else(|| available.first().cloned()),
        _ => active_id
            .as_deref()
            .and_then(|id| available.iter().find(|credential| credential.id == id))
            .cloned()
            .or_else(|| available.first().cloned()),
    };

    if let Some(selected) = selected.as_ref()
        && selected.status != "disabled"
    {
        inner
            .active_provider_credentials
            .insert(provider_id.to_owned(), selected.id.clone());
    }
    selected
}

fn provider_failure_can_trigger_cooldown(failure_kind: &str) -> bool {
    matches!(
        failure_kind,
        "account" | "rate_limit" | "config" | "upstream_unavailable"
    )
}

fn round_robin_provider_credential(
    candidates: &[ProviderCredentialRecord],
    current_id: Option<&str>,
) -> Option<ProviderCredentialRecord> {
    if candidates.is_empty() {
        return None;
    }
    let Some(current_id) = current_id else {
        return candidates.first().cloned();
    };
    candidates
        .iter()
        .skip_while(|credential| credential.id != current_id)
        .skip(1)
        .chain(candidates.iter())
        .find(|credential| credential.id != current_id)
        .cloned()
        .or_else(|| candidates.first().cloned())
}

fn has_usable_provider_credential_locked(
    inner: &ControlInner,
    provider_id: &str,
    now: u64,
) -> bool {
    let Some(credentials) = inner.provider_credentials.get(provider_id) else {
        return false;
    };
    let health = inner.provider_credential_health.get(provider_id);
    credentials
        .values()
        .any(|credential| provider_credential_is_usable(credential, health, now))
}

fn provider_credential_is_usable(
    credential: &ProviderCredentialRecord,
    health: Option<&BTreeMap<String, ProviderCredentialHealthRecord>>,
    now: u64,
) -> bool {
    credential.status != "disabled"
        && env::var(&credential.api_key_env)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
        && health
            .and_then(|health| health.get(&credential.id))
            .and_then(|record| record.cooldown_until_ms)
            .is_none_or(|until| until <= now)
}

fn provider_credential_pool_mode_locked(inner: &ControlInner, provider_id: &str) -> String {
    inner
        .provider_credential_pool_modes
        .get(provider_id)
        .map(String::as_str)
        .unwrap_or("failover")
        .to_owned()
}

fn rotate_provider_credential_locked(
    inner: &mut ControlInner,
    provider_id: &str,
    now: u64,
) -> Option<(String, String, String)> {
    let credentials = inner.provider_credentials.get(provider_id)?;
    let current_id = inner.active_provider_credentials.get(provider_id).cloned();
    let health = inner.provider_credential_health.get(provider_id);
    let candidates = credentials
        .values()
        .filter(|credential| provider_credential_is_usable(credential, health, now))
        .cloned()
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    let next = if let Some(current_id) = current_id.as_deref() {
        candidates
            .iter()
            .skip_while(|credential| credential.id != current_id)
            .skip(1)
            .chain(candidates.iter())
            .find(|credential| credential.id != current_id)?
    } else {
        &candidates[0]
    };
    let from_id = current_id.unwrap_or_else(|| "default".to_owned());
    inner
        .active_provider_credentials
        .insert(provider_id.to_owned(), next.id.clone());
    Some((from_id, next.id.clone(), next.name.clone()))
}

fn next_enabled_provider_credential_id(
    credentials: &BTreeMap<String, ProviderCredentialRecord>,
    exclude_id: Option<&str>,
) -> Option<String> {
    credentials
        .values()
        .find(|credential| {
            credential.status != "disabled" && exclude_id != Some(credential.id.as_str())
        })
        .map(|credential| credential.id.clone())
}

fn validate_usd_limit(field: &str, value: f64) -> Result<f64, AppError> {
    if !value.is_finite() || value < 0.0 {
        return Err(AppError::InvalidRequest(format!(
            "{field} must be zero or greater"
        )));
    }
    Ok(value)
}

fn validate_team_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 80 {
        return Err(AppError::InvalidRequest(
            "team name must be 1-80 characters".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn validate_team_slug(value: &str) -> Result<String, AppError> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AppError::InvalidRequest(
            "team slug may only contain lowercase letters, numbers, dashes, and underscores"
                .to_owned(),
        ));
    }
    Ok(value)
}

fn validate_team_status(value: &str) -> Result<String, AppError> {
    match value.trim() {
        "active" | "archived" | "disabled" => Ok(value.trim().to_owned()),
        _ => Err(AppError::InvalidRequest("invalid team status".to_owned())),
    }
}

fn slug_from_name(value: &str) -> String {
    let mut slug = String::new();
    for ch in value.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            slug.push(ch);
        } else if ch.is_whitespace() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    if slug.is_empty() {
        format!("team-{}", Uuid::new_v4().simple())
    } else {
        slug.trim_matches('-').chars().take(64).collect()
    }
}

fn validate_provider_id(value: &str) -> Result<String, AppError> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 80
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AppError::InvalidRequest(
            "provider id may only contain lowercase letters, numbers, dashes, and underscores"
                .to_owned(),
        ));
    }
    Ok(value)
}

fn validate_non_empty(field: &str, value: &str, max_len: usize) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_len {
        return Err(AppError::InvalidRequest(format!(
            "{field} must be 1-{max_len} characters"
        )));
    }
    Ok(value.to_owned())
}

fn validate_model_status(value: &str) -> Result<String, AppError> {
    match value.trim() {
        "active" | "disabled" => Ok(value.trim().to_owned()),
        _ => Err(AppError::InvalidRequest(
            "model status must be active or disabled".to_owned(),
        )),
    }
}

fn default_true() -> bool {
    true
}

fn default_max_tokens_field() -> String {
    "max_completion_tokens".to_owned()
}

fn default_fidelity_mode() -> String {
    "best_effort".to_owned()
}

fn default_model_status() -> String {
    "active".to_owned()
}

fn resolve_team_ref(
    inner: &ControlInner,
    team_id: Option<String>,
) -> Result<(Option<String>, Option<String>), AppError> {
    let Some(team_id) = team_id.map(|value| value.trim().to_owned()) else {
        return Ok((None, None));
    };
    if team_id.is_empty() {
        return Ok((None, None));
    }
    let Some(team) = inner.teams.get(&team_id) else {
        return Err(AppError::InvalidRequest("team not found".to_owned()));
    };
    Ok((Some(team.id.clone()), Some(team.name.clone())))
}

struct ApiKeyPolicyCheck<'a> {
    policy: &'a ApiKeyPolicy,
    client_ip: Option<&'a str>,
    requested_model: &'a str,
    resolved_model: &'a str,
    provider_id: &'a str,
}

fn enforce_api_key_policy(check: ApiKeyPolicyCheck<'_>) -> Result<(), AppError> {
    let ApiKeyPolicyCheck {
        policy,
        client_ip,
        requested_model,
        resolved_model,
        provider_id,
    } = check;

    enforce_ip_policy(policy.ip_restricted, &policy.allowed_ips, client_ip)?;
    enforce_model_policy(
        "API key",
        &policy.allowed_models,
        requested_model,
        resolved_model,
    )?;
    enforce_provider_policy("API key", &policy.allowed_providers, provider_id)?;
    enforce_model_policy(
        "team",
        &policy.team_allowed_models,
        requested_model,
        resolved_model,
    )?;
    enforce_provider_policy("team", &policy.team_allowed_providers, provider_id)?;

    Ok(())
}

fn effective_aliases_locked(
    base_aliases: &HashMap<String, String>,
    route_config: &RouteConfigRecord,
) -> HashMap<String, String> {
    let mut aliases = base_aliases.clone();
    for alias in &route_config.deleted_aliases {
        aliases.remove(alias);
    }
    for (alias, target) in &route_config.aliases {
        aliases.insert(alias.clone(), target.clone());
    }
    aliases
}

fn reset_expired_quotas_locked(inner: &mut ControlInner, now: u64) {
    for quota in inner.quotas.values_mut() {
        if quota.reset_at_ms > now {
            continue;
        }
        let (start, end) = current_period(&quota.period, now);
        quota.period_start_ms = start;
        quota.period_end_ms = end;
        quota.reset_at_ms = end;
    }
}

fn client_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
        })
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(false)
}

fn new_api_key() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("sk-mp-{}", hex_bytes(&bytes))
}

fn hash_secret(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn preview_secret(value: &str) -> String {
    let start = value.chars().take(8).collect::<String>();
    let end = value
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}...{end}")
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn rebuild_api_key_hash_index(inner: &mut ControlInner) {
    inner.api_key_hash_index = inner
        .api_keys
        .iter()
        .map(|(id, record)| (record.key_hash.clone(), id.clone()))
        .collect();
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn failing_store_path(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "modelport-{label}-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    fn team_input(id: Option<String>, name: &str) -> UpsertTeamInput {
        UpsertTeamInput {
            id,
            name: name.to_owned(),
            slug: None,
            description: None,
            status: None,
            daily_limit_usd: None,
            monthly_limit_usd: None,
            allowed_models: None,
            allowed_providers: None,
        }
    }

    fn api_key_input(team_id: Option<String>) -> CreateApiKeyInput {
        CreateApiKeyInput {
            user_id: "usr_test".to_owned(),
            username: Some("test-user".to_owned()),
            name: "local".to_owned(),
            group: None,
            team_id,
            allowed_models: None,
            allowed_providers: None,
            expires_at: None,
        }
    }

    #[test]
    fn api_key_tenant_scope_is_admin_bound_and_used_for_requests() {
        let store = ControlStore::for_tests();
        let created = store.create_api_key(api_key_input(None)).unwrap();
        let bound = store
            .bind_api_key_scope(
                &created.public.id,
                BindApiKeyScopeInput {
                    organization_id: "org_dave".to_owned(),
                    project_id: "prj_quantpilot".to_owned(),
                    environment_id: "env_test".to_owned(),
                },
            )
            .unwrap();
        assert_eq!(bound.organization_id, "org_dave");
        assert_eq!(bound.project_id, "prj_quantpilot");
        assert_eq!(bound.environment_id, "env_test");

        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&created.key).unwrap());
        let identity = store.authenticate_headers(&headers).unwrap().unwrap();
        let tenant = store.tenant_scope(&identity).unwrap();
        assert_eq!(tenant.organization_id.as_str(), "org_dave");
        assert_eq!(tenant.project_id.as_str(), "prj_quantpilot");
        assert_eq!(tenant.environment_id.as_str(), "env_test");
    }

    #[test]
    fn api_key_scope_binding_rejects_unsafe_identifiers() {
        let store = ControlStore::for_tests();
        let created = store.create_api_key(api_key_input(None)).unwrap();
        assert!(
            store
                .bind_api_key_scope(
                    &created.public.id,
                    BindApiKeyScopeInput {
                        organization_id: "org dave".to_owned(),
                        project_id: "prj_quantpilot".to_owned(),
                        environment_id: "env_test".to_owned(),
                    },
                )
                .is_err()
        );
    }

    #[test]
    fn historical_control_errors_are_redacted_before_use() {
        let mut file = ControlFile {
            provider_tests: vec![ProviderTestRecord {
                provider_id: "local".to_owned(),
                tested_at_ms: 1,
                success: false,
                message: "provider echoed Bearer private-token".to_owned(),
                discovered_models: Vec::new(),
            }],
            provider_health: vec![ProviderHealthRecord {
                provider_id: "local".to_owned(),
                last_error: Some("tool schema path /properties/private_customer_id".to_owned()),
                ..ProviderHealthRecord::default()
            }],
            ..ControlFile::default()
        };

        assert!(redact_historical_control_errors(&mut file));
        assert_eq!(
            file.provider_tests[0].message,
            "provider test failed [details redacted]"
        );
        assert_eq!(
            file.provider_health[0].last_error.as_deref(),
            Some("request failed: tool protocol error [details redacted]")
        );
        assert!(!redact_historical_control_errors(&mut file));
    }

    fn provider_override(id: &str, display_name: &str) -> ProviderOverrideRecord {
        ProviderOverrideRecord {
            id: id.to_owned(),
            display_name: display_name.to_owned(),
            protocol: "openai-compat".to_owned(),
            base_url: "https://api.example.com/v1".to_owned(),
            api_key_env: Some("TEST_PROVIDER_API_KEY".to_owned()),
            api_key_required: true,
            default_model: "test-model".to_owned(),
            models: vec!["test-model".to_owned()],
            model_prefixes: vec!["test-".to_owned()],
            passthrough_unknown_models: false,
            max_tokens_field: "max_tokens".to_owned(),
            deduplicate_stream_text: false,
            buffer_stream_text: false,
            fidelity_mode: "strict".to_owned(),
            tool_use: ToolUseConfig::default(),
            pricing: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }

    fn provider_credential(provider_id: &str, id: &str) -> ProviderCredentialRecord {
        ProviderCredentialRecord {
            id: id.to_owned(),
            provider_id: provider_id.to_owned(),
            name: id.to_owned(),
            api_key_env: format!("TEST_{}_API_KEY", id.replace('-', "_").to_uppercase()),
            base_url: None,
            status: "active".to_owned(),
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }

    fn provider_routing_state(store: &ControlStore) -> serde_json::Value {
        let inner = store.inner.lock().expect("control lock poisoned");
        json!({
            "routeConfig": &inner.route_config,
            "providerTests": &inner.provider_tests,
            "providerHealth": &inner.provider_health,
            "providerOverrides": &inner.provider_overrides,
            "disabledProviders": &inner.disabled_providers,
            "deletedProviders": &inner.deleted_providers,
            "providerModelOverrides": &inner.provider_model_overrides,
            "providerCredentials": &inner.provider_credentials,
            "activeProviderCredentials": &inner.active_provider_credentials,
            "providerCredentialPoolModes": &inner.provider_credential_pool_modes,
            "providerCredentialHealth": &inner.provider_credential_health,
        })
    }

    #[test]
    fn creates_and_authenticates_api_key() {
        let store = ControlStore::for_tests();
        let created = store
            .create_api_key(CreateApiKeyInput {
                user_id: "usr_test".to_owned(),
                username: Some("test-user".to_owned()),
                name: "local".to_owned(),
                group: None,
                team_id: None,
                allowed_models: None,
                allowed_providers: None,
                expires_at: None,
            })
            .unwrap();
        assert!(!format!("{created:?}").contains(&created.key));
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&created.key).unwrap());
        let identity = store.authenticate_headers(&headers).unwrap().unwrap();
        assert_eq!(identity.user_id, "usr_test");
        assert_eq!(store.active_api_key_count("usr_test"), 1);
    }

    #[test]
    fn api_key_creation_rejects_invalid_or_expired_timestamps() {
        let store = ControlStore::for_tests();
        let create = |expires_at: String| CreateApiKeyInput {
            user_id: "usr_test".to_owned(),
            username: Some("test-user".to_owned()),
            name: "local".to_owned(),
            group: None,
            team_id: None,
            allowed_models: None,
            allowed_providers: None,
            expires_at: Some(expires_at),
        };

        assert!(matches!(
            store.create_api_key(create("not-a-timestamp".to_owned())),
            Err(AppError::InvalidRequest(message)) if message.contains("millisecond timestamp")
        ));
        assert!(matches!(
            store.create_api_key(create(now_millis().saturating_sub(1).to_string())),
            Err(AppError::InvalidRequest(message)) if message.contains("expired")
        ));
    }

    #[test]
    fn deleting_user_resources_revokes_keys_and_removes_quotas() {
        let store = ControlStore::for_tests();
        let created = store
            .create_api_key(CreateApiKeyInput {
                user_id: "usr_test".to_owned(),
                username: Some("test-user".to_owned()),
                name: "local".to_owned(),
                group: None,
                team_id: None,
                allowed_models: None,
                allowed_providers: None,
                expires_at: None,
            })
            .unwrap();
        store
            .upsert_quota(UpsertQuotaInput {
                id: None,
                user_id: "usr_test".to_owned(),
                username: "test-user".to_owned(),
                quota_type: "tokens".to_owned(),
                limit: 1_000.0,
                period: "monthly".to_owned(),
            })
            .unwrap();

        store.delete_user_resources("usr_test").unwrap();

        assert_eq!(store.active_api_key_count("usr_test"), 0);
        assert!(store.list_quotas().unwrap().is_empty());
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&created.key).unwrap());
        assert!(matches!(
            store.authenticate_headers(&headers),
            Err(AppError::Auth)
        ));
    }

    #[test]
    fn updates_and_restores_api_key() {
        let store = ControlStore::for_tests();
        let created = store
            .create_api_key(CreateApiKeyInput {
                user_id: "usr_test".to_owned(),
                username: Some("test-user".to_owned()),
                name: "local".to_owned(),
                group: Some("dev".to_owned()),
                team_id: None,
                allowed_models: None,
                allowed_providers: None,
                expires_at: None,
            })
            .unwrap();

        store.revoke_api_key(&created.public.id).unwrap();
        assert_eq!(store.active_api_key_count("usr_test"), 0);

        let updated = store
            .update_api_key(
                &created.public.id,
                UpdateApiKeyInput {
                    name: Some("local restored".to_owned()),
                    group: Some(String::new()),
                    team_id: None,
                    allowed_models: Some(vec!["mimo*".to_owned()]),
                    allowed_providers: Some(vec!["mimo".to_owned()]),
                    expires_at: None,
                    status: Some("active".to_owned()),
                    ip_restricted: Some(true),
                    allowed_ips: Some(vec!["127.0.0.1".to_owned(), "10.0.0.0/8".to_owned()]),
                    spend_limit_usd: Some(20.0),
                    rate_limited: Some(true),
                    five_hour_limit_usd: Some(0.0),
                    daily_limit_usd: Some(5.0),
                    weekly_limit_usd: Some(25.0),
                    monthly_limit_usd: Some(100.0),
                },
            )
            .unwrap();

        assert_eq!(updated.name, "local restored");
        assert_eq!(updated.group, None);
        assert_eq!(updated.allowed_models, vec!["mimo*"]);
        assert_eq!(updated.allowed_providers, vec!["mimo"]);
        assert_eq!(updated.status, "active");
        assert!(updated.ip_restricted);
        assert_eq!(updated.allowed_ips, vec!["127.0.0.1", "10.0.0.0/8"]);
        assert_eq!(updated.daily_limit_usd, 5.0);
        assert_eq!(store.active_api_key_count("usr_test"), 1);
    }

    #[test]
    fn api_key_ip_allowlist_is_enforced() {
        let store = ControlStore::for_tests();
        let identity = ClientIdentity {
            user_id: "usr_test".to_owned(),
            username: "test-user".to_owned(),
            api_key_id: Some("key_test".to_owned()),
            api_key_name: Some("local".to_owned()),
            api_key_group: Some("test".to_owned()),
            team_id: None,
            team_name: None,
            enforce_quotas: true,
            api_key_policy: ApiKeyPolicy {
                ip_restricted: true,
                allowed_ips: vec!["10.0.0.0/8".to_owned(), "127.0.0.1".to_owned()],
                ..ApiKeyPolicy::default()
            },
        };

        store
            .check_quotas(
                &identity,
                Some("10.1.2.3"),
                "mimo-v2.5-pro",
                "mimo-v2.5-pro",
                "mimo",
            )
            .unwrap();
        assert!(
            store
                .check_quotas(
                    &identity,
                    Some("192.168.1.10"),
                    "mimo-v2.5-pro",
                    "mimo-v2.5-pro",
                    "mimo",
                )
                .is_err()
        );
    }

    #[test]
    fn backup_validation_rejects_malformed_control_records() {
        let malformed = json!({
            "apiKeys": [{ "id": "key_incomplete" }]
        });

        assert!(validate_backup_document(&malformed).is_err());
    }

    #[test]
    fn team_model_and_provider_policy_is_enforced() {
        let store = ControlStore::for_tests();
        let team = store
            .upsert_team(UpsertTeamInput {
                id: Some("team_prod".to_owned()),
                name: "Prod".to_owned(),
                slug: Some("prod".to_owned()),
                description: None,
                status: Some("active".to_owned()),
                daily_limit_usd: Some(0.0),
                monthly_limit_usd: Some(0.0),
                allowed_models: Some(vec!["mimo*".to_owned()]),
                allowed_providers: Some(vec!["mimo".to_owned()]),
            })
            .unwrap();
        assert_eq!(team["slug"], "prod");
        let created = store
            .create_api_key(CreateApiKeyInput {
                user_id: "usr_test".to_owned(),
                username: Some("test-user".to_owned()),
                name: "team key".to_owned(),
                group: None,
                team_id: Some("team_prod".to_owned()),
                allowed_models: None,
                allowed_providers: None,
                expires_at: None,
            })
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&created.key).unwrap());
        let identity = store.authenticate_headers(&headers).unwrap().unwrap();

        store
            .check_quotas(&identity, None, "mimo-v2.5-pro", "mimo-v2.5-pro", "mimo")
            .unwrap();
        assert!(
            store
                .check_quotas(&identity, None, "gpt-5", "gpt-5", "openai",)
                .is_err()
        );
    }

    #[test]
    fn team_with_referencing_api_keys_cannot_be_deleted() {
        let store = ControlStore::for_tests();
        store
            .upsert_team(UpsertTeamInput {
                id: Some("team_safe".to_owned()),
                name: "Safe".to_owned(),
                slug: Some("safe".to_owned()),
                description: None,
                status: Some("active".to_owned()),
                daily_limit_usd: Some(1.0),
                monthly_limit_usd: Some(10.0),
                allowed_models: Some(vec!["mimo*".to_owned()]),
                allowed_providers: Some(vec!["mimo".to_owned()]),
            })
            .unwrap();
        let key = store
            .create_api_key(CreateApiKeyInput {
                user_id: "usr_test".to_owned(),
                username: Some("test-user".to_owned()),
                name: "team key".to_owned(),
                group: None,
                team_id: Some("team_safe".to_owned()),
                allowed_models: None,
                allowed_providers: None,
                expires_at: None,
            })
            .unwrap();

        assert!(store.delete_team("team_safe").is_err());
        store.delete_api_key(&key.public.id).unwrap();
        store.delete_team("team_safe").unwrap();
        assert!(store.list_teams().is_empty());
    }

    #[test]
    fn provider_health_marks_insufficient_balance_for_recharge() {
        let row = provider_health_row(
            &ProviderHealthRecord {
                provider_id: "deepseek".to_owned(),
                requests_total: 1,
                failures_total: 1,
                consecutive_failures: 1,
                last_error: Some(
                    r#"upstream returned HTTP 402: {"error":{"message":"Insufficient Balance"}}"#
                        .to_owned(),
                ),
                last_status_code: Some(402),
                ..ProviderHealthRecord::default()
            },
            now_millis(),
        );

        assert_eq!(row["failureKind"], "account");
        assert_eq!(row["accountIssue"], "insufficient_balance");
        assert_eq!(row["rechargeRequired"], true);
        assert_eq!(row["rechargeBadge"], "等待充值");
        assert!(
            row["recommendedAction"]
                .as_str()
                .is_some_and(|value| value.contains("充值后重试"))
        );
    }

    #[test]
    fn provider_health_does_not_mark_auth_error_for_recharge() {
        let row = provider_health_row(
            &ProviderHealthRecord {
                provider_id: "deepseek".to_owned(),
                requests_total: 1,
                failures_total: 1,
                consecutive_failures: 1,
                last_error: Some("upstream returned HTTP 401: invalid api key".to_owned()),
                last_status_code: Some(401),
                ..ProviderHealthRecord::default()
            },
            now_millis(),
        );

        assert_eq!(row["failureKind"], "account");
        assert_eq!(row["accountIssue"], "auth");
        assert_eq!(row["rechargeRequired"], false);
        assert!(row["rechargeBadge"].is_null());
    }

    #[test]
    fn credential_health_marks_insufficient_balance_for_recharge() {
        let row = provider_credential_health_row(
            &ProviderCredentialHealthRecord {
                provider_id: "deepseek".to_owned(),
                credential_id: "main".to_owned(),
                requests_total: 1,
                failures_total: 1,
                consecutive_failures: 1,
                last_error: Some("余额不足，请充值后重试".to_owned()),
                last_status_code: Some(402),
                ..ProviderCredentialHealthRecord::default()
            },
            now_millis(),
        );

        assert_eq!(row["accountIssue"], "insufficient_balance");
        assert_eq!(row["rechargeRequired"], true);
        assert_eq!(row["rechargeBadge"], "等待充值");
    }

    #[test]
    fn account_failure_sets_longer_credential_cooldown() {
        let mut inner = ControlInner::default();
        record_provider_credential_health_locked(
            &mut inner,
            "deepseek",
            "main",
            ProviderHealthUpdate {
                success: false,
                status_code: 402,
                error_message: Some("Insufficient Balance"),
                failure_kind: "account",
                now: 1_000,
            },
        );

        let cooldown_until = inner
            .provider_credential_health
            .get("deepseek")
            .and_then(|items| items.get("main"))
            .and_then(|record| record.cooldown_until_ms)
            .unwrap();
        assert!(
            cooldown_until
                >= 1_000
                    + crate::provider_status::ACCOUNT_ISSUE_CREDENTIAL_COOLDOWN_SECONDS * 1_000
        );
    }

    #[test]
    fn client_request_errors_do_not_open_provider_or_credential_circuit() {
        let mut inner = ControlInner::default();
        for now in 1_000..1_003 {
            let failure_kind = record_provider_health_locked(
                &mut inner,
                "provider-a",
                false,
                400,
                Some("invalid request payload"),
                now,
            );
            record_provider_credential_health_locked(
                &mut inner,
                "provider-a",
                "credential-a",
                ProviderHealthUpdate {
                    success: false,
                    status_code: 400,
                    error_message: Some("invalid request payload"),
                    failure_kind,
                    now,
                },
            );
        }

        assert!(
            inner
                .provider_health
                .get("provider-a")
                .is_some_and(
                    |health| health.cooldown_until_ms.is_none() && health.consecutive_failures == 0
                )
        );
        assert!(
            inner
                .provider_credential_health
                .get("provider-a")
                .and_then(|items| items.get("credential-a"))
                .is_some_and(
                    |health| health.cooldown_until_ms.is_none() && health.consecutive_failures == 0
                )
        );
    }

    #[test]
    fn route_alias_overrides_are_persistent_in_control_store() {
        let store = ControlStore::for_tests();
        let base_aliases = HashMap::from([("base".to_owned(), "mimo".to_owned())]);

        store
            .upsert_alias("fast".to_owned(), "mimo:mimo-v2.5-pro".to_owned())
            .unwrap();
        let aliases = store.effective_aliases(&base_aliases);
        assert_eq!(
            aliases.get("fast").map(String::as_str),
            Some("mimo:mimo-v2.5-pro")
        );
        assert_eq!(aliases.get("base").map(String::as_str), Some("mimo"));

        store.delete_alias("base", true).unwrap();
        let aliases = store.effective_aliases(&base_aliases);
        assert!(!aliases.contains_key("base"));
        assert_eq!(
            aliases.get("fast").map(String::as_str),
            Some("mimo:mimo-v2.5-pro")
        );
    }

    #[test]
    fn failed_provider_control_writes_restore_all_routing_state() {
        let seed = ControlStore::for_tests();
        seed.upsert_alias("fast".to_owned(), "provider-a:test-model".to_owned())
            .unwrap();
        seed.set_default_provider("provider-a".to_owned()).unwrap();
        seed.set_provider_order(vec!["provider-a".to_owned(), "provider-b".to_owned()])
            .unwrap();
        seed.upsert_provider_override(provider_override("provider-a", "Provider A"))
            .unwrap();
        seed.upsert_provider_model_override(ProviderModelOverrideRecord {
            provider_id: "provider-a".to_owned(),
            model: "test-model".to_owned(),
            status: "active".to_owned(),
            display_name: Some("Test Model".to_owned()),
            family: Some("test".to_owned()),
            context_window: Some(8_192),
            created_at_ms: 0,
            updated_at_ms: 0,
        })
        .unwrap();
        seed.upsert_provider_credential(provider_credential("provider-a", "credential-a"))
            .unwrap();
        seed.upsert_provider_credential(provider_credential("provider-a", "credential-b"))
            .unwrap();
        seed.set_active_provider_credential("provider-a", "credential-a")
            .unwrap();
        seed.set_provider_credential_pool_mode("provider-a", "round_robin")
            .unwrap();
        seed.record_provider_test(
            "provider-a".to_owned(),
            true,
            "reachable".to_owned(),
            vec!["test-model".to_owned()],
        )
        .unwrap();

        let path = failing_store_path("provider-control-write-failure");
        let store = ControlStore {
            store: Some(JsonStore::File(path.clone())),
            inner: Mutex::new(seed.inner.lock().unwrap().clone()),
            persistence_degraded: AtomicBool::new(false),
        };
        let expected = provider_routing_state(&store);

        macro_rules! assert_write_rolled_back {
            ($operation:expr) => {{
                assert!(matches!($operation, Err(AppError::Io(_))));
                assert_eq!(provider_routing_state(&store), expected);
            }};
        }

        assert_write_rolled_back!(
            store.upsert_alias("new-alias".to_owned(), "provider-b:model".to_owned())
        );
        assert!(matches!(store.health_check(), Err(AppError::NotReady(_))));
        assert_write_rolled_back!(store.delete_alias("fast", true));
        assert_write_rolled_back!(store.set_default_provider("provider-b".to_owned()));
        assert_write_rolled_back!(store.set_provider_order(vec!["provider-b".to_owned()]));
        assert_write_rolled_back!(
            store.upsert_provider_override(provider_override("provider-a", "Changed Provider"))
        );
        assert_write_rolled_back!(store.set_provider_disabled("provider-a", true));
        assert_write_rolled_back!(store.delete_provider("provider-a", true));
        assert_write_rolled_back!(store.upsert_provider_model_override(
            ProviderModelOverrideRecord {
                provider_id: "provider-a".to_owned(),
                model: "second-model".to_owned(),
                status: "active".to_owned(),
                display_name: None,
                family: None,
                context_window: None,
                created_at_ms: 0,
                updated_at_ms: 0,
            }
        ));
        assert_write_rolled_back!(store.delete_provider_model_override("provider-a", "test-model"));
        assert_write_rolled_back!(
            store.upsert_provider_credential(provider_credential("provider-a", "credential-c",))
        );
        assert_write_rolled_back!(store.set_provider_credential_pool_mode("provider-a", "manual"));
        assert_write_rolled_back!(
            store.set_active_provider_credential("provider-a", "credential-b")
        );
        assert_write_rolled_back!(store.delete_provider_credential("provider-a", "credential-a"));
        assert_write_rolled_back!(store.record_provider_test(
            "provider-a".to_owned(),
            false,
            "failed".to_owned(),
            Vec::new(),
        ));
        assert_write_rolled_back!(store.record_provider_outcome_for_credential(
            "provider-a",
            Some("credential-a"),
            false,
            401,
            Some("invalid API key"),
            true,
        ));
        assert!(matches!(store.health_check(), Err(AppError::NotReady(_))));

        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_control_writes_restore_all_security_mutations() {
        let path = failing_store_path("control-write-failure");
        let store = ControlStore {
            store: Some(JsonStore::File(path.clone())),
            inner: Mutex::new(ControlInner::default()),
            persistence_degraded: AtomicBool::new(false),
        };

        assert!(matches!(
            store.create_api_key(api_key_input(None)),
            Err(AppError::Io(_))
        ));
        assert!(store.inner.lock().unwrap().api_keys.is_empty());

        let seed = ControlStore::for_tests();
        let team_a = seed.upsert_team(team_input(None, "Alpha Team")).unwrap();
        let team_b = seed.upsert_team(team_input(None, "Beta Team")).unwrap();
        let team_a_id = team_a["id"].as_str().unwrap().to_owned();
        let team_b_id = team_b["id"].as_str().unwrap().to_owned();
        let key_id = seed
            .create_api_key(api_key_input(Some(team_a_id.clone())))
            .unwrap()
            .public
            .id;
        let quota_id = seed
            .upsert_quota(UpsertQuotaInput {
                id: None,
                user_id: "usr_test".to_owned(),
                username: "test-user".to_owned(),
                quota_type: "tokens".to_owned(),
                limit: 1_000.0,
                period: "monthly".to_owned(),
            })
            .unwrap()
            .id;
        *store.inner.lock().unwrap() = seed.inner.lock().unwrap().clone();

        assert!(matches!(
            store.upsert_team(team_input(Some(team_a_id.clone()), "Renamed Team")),
            Err(AppError::Io(_))
        ));
        {
            let inner = store.inner.lock().unwrap();
            assert_eq!(inner.teams[&team_a_id].name, "Alpha Team");
            assert_eq!(
                inner.api_keys[&key_id].team_name.as_deref(),
                Some("Alpha Team")
            );
        }

        assert!(matches!(
            store.delete_team(&team_b_id),
            Err(AppError::Io(_))
        ));
        assert!(store.inner.lock().unwrap().teams.contains_key(&team_b_id));

        assert!(matches!(
            store.revoke_api_key(&key_id),
            Err(AppError::Io(_))
        ));
        assert_eq!(
            store.inner.lock().unwrap().api_keys[&key_id].status,
            "active"
        );

        assert!(matches!(
            store.update_api_key(
                &key_id,
                UpdateApiKeyInput {
                    name: Some("renamed-key".to_owned()),
                    group: None,
                    team_id: None,
                    allowed_models: None,
                    allowed_providers: None,
                    expires_at: None,
                    status: None,
                    ip_restricted: None,
                    allowed_ips: None,
                    spend_limit_usd: None,
                    rate_limited: None,
                    five_hour_limit_usd: None,
                    daily_limit_usd: None,
                    weekly_limit_usd: None,
                    monthly_limit_usd: None,
                },
            ),
            Err(AppError::Io(_))
        ));
        assert_eq!(store.inner.lock().unwrap().api_keys[&key_id].name, "local");

        assert!(matches!(
            store.delete_api_key(&key_id),
            Err(AppError::Io(_))
        ));
        assert!(store.inner.lock().unwrap().api_keys.contains_key(&key_id));

        assert!(matches!(
            store.delete_user_resources("usr_test"),
            Err(AppError::Io(_))
        ));
        {
            let inner = store.inner.lock().unwrap();
            assert_eq!(inner.api_keys[&key_id].status, "active");
            assert!(inner.quotas.contains_key(&quota_id));
        }

        assert!(matches!(
            store.upsert_quota(UpsertQuotaInput {
                id: Some(quota_id.clone()),
                user_id: "usr_test".to_owned(),
                username: "test-user".to_owned(),
                quota_type: "tokens".to_owned(),
                limit: 2_000.0,
                period: "monthly".to_owned(),
            }),
            Err(AppError::Io(_))
        ));
        assert_eq!(store.inner.lock().unwrap().quotas[&quota_id].limit, 1_000.0);

        assert!(matches!(
            store.delete_quota(&quota_id),
            Err(AppError::Io(_))
        ));
        assert!(store.inner.lock().unwrap().quotas.contains_key(&quota_id));

        {
            let mut inner = store.inner.lock().unwrap();
            let quota = inner.quotas.get_mut(&quota_id).unwrap();
            quota.period_end_ms = 0;
            quota.reset_at_ms = 0;
        }
        assert!(matches!(store.list_quotas(), Err(AppError::Io(_))));
        {
            let inner = store.inner.lock().unwrap();
            let quota = &inner.quotas[&quota_id];
            assert_eq!(quota.period_end_ms, 0);
            assert_eq!(quota.reset_at_ms, 0);
        }
        assert!(matches!(store.health_check(), Err(AppError::NotReady(_))));

        std::fs::remove_dir_all(path).unwrap();
    }
}
