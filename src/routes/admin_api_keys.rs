use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde_json::{Value, json};

use crate::control::{CreateApiKeyInput, UpdateApiKeyInput};

use super::*;

pub(super) async fn admin_api_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let actor = require_console_user(&state, &headers)?;
    if actor.role == "user" {
        Ok(Json(json!(state.control.list_user_api_keys(&actor.id))))
    } else {
        Ok(Json(json!(state.control.list_api_keys())))
    }
}

pub(super) async fn admin_user_api_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let actor = require_console_user(&state, &headers)?;
    if actor.role == "user" && actor.id != user_id {
        return Err(AppError::Forbidden(
            "cannot read another user's API keys".to_owned(),
        ));
    }
    Ok(Json(json!(state.control.list_user_api_keys(&user_id))))
}

pub(super) async fn admin_create_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut body): Json<CreateApiKeyInput>,
) -> Result<Json<Value>, AppError> {
    let actor = require_api_key_write_user(&state, &headers)?;
    if actor.role != "admin" {
        if body.user_id != actor.id {
            return Err(AppError::Forbidden(
                "cannot create API keys for another user".to_owned(),
            ));
        }
        body.username = Some(actor.username.clone());
        body.team_id = None;
    }
    if body.username.is_none()
        && let Some(user) = state
            .auth
            .list_users(0)
            .into_iter()
            .find(|user| user.id == body.user_id)
    {
        body.username = Some(user.username);
    }
    let created = state.control.create_api_key(body)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("api_key:{}", created.public.id),
        format!(
            "为用户 {} 创建 API Key {}",
            created.public.username, created.public.name
        ),
        "info",
    );
    Ok(Json(json!(created)))
}

pub(super) async fn admin_revoke_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let actor = require_api_key_write_user(&state, &headers)?;
    ensure_api_key_access(&state, &actor, &key_id)?;
    state.control.revoke_api_key(&key_id)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("api_key:{key_id}"),
        format!("吊销 API Key {key_id}"),
        "warning",
    );
    Ok(Json(json!({ "ok": true })))
}

pub(super) async fn admin_update_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key_id): Path<String>,
    Json(body): Json<UpdateApiKeyInput>,
) -> Result<Json<Value>, AppError> {
    let actor = require_api_key_write_user(&state, &headers)?;
    ensure_api_key_access(&state, &actor, &key_id)?;
    let updated = state.control.update_api_key(&key_id, body)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("api_key:{key_id}"),
        format!("更新 API Key {} ({})", updated.name, updated.status),
        "info",
    );
    Ok(Json(json!(updated)))
}

pub(super) async fn admin_delete_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let actor = require_api_key_write_user(&state, &headers)?;
    ensure_api_key_access(&state, &actor, &key_id)?;
    state.control.delete_api_key(&key_id)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("api_key:{key_id}"),
        format!("删除 API Key {key_id}"),
        "warning",
    );
    Ok(Json(json!({ "ok": true })))
}
