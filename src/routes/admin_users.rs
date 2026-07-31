use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde_json::{Value, json};

use crate::auth::{CreateUserInput, UpdateUserInput};

use super::*;

pub(super) async fn admin_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let actor = require_console_user(&state, &headers)?;
    let usage = state.ledger.management_usage().await?;
    let mut users = state.auth.list_users(0);
    if actor.role == "user" {
        users.retain(|user| user.id == actor.id);
    }
    for user in &mut users {
        user.api_key_count = state.control.active_api_key_count(&user.id);
        user.request_count_24h = usage.users_24h.get(&user.id).copied().unwrap_or(0);
    }
    Ok(Json(json!(users)))
}

pub(super) async fn admin_create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateUserInput>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let approval_id = require_high_risk_change(
        &state,
        &headers,
        "identity.permission",
        &format!("user:new:{}", body.username),
        &json!({
            "username": body.username.clone(),
            "email": body.email.clone(),
            "role": body.role.clone(),
            "status": body.status.clone(),
        }),
    )?;
    let auth = state.auth.clone();
    let user = tokio::task::spawn_blocking(move || auth.create_user(body))
        .await
        .map_err(|error| AppError::Config(format!("password worker failed: {error}")))??;
    state.governance.mark_change_applied(&approval_id)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("user:{}", user.id),
        format!("创建用户 {}", user.username),
        "info",
    )
    .await;
    Ok(Json(json!(user)))
}

pub(super) async fn admin_update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(body): Json<UpdateUserInput>,
) -> Result<Json<Value>, AppError> {
    let current_user = require_admin_write_user(&state, &headers)?;
    let approval_id = if body.role.is_some() || body.status.is_some() {
        Some(require_high_risk_change(
            &state,
            &headers,
            "identity.permission",
            &format!("user:{user_id}"),
            &json!({
                "role": body.role.clone(),
                "status": body.status.clone(),
            }),
        )?)
    } else {
        None
    };
    let was_inactive = state
        .auth
        .user_by_id(&user_id)
        .is_some_and(|user| user.status != "active");
    let reactivating = was_inactive && body.status.as_deref() == Some("active");
    if reactivating {
        // Re-enabling an account must not resurrect keys that were revoked
        // when it was disabled. Commit the fail-closed control mutation first.
        state.control.delete_user_resources(&user_id)?;
    }
    let auth = state.auth.clone();
    let update_user_id = user_id.clone();
    let current_user_id = current_user.id.clone();
    let user = tokio::task::spawn_blocking(move || {
        auth.update_user(&update_user_id, &current_user_id, body)
    })
    .await
    .map_err(|error| AppError::Config(format!("password worker failed: {error}")))??;
    if user.status != "active" {
        state.control.delete_user_resources(&user.id)?;
    }
    if let Some(approval_id) = approval_id {
        state.governance.mark_change_applied(&approval_id)?;
    }
    record_admin_activity(
        &state,
        &current_user,
        "config_change",
        format!("user:{user_id}"),
        format!("更新用户 {} ({})", user.username, user.role),
        "info",
    )
    .await;
    Ok(Json(json!(user)))
}

pub(super) async fn admin_delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let current_user = require_admin_write_user(&state, &headers)?;
    let approval_id = require_high_risk_change(
        &state,
        &headers,
        "identity.permission",
        &format!("user:{user_id}"),
        &json!({ "delete": true }),
    )?;
    state.auth.delete_user(&user_id, &current_user.id)?;
    state.control.delete_user_resources(&user_id)?;
    state.governance.mark_change_applied(&approval_id)?;
    record_admin_activity(
        &state,
        &current_user,
        "config_change",
        format!("user:{user_id}"),
        format!("删除用户 {user_id} 并回收相关资源"),
        "warning",
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}
