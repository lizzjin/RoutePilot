use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpsSeverity {
    #[serde(rename = "SEV-1")]
    Sev1,
    #[serde(rename = "SEV-2")]
    Sev2,
    #[serde(rename = "SEV-3")]
    Sev3,
    #[serde(rename = "SEV-4")]
    Sev4,
}

impl OpsSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sev1 => "SEV-1",
            Self::Sev2 => "SEV-2",
            Self::Sev3 => "SEV-3",
            Self::Sev4 => "SEV-4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpsIncidentStatus {
    Open,
    Acknowledged,
    Mitigating,
    Monitoring,
    Resolved,
    Suppressed,
}

impl OpsIncidentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Acknowledged => "acknowledged",
            Self::Mitigating => "mitigating",
            Self::Monitoring => "monitoring",
            Self::Resolved => "resolved",
            Self::Suppressed => "suppressed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsObservation {
    pub event_key: String,
    pub detector_type: String,
    pub severity: OpsSeverity,
    pub title: String,
    pub summary: String,
    pub affected_scope: Value,
    pub evidence: Value,
    pub observed_at_ms: u64,
    pub active: bool,
    pub recovery_criteria: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsHeartbeat {
    pub instance_id: String,
    pub agent_version: String,
    pub mode: String,
    pub rule_set_version: String,
    pub observed_at_ms: u64,
    pub queue_depth: u64,
    pub interval_seconds: u64,
    #[serde(default)]
    pub analysis_enabled: bool,
    #[serde(default)]
    pub selected_model: Option<String>,
    #[serde(default = "default_disabled")]
    pub model_status: String,
    #[serde(default)]
    pub model_last_success_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAgentConfiguration {
    pub enabled: bool,
    pub analysis_enabled: bool,
    pub selected_model: Option<String>,
    pub prefer_local: bool,
    pub model_ready: bool,
    pub selected_model_local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsModelCandidate {
    pub id: String,
    pub provider_id: String,
    pub model: String,
    pub display_name: String,
    pub local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAgentConfigurationView {
    #[serde(flatten)]
    pub configuration: OpsAgentConfiguration,
    pub recommended_model: Option<String>,
    pub candidates: Vec<OpsModelCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAgentConfigurationUpdate {
    pub enabled: bool,
    pub analysis_enabled: bool,
    pub selected_model: Option<String>,
    #[serde(default = "default_true")]
    pub prefer_local: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsRequestWindow {
    pub window_seconds: u64,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub server_errors: u64,
    pub rate_limited: u64,
    pub protocol_failures: u64,
    pub stream_failures: u64,
    pub average_latency_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsLedgerHealth {
    pub unreconciled_requests: u64,
    pub open_usage_reservations: u64,
    pub oldest_open_reservation_age_ms: u64,
    pub budget_accounts_at_or_above_80_percent: u64,
    pub budget_accounts_exhausted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsSnapshot {
    pub captured_at_ms: u64,
    pub gateway_ready: bool,
    pub database_ready: bool,
    pub auth_ready: bool,
    pub control_ready: bool,
    pub governance_ready: bool,
    pub draining: bool,
    pub pending_finalizers: u64,
    pub degraded_ledger_operations: Vec<String>,
    pub provider_health: BTreeMap<String, Value>,
    pub requests: OpsRequestWindow,
    pub ledger: OpsLedgerHealth,
    pub recent_change_at_ms: Option<u64>,
    #[serde(default)]
    pub agent_configuration: OpsAgentConfiguration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsIncidentSummary {
    pub id: String,
    pub event_key: String,
    pub detector_type: String,
    pub severity: OpsSeverity,
    pub status: OpsIncidentStatus,
    pub title: String,
    pub summary: String,
    pub affected_scope: Value,
    pub recovery_criteria: String,
    pub first_seen_at_ms: u64,
    pub last_seen_at_ms: u64,
    pub resolved_at_ms: Option<u64>,
    pub occurrence_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsIncidentEvidence {
    pub id: String,
    pub incident_id: String,
    pub observed_at_ms: u64,
    pub evidence: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsIncidentTimelineEntry {
    pub id: String,
    pub incident_id: String,
    pub event_type: String,
    pub actor_id: String,
    pub actor_name: String,
    pub message: String,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsIncidentDetail {
    #[serde(flatten)]
    pub incident: OpsIncidentSummary,
    pub evidence: Vec<OpsIncidentEvidence>,
    pub timeline: Vec<OpsIncidentTimelineEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAgentSummary {
    pub instance_id: String,
    pub agent_version: String,
    pub mode: String,
    pub rule_set_version: String,
    pub observed_at_ms: u64,
    pub queue_depth: u64,
    pub interval_seconds: u64,
    pub online: bool,
    pub analysis_enabled: bool,
    pub selected_model: Option<String>,
    pub model_status: String,
    pub model_last_success_at_ms: Option<u64>,
}

fn default_true() -> bool {
    true
}

fn default_disabled() -> String {
    "disabled".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsIncidentList {
    pub items: Vec<OpsIncidentSummary>,
    pub total: u64,
    pub open: u64,
    pub highest_open_severity: Option<OpsSeverity>,
    pub agents: Vec<OpsAgentSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsIncidentStatusUpdate {
    pub status: OpsIncidentStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsIncidentFeedbackInput {
    pub outcome: String,
    pub root_cause_correct: Option<bool>,
    pub recommendation_adopted: Option<bool>,
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incident_contract_uses_stable_external_values() {
        assert_eq!(serde_json::to_value(OpsSeverity::Sev2).unwrap(), "SEV-2");
        assert_eq!(
            serde_json::to_value(OpsIncidentStatus::Acknowledged).unwrap(),
            "acknowledged"
        );
    }
}
