use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde_json::{Value, json};

use crate::{domain::TenantScope, governance::ChangeRequestInput};

use super::*;

pub(super) async fn self_service_governance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let actor = require_console_user(&state, &headers)?;
    let tenant = TenantScope::legacy_local();
    let policy = state.governance.effective_policy(&tenant);
    Ok(Json(json!({
        "view": "user-self-service",
        "user": actor,
        "effectivePolicy": policy,
        "scheduler": state.local_scheduler.snapshot(),
        "capabilities": {
            "mayRestrictRoutingMode": true,
            "mayExpandRoutingMode": false,
            "maySubmitHighRiskChange": actor.role == "admin",
        }
    })))
}

pub(super) async fn admin_governance_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin_user(&state, &headers)?;
    Ok(Json(json!({
        "view": "administrator-control-plane",
        "ready": state.governance.is_ready(),
        "projectPolicies": state.governance.list_policies(),
        "changeRequests": state.governance.list_change_requests(),
        "scheduler": state.local_scheduler.snapshot(),
        "highRiskActions": [
            "project_policy.upsert",
            "provider.allowlist_change",
            "routing.cloud_first",
            "budget.hard_limit",
            "identity.permission",
            "model.production_promotion",
            "data_egress.change",
            "database.major_migration",
            "secret.rotation"
        ]
    })))
}

pub(super) async fn admin_change_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin_user(&state, &headers)?;
    Ok(Json(json!(state.governance.list_change_requests())))
}

pub(super) async fn admin_create_change_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ChangeRequestInput>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let change = state
        .governance
        .create_change_request(&actor.id, &actor.username, body)?;
    record_admin_activity(
        &state,
        &actor,
        "high_risk_change_requested",
        format!("change:{}", change.id),
        format!("提交高风险变更 {}，等待第二名管理员审批", change.action),
        "warning",
    )
    .await;
    Ok(Json(json!(change)))
}

pub(super) async fn admin_approve_change_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(change_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let change = state
        .governance
        .approve_change_request(&change_id, &actor.id, &actor.username)?;
    record_admin_activity(
        &state,
        &actor,
        "high_risk_change_approved",
        format!("change:{}", change.id),
        format!("第二人审批通过高风险变更 {}", change.action),
        "warning",
    )
    .await;
    Ok(Json(json!(change)))
}

pub(super) async fn admin_apply_change_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(change_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let change = state.governance.approved_change(&change_id)?;
    let result = match change.action.as_str() {
        "project_policy.upsert" => {
            json!(
                state
                    .governance
                    .apply_project_policy(&change_id, &actor.id)?
            )
        }
        "budget.hard_limit" => {
            let update: EnterpriseBudgetUpdate = serde_json::from_value(change.payload.clone())?;
            let view = state.ledger.update_budget(&update).await?;
            state.governance.mark_change_applied(&change_id)?;
            json!(view)
        }
        _ => {
            return Err(AppError::InvalidRequest(format!(
                "approved action {} must be applied by its dedicated production runbook",
                change.action
            )));
        }
    };
    record_admin_activity(
        &state,
        &actor,
        "high_risk_change_applied",
        format!("change:{}", change.id),
        format!("应用高风险变更 {}", change.action),
        "warning",
    )
    .await;
    Ok(Json(json!({
        "ok": true,
        "changeId": change.id,
        "action": change.action,
        "result": result,
    })))
}
