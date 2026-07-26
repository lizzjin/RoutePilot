use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header::CONTENT_TYPE},
    response::IntoResponse,
};
use serde_json::json;

use super::*;

const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

pub(super) async fn livez(State(state): State<AppState>) -> Json<serde_json::Value> {
    let started = Instant::now();
    state.metrics.record_route("livez", true, started.elapsed());
    Json(json!({
        "status": "ok",
        "service": "model-port",
        "build": crate::version::json(),
    }))
}

pub(super) async fn readyz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let started = Instant::now();
    let result = async {
        authenticate_client(&state, &headers)?;
        state
            .auth
            .health_check()
            .map_err(|error| AppError::NotReady(format!("auth storage: {error}")))?;
        state
            .control
            .health_check()
            .map_err(|error| AppError::NotReady(format!("control storage: {error}")))?;
        state
            .ledger
            .health_check()
            .await
            .map_err(|error| AppError::NotReady(format!("enterprise ledger: {error}")))?;
        let degraded_operations = state.metrics.degraded_ledger_operations();
        if !degraded_operations.is_empty() {
            return Err(AppError::NotReady(format!(
                "enterprise ledger operations degraded: {}",
                degraded_operations.join(", ")
            )));
        }
        Ok(Json(detailed_health_body(&state)))
    }
    .await;
    state
        .metrics
        .record_route("readyz", result.is_ok(), started.elapsed());
    result
}

pub(super) async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let started = Instant::now();
    let detailed = state.security.expose_detailed_public_health
        || authenticate_client(&state, &headers).is_ok();
    state
        .metrics
        .record_route("health", true, started.elapsed());
    if detailed {
        Json(detailed_health_body(&state))
    } else {
        Json(json!({
            "status": "ok",
            "service": "model-port",
            "build": crate::version::json(),
        }))
    }
}

fn detailed_health_body(state: &AppState) -> serde_json::Value {
    let provider_health = state.control.provider_health_rows();
    let config = effective_config(state);

    json!({
        "status": "ok",
        "service": "model-port",
        "build": crate::version::json(),
        "providers": config.provider_order,
        "storage": {
            "auth": state.auth.data_path(),
            "control": state.control.data_path(),
            "enterpriseLedger": state.ledger.location(),
            "status": "ready",
            "pendingFinalizers": state.finalizers.active(),
            "degradedLedgerOperations": state.metrics.degraded_ledger_operations(),
        },
        "providerHealth": provider_health,
    })
}

pub(super) async fn metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let started = Instant::now();

    if let Err(err) = authenticate_client(&state, &headers) {
        state
            .metrics
            .record_route("metrics", false, started.elapsed());
        return Err(err);
    }

    state
        .metrics
        .record_route("metrics", true, started.elapsed());
    let mut metrics = state.metrics.render_prometheus();
    metrics.push_str(
        "\n# HELP modelport_ledger_pending_finalizers Streaming ledger finalizers pending database commit.\n",
    );
    metrics.push_str("# TYPE modelport_ledger_pending_finalizers gauge\n");
    metrics.push_str(&format!(
        "modelport_ledger_pending_finalizers {}\n",
        state.finalizers.active()
    ));
    Ok(([(CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)], metrics))
}
