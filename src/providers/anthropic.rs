use axum::{
    Json,
    http::HeaderMap,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::StreamExt;

use crate::{
    config::ResolvedProvider,
    error::AppError,
    http::{Header, SseFrame},
    pricing::{self, USAGE_HEADER},
    routes::AppState,
    types::{AnthropicRequest, anthropic_error_event, anthropic_request_value},
};

pub async fn messages(
    state: AppState,
    resolved: ResolvedProvider,
    request: AnthropicRequest,
    client_headers: &HeaderMap,
) -> Result<Response, AppError> {
    let body = anthropic_request_value(&request, &resolved.model)?;
    let headers = headers(&resolved.provider, client_headers)?;
    let url = resolved.provider.endpoint("/v1/messages");

    if request.stream.unwrap_or(false) {
        let frames = state.transport.post_json_sse(url, headers, body);
        let events = frames.map(|result| match result {
            Ok(frame) => Ok(frame_to_event(frame)),
            Err(err) => anthropic_error_event(&err),
        });
        Ok(Sse::new(events)
            .keep_alive(KeepAlive::default())
            .into_response())
    } else {
        let response = state.transport.post_json(&url, &headers, &body).await?;
        let usage = pricing::anthropic_usage(&response);
        let mut response = Json(response).into_response();
        response.headers_mut().insert(
            USAGE_HEADER,
            pricing::usage_header_value(&resolved.model, usage)?,
        );
        Ok(response)
    }
}

fn headers(
    provider: &crate::config::ProviderConfig,
    client_headers: &HeaderMap,
) -> Result<Vec<Header>, AppError> {
    let mut headers = Vec::new();

    if let Some(api_key) = provider.api_key()? {
        headers.push(("x-api-key".to_owned(), api_key.to_owned()));
    }

    headers.push((
        "anthropic-version".to_owned(),
        client_header(client_headers, "anthropic-version")
            .unwrap_or_else(|| "2023-06-01".to_owned()),
    ));

    for name in ["anthropic-beta", "x-request-id"] {
        if let Some(value) = client_header(client_headers, name) {
            headers.push((name.to_owned(), value));
        }
    }

    Ok(headers)
}

fn client_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn frame_to_event(frame: SseFrame) -> Event {
    let mut event = Event::default().data(frame.data);
    if let Some(name) = frame.event {
        event = event.event(name);
    }
    if let Some(id) = frame.id {
        event = event.id(id);
    }
    if let Some(retry) = frame.retry {
        event = event.retry(retry);
    }
    for comment in frame.comments {
        event = event.comment(comment);
    }
    event
}
