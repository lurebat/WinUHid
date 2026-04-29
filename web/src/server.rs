//! HTTP + WebSocket surface for the WinUHid web UI.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Request, State};
use axum::http::{header, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::manager::{
    DeviceEvent, DeviceSummary, GenericDeviceParams, Manager, Ps4State, Ps5State, XOneState,
};

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/frontend"]
struct Frontend;

pub fn router(manager: Arc<Manager>, token: Option<String>) -> Router {
    let mut api = Router::new()
        .route("/api/health", get(health))
        .route("/api/devices", get(list_devices))
        .route("/api/devices/:id", delete(destroy_device))
        .route("/api/devices/:id/events", get(ws_events))
        .route("/api/devices/generic", post(create_generic))
        .route("/api/devices/:id/generic/input", post(generic_input))
        .route("/api/devices/mouse", post(create_mouse))
        .route("/api/devices/:id/mouse/motion", post(mouse_motion))
        .route("/api/devices/:id/mouse/button", post(mouse_button))
        .route("/api/devices/:id/mouse/scroll", post(mouse_scroll))
        .route("/api/devices/ps4", post(create_ps4))
        .route("/api/devices/:id/ps4/state", post(ps4_state))
        .route("/api/devices/ps5", post(create_ps5))
        .route("/api/devices/:id/ps5/state", post(ps5_state))
        .route("/api/devices/xone", post(create_xone))
        .route("/api/devices/:id/xone/state", post(xone_state));

    if let Some(t) = token {
        let token_state = Arc::new(t);
        api = api.layer(middleware::from_fn_with_state(token_state, auth_middleware));
    }

    Router::new()
        .route("/", get(index))
        .route("/static/*path", get(static_asset))
        .route("/favicon.ico", get(favicon))
        .merge(api)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(manager)
}

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------

/// Token gate applied to `/api/*` when `--token` is configured.
///
/// Accepts the token in either an `Authorization: Bearer <token>`
/// header (preferred for REST) or a `?token=<token>` query parameter
/// (the only option for WebSocket upgrades, since browsers can't set
/// custom headers on `WebSocket`). On mismatch or absence, returns
/// `401 Unauthorized` without leaking which one was wrong.
async fn auth_middleware(
    State(expected): State<Arc<String>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided = extract_bearer(req.headers().get(header::AUTHORIZATION))
        .or_else(|| extract_query_token(req.uri()));
    match provided {
        Some(p) if p == *expected => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn extract_bearer(header_value: Option<&header::HeaderValue>) -> Option<String> {
    let raw = header_value?.to_str().ok()?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    Some(token.trim().to_string())
}

fn extract_query_token(uri: &Uri) -> Option<String> {
    let query = uri.query()?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next()?;
        if key == "token" {
            let value = kv.next().unwrap_or("");
            return Some(percent_decode(value));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let (Some(h), Some(l)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) {
                    out.push((h << 4) | l);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Static asset serving (frontend baked into the binary).
// ---------------------------------------------------------------------------

async fn index() -> Response {
    serve_embedded("index.html")
}

async fn favicon() -> Response {
    if Frontend::get("favicon.ico").is_some() {
        serve_embedded("favicon.ico")
    } else {
        StatusCode::NO_CONTENT.into_response()
    }
}

async fn static_asset(Path(path): Path<String>) -> Response {
    serve_embedded(&path)
}

fn serve_embedded(path: &str) -> Response {
    match Frontend::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

// ---------------------------------------------------------------------------
// REST handlers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Health {
    driver_version: u32,
    devs_available: bool,
}

async fn health(State(m): State<Arc<Manager>>) -> Json<Health> {
    Json(Health {
        driver_version: m.driver_version(),
        devs_available: m.devs_available(),
    })
}

async fn list_devices(State(m): State<Arc<Manager>>) -> Json<Vec<DeviceSummary>> {
    Json(m.list())
}

async fn destroy_device(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.destroy(id)?;
    Ok(Json(json!({ "ok": true })))
}

async fn create_generic(
    State(m): State<Arc<Manager>>,
    Json(params): Json<GenericDeviceParams>,
) -> Result<Json<DeviceSummary>, AppError> {
    Ok(Json(m.create_generic(params)?))
}

#[derive(Deserialize)]
struct GenericInput {
    hex: String,
}
async fn generic_input(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    Json(body): Json<GenericInput>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.submit_generic_input(id, &body.hex)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize, Default)]
struct NamedRequest {
    name: Option<String>,
}

async fn create_mouse(
    State(m): State<Arc<Manager>>,
    body: Option<Json<NamedRequest>>,
) -> Result<Json<DeviceSummary>, AppError> {
    let name = body.and_then(|Json(b)| b.name);
    Ok(Json(m.create_mouse(name)?))
}

#[derive(Deserialize)]
struct MouseMotion {
    dx: i16,
    dy: i16,
}
async fn mouse_motion(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    Json(body): Json<MouseMotion>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.mouse_motion(id, body.dx, body.dy)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MouseButton {
    button: u8,
    down: bool,
}
async fn mouse_button(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    Json(body): Json<MouseButton>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.mouse_button(id, body.button, body.down)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MouseScroll {
    value: i16,
    #[serde(default)]
    horizontal: bool,
}
async fn mouse_scroll(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    Json(body): Json<MouseScroll>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.mouse_scroll(id, body.value, body.horizontal)?;
    Ok(Json(json!({ "ok": true })))
}

async fn create_ps4(
    State(m): State<Arc<Manager>>,
    body: Option<Json<NamedRequest>>,
) -> Result<Json<DeviceSummary>, AppError> {
    let name = body.and_then(|Json(b)| b.name);
    Ok(Json(m.create_ps4(name)?))
}

async fn ps4_state(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    Json(state): Json<Ps4State>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.submit_ps4_state(id, &state)?;
    Ok(Json(json!({ "ok": true })))
}

async fn create_ps5(
    State(m): State<Arc<Manager>>,
    body: Option<Json<NamedRequest>>,
) -> Result<Json<DeviceSummary>, AppError> {
    let name = body.and_then(|Json(b)| b.name);
    Ok(Json(m.create_ps5(name)?))
}

async fn ps5_state(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    Json(state): Json<Ps5State>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.submit_ps5_state(id, &state)?;
    Ok(Json(json!({ "ok": true })))
}

async fn create_xone(
    State(m): State<Arc<Manager>>,
    body: Option<Json<NamedRequest>>,
) -> Result<Json<DeviceSummary>, AppError> {
    let name = body.and_then(|Json(b)| b.name);
    Ok(Json(m.create_xone(name)?))
}

async fn xone_state(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    Json(state): Json<XOneState>,
) -> Result<Json<serde_json::Value>, AppError> {
    m.submit_xone_state(id, &state)?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// WebSocket: per-device event stream
// ---------------------------------------------------------------------------

async fn ws_events(
    State(m): State<Arc<Manager>>,
    Path(id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(dev) = m.get(id) else {
        return (StatusCode::NOT_FOUND, "no such device").into_response();
    };
    let rx = dev.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: tokio::sync::broadcast::Receiver<DeviceEvent>,
) {
    loop {
        tokio::select! {
            biased;

            // If the client sends anything (or disconnects), bail out
            // promptly. We don't actually act on incoming messages
            // beyond using them as a liveness signal.
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => continue,
                }
            }
            ev = rx.recv() => {
                match ev {
                    Ok(ev) => {
                        let payload = match serde_json::to_string(&ev) {
                            Ok(s) => s,
                            Err(e) => { tracing::warn!("ws serialize: {e}"); continue; }
                        };
                        if socket.send(Message::Text(payload)).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }
    let _ = socket.close().await;
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

pub struct AppError(pub anyhow::Error);

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("{:#}", self.0) })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_bearer, extract_query_token, percent_decode};
    use axum::http::{header::HeaderValue, Uri};

    #[test]
    fn bearer_header_extracts_token() {
        let h = HeaderValue::from_static("Bearer abc.123");
        assert_eq!(extract_bearer(Some(&h)), Some("abc.123".to_string()));
    }

    #[test]
    fn bearer_header_is_case_insensitive_on_scheme() {
        let h = HeaderValue::from_static("bearer abc.123");
        assert_eq!(extract_bearer(Some(&h)), Some("abc.123".to_string()));
    }

    #[test]
    fn bearer_header_missing_or_wrong_scheme() {
        assert_eq!(extract_bearer(None), None);
        let h = HeaderValue::from_static("Basic abc");
        assert_eq!(extract_bearer(Some(&h)), None);
    }

    #[test]
    fn query_token_extracts_when_present() {
        let uri: Uri = "/api/devices?token=abc".parse().unwrap();
        assert_eq!(extract_query_token(&uri), Some("abc".to_string()));
    }

    #[test]
    fn query_token_extracts_among_other_params() {
        let uri: Uri = "/api/devices?foo=bar&token=abc&baz=qux".parse().unwrap();
        assert_eq!(extract_query_token(&uri), Some("abc".to_string()));
    }

    #[test]
    fn query_token_percent_decoded() {
        let uri: Uri = "/api/devices?token=a%2Fb%20c".parse().unwrap();
        assert_eq!(extract_query_token(&uri), Some("a/b c".to_string()));
    }

    #[test]
    fn query_token_absent() {
        let uri: Uri = "/api/devices".parse().unwrap();
        assert_eq!(extract_query_token(&uri), None);
        let uri: Uri = "/api/devices?foo=bar".parse().unwrap();
        assert_eq!(extract_query_token(&uri), None);
    }

    #[test]
    fn percent_decode_handles_plus_and_invalid_escapes() {
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%2g"), "%2g");
    }
}
