use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::*;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeleteProviderQuery {
    force: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProviderWriteBody {
    id: Option<String>,
    display_name: Option<String>,
    protocol: Option<String>,
    base_url: Option<String>,
    api_key_env: Option<String>,
    api_key_required: Option<bool>,
    default_model: Option<String>,
    models: Option<Vec<String>>,
    model_prefixes: Option<Vec<String>>,
    passthrough_unknown_models: Option<bool>,
    max_tokens_field: Option<String>,
    deduplicate_stream_text: Option<bool>,
    buffer_stream_text: Option<bool>,
    fidelity_mode: Option<String>,
    disabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProviderDisableBody {
    disabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProviderModelWriteBody {
    model: String,
    status: Option<String>,
    display_name: Option<String>,
    family: Option<String>,
    context_window: Option<u64>,
}

pub(super) async fn admin_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_console_user(&state, &headers)?;
    Ok(Json(Value::Array(provider_rows(&state))))
}

pub(super) async fn admin_create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ProviderWriteBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let provider_id = body
        .id
        .clone()
        .ok_or_else(|| AppError::InvalidRequest("provider id is required".to_owned()))?;
    let disabled = body.disabled.unwrap_or(false);
    let record = provider_body_to_record(&provider_id, body, None)?;
    let record = state.control.upsert_provider_override(record)?;
    state.control.set_provider_disabled(&record.id, disabled)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("provider:{}", record.id),
        format!("新增供应商 {}", record.id),
        "info",
    );
    Ok(Json(provider_row_by_id(&state, &record.id)?))
}

pub(super) async fn admin_update_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Json(body): Json<ProviderWriteBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let disabled = body.disabled;
    let current = management_config(&state)
        .providers
        .get(&provider_id)
        .cloned()
        .ok_or_else(|| AppError::ProviderNotFound(provider_id.clone()))?;
    let record = provider_body_to_record(&provider_id, body, Some(current))?;
    let record = state.control.upsert_provider_override(record)?;
    if let Some(disabled) = disabled {
        state.control.set_provider_disabled(&record.id, disabled)?;
    }
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("provider:{}", record.id),
        format!("更新供应商 {}", record.id),
        "info",
    );
    Ok(Json(provider_row_by_id(&state, &record.id)?))
}

pub(super) async fn admin_set_provider_disabled(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Json(body): Json<ProviderDisableBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    if !management_config(&state)
        .providers
        .contains_key(&provider_id)
    {
        return Err(AppError::ProviderNotFound(provider_id));
    }
    let disabled = body.disabled.unwrap_or(true);
    state
        .control
        .set_provider_disabled(&provider_id, disabled)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("provider:{provider_id}"),
        if disabled {
            format!("禁用供应商 {provider_id}")
        } else {
            format!("启用供应商 {provider_id}")
        },
        if disabled { "warning" } else { "info" },
    );
    Ok(Json(provider_row_by_id(&state, &provider_id)?))
}

pub(super) async fn admin_delete_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Query(query): Query<DeleteProviderQuery>,
) -> Result<Response, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let management = management_config(&state);
    if !management.providers.contains_key(&provider_id) {
        return Err(AppError::ProviderNotFound(provider_id));
    }
    let dependencies = provider_delete_dependencies(&state, &management, &provider_id);
    if !dependencies.is_empty() && !query.force.unwrap_or(false) {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "ok": false,
                "blocked": true,
                "providerId": provider_id,
                "message": "provider is still referenced; pass force=true after reviewing dependencies",
                "dependencies": dependencies,
            })),
        )
            .into_response());
    }

    for dependency in dependencies
        .iter()
        .filter(|dependency| dependency.get("type").and_then(Value::as_str) == Some("alias"))
    {
        if let Some(alias) = dependency.get("id").and_then(Value::as_str) {
            let tombstone = state.config.snapshot().aliases.contains_key(alias);
            state.control.delete_alias(alias, tombstone)?;
        }
    }

    let tombstone = state.config.snapshot().providers.contains_key(&provider_id);
    state.control.delete_provider(&provider_id, tombstone)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("provider:{provider_id}"),
        format!("删除供应商 {provider_id}"),
        "warning",
    );
    Ok(Json(json!({ "ok": true, "providerId": provider_id })).into_response())
}

pub(super) async fn admin_provider_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let config = management_config(&state);
    let Some(provider) = config.providers.get(&provider_id).cloned() else {
        return Err(AppError::ProviderNotFound(provider_id));
    };

    let (success, message, models) = match discover_provider_models(&state, &provider).await {
        Ok(models) => {
            let message = if provider.protocol == ProviderProtocol::OpenaiCompat {
                format!("discovered {} model(s)", models.len())
            } else {
                "model discovery is not available for this protocol; returned configured models"
                    .to_owned()
            };
            (true, message, models)
        }
        Err(err) => (false, err.to_string(), Vec::new()),
    };
    let tested_at = state.control.record_provider_test(
        provider_id.clone(),
        success,
        message.clone(),
        models.clone(),
    )?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("provider:{provider_id}"),
        format!("发现供应商 {provider_id} 模型: {message}"),
        if success { "info" } else { "warning" },
    );

    Ok(Json(json!({
        "providerId": provider_id,
        "success": success,
        "message": message,
        "models": models,
        "modelCount": models.len(),
        "discoveredAt": tested_at.to_string(),
    })))
}

pub(super) async fn admin_upsert_provider_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Json(body): Json<ProviderModelWriteBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let config = management_config(&state);
    let Some(provider) = config.providers.get(&provider_id) else {
        return Err(AppError::ProviderNotFound(provider_id));
    };
    let status = body.status.unwrap_or_else(|| "active".to_owned());
    if status == "disabled"
        && provider.default_model == body.model
        && provider
            .models
            .iter()
            .filter(|model| *model != &body.model)
            .count()
            == 0
    {
        return Err(AppError::InvalidRequest(
            "cannot disable the last/default model of an enabled provider; disable the provider instead"
                .to_owned(),
        ));
    }
    let record = state
        .control
        .upsert_provider_model_override(ProviderModelOverrideRecord {
            provider_id: provider_id.clone(),
            model: body.model,
            status,
            display_name: body.display_name,
            family: body.family,
            context_window: body.context_window,
            created_at_ms: 0,
            updated_at_ms: 0,
        })?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("provider:{provider_id}:model:{}", record.model),
        format!(
            "更新供应商 {provider_id} 模型 {} 为 {}",
            record.model, record.status
        ),
        if record.status == "disabled" {
            "warning"
        } else {
            "info"
        },
    );
    Ok(Json(json!({
        "ok": true,
        "providerId": provider_id,
        "model": provider_model_row(&record),
        "provider": provider_row_by_id(&state, &record.provider_id)?,
    })))
}

pub(super) async fn admin_delete_provider_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Json(body): Json<ProviderModelWriteBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_admin_write_user(&state, &headers)?;
    let config = management_config(&state);
    let Some(provider) = config.providers.get(&provider_id) else {
        return Err(AppError::ProviderNotFound(provider_id));
    };
    if provider.default_model == body.model
        && provider
            .models
            .iter()
            .filter(|model| *model != &body.model)
            .count()
            == 0
    {
        return Err(AppError::InvalidRequest(
            "cannot delete the last/default model of an enabled provider; disable the provider instead"
                .to_owned(),
        ));
    }
    let record = state
        .control
        .delete_provider_model_override(&provider_id, &body.model)?;
    record_admin_activity(
        &state,
        &actor,
        "config_change",
        format!("provider:{provider_id}:model:{}", record.model),
        format!("从供应商 {provider_id} 可路由列表移除模型 {}", record.model),
        "warning",
    );
    Ok(Json(json!({
        "ok": true,
        "providerId": provider_id,
        "model": provider_model_row(&record),
        "provider": provider_row_by_id(&state, &provider_id)?,
    })))
}

fn provider_body_to_record(
    provider_id: &str,
    body: ProviderWriteBody,
    current: Option<ProviderConfig>,
) -> Result<ProviderOverrideRecord, AppError> {
    let current_provider = current.as_ref();
    let id = provider_id.trim().to_ascii_lowercase();
    let display_name = body
        .display_name
        .or_else(|| current_provider.map(|provider| provider.display_name.clone()))
        .unwrap_or_else(|| id.clone());
    let protocol = body
        .protocol
        .or_else(|| {
            current_provider.map(|provider| provider_protocol_value(provider.protocol).to_owned())
        })
        .unwrap_or_else(|| "openai-compat".to_owned());
    parse_provider_protocol(&protocol)?;
    let base_url = body
        .base_url
        .or_else(|| current_provider.map(|provider| provider.base_url.clone()))
        .ok_or_else(|| AppError::InvalidRequest("baseUrl is required".to_owned()))?;
    let default_model = body
        .default_model
        .or_else(|| current_provider.map(|provider| provider.default_model.clone()))
        .ok_or_else(|| AppError::InvalidRequest("defaultModel is required".to_owned()))?;
    let mut models = body
        .models
        .or_else(|| current_provider.map(|provider| provider.models.clone()))
        .unwrap_or_default();
    if !models.contains(&default_model) {
        models.insert(0, default_model.clone());
    }
    let model_prefixes = body
        .model_prefixes
        .or_else(|| current_provider.map(|provider| provider.model_prefixes.clone()))
        .unwrap_or_default();
    let api_key_env = body
        .api_key_env
        .or_else(|| current_provider.and_then(|provider| provider.api_key_env.clone()));
    let max_tokens_field = body
        .max_tokens_field
        .or_else(|| {
            current_provider
                .map(|provider| max_tokens_field_value(provider.max_tokens_field).to_owned())
        })
        .unwrap_or_else(|| "max_completion_tokens".to_owned());
    parse_max_tokens_field(&max_tokens_field)?;
    let fidelity_mode = body
        .fidelity_mode
        .or_else(|| {
            current_provider.map(|provider| fidelity_mode_value(provider.fidelity_mode).to_owned())
        })
        .unwrap_or_else(|| "best_effort".to_owned());
    parse_fidelity_mode(&fidelity_mode)?;

    Ok(ProviderOverrideRecord {
        id,
        display_name,
        protocol,
        base_url,
        api_key_env,
        api_key_required: body
            .api_key_required
            .or_else(|| current_provider.map(|provider| provider.api_key_required))
            .unwrap_or(true),
        default_model,
        models,
        model_prefixes,
        passthrough_unknown_models: body
            .passthrough_unknown_models
            .or_else(|| current_provider.map(|provider| provider.passthrough_unknown_models))
            .unwrap_or(false),
        max_tokens_field,
        deduplicate_stream_text: body
            .deduplicate_stream_text
            .or_else(|| current_provider.map(|provider| provider.deduplicate_stream_text))
            .unwrap_or(false),
        buffer_stream_text: body
            .buffer_stream_text
            .or_else(|| current_provider.map(|provider| provider.buffer_stream_text))
            .unwrap_or(false),
        fidelity_mode,
        created_at_ms: 0,
        updated_at_ms: 0,
    })
}
