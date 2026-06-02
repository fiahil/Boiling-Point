//! The admin HTTP API: read endpoints over the projection (fleet, rooms, reveal,
//! replay, plus an SSE live feed) and the elevated command endpoints (reload,
//! toggle, room lifecycle). Everything is served under `/admin/*` on a listener
//! separate from the player WebSocket; the read path never mutates state and the
//! command path goes through the server's authoritative primitives (which emit
//! their own `admin.command` audit spans).

use std::convert::Infallible;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast;

use boiling_point_protocol::RoomCode;
use boiling_point_protocol::vocab::{EffectKind, ModifierKind};

use crate::lobby::registry::ContentSelector;

use super::projection::RevealOutcome;
use super::{AdminState, Elevated, Operator};

/// Assemble the admin Router with all routes bound to `state`.
pub fn router(state: AdminState) -> Router {
    Router::new()
        // Thin web app shell (static; the data it loads is gated below).
        .route("/admin/", get(index))
        .route("/admin/app.js", get(app_js))
        .route("/admin/style.css", get(style_css))
        // Read surfaces (any authenticated operator — observer or elevated).
        .route("/admin/me", get(me))
        .route("/admin/fleet", get(fleet))
        .route("/admin/rooms", get(rooms))
        .route("/admin/rooms/{code}", get(room_detail))
        .route("/admin/balance", get(balance))
        .route("/admin/replay", get(replay_list))
        .route("/admin/replay/{game_id}", get(replay_get))
        .route("/admin/live", get(live))
        // Hidden-state reveal — a read, served only over the admin channel.
        .route("/admin/rooms/{code}/reveal", get(reveal))
        // Command plane (elevated only).
        .route("/admin/commands/reload", post(cmd_reload))
        .route("/admin/commands/toggle", post(cmd_toggle))
        .route("/admin/commands/rooms/seed", post(cmd_seed))
        .route(
            "/admin/commands/rooms/{code}/force-start",
            post(cmd_force_start),
        )
        .route("/admin/commands/rooms/{code}/kill", post(cmd_kill))
        .with_state(state)
}

// ---- static web shell ----

async fn index() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("web/index.html"),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("web/app.js"),
    )
}

async fn style_css() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("web/style.css"),
    )
}

// ---- read endpoints (Operator: observer or elevated) ----

/// The authenticated operator's identity and role (so the web app can show what it
/// may do — the reveal is a read for any operator; control needs elevation).
async fn me(op: Operator) -> Response {
    Json(json!({ "name": op.name, "role": op.role })).into_response()
}

async fn fleet(_op: Operator, State(s): State<AdminState>) -> Response {
    Json(s.projection.fleet()).into_response()
}

async fn rooms(_op: Operator, State(s): State<AdminState>) -> Response {
    Json(s.projection.rooms()).into_response()
}

async fn room_detail(
    _op: Operator,
    State(s): State<AdminState>,
    Path(code): Path<String>,
) -> Response {
    match s.projection.room(&code) {
        Some(view) => Json(view).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no such live room" })),
        )
            .into_response(),
    }
}

async fn balance(_op: Operator, State(s): State<AdminState>) -> Response {
    Json(s.projection.balance()).into_response()
}

async fn replay_list(_op: Operator, State(s): State<AdminState>) -> Response {
    Json(s.projection.replay_list()).into_response()
}

async fn replay_get(
    _op: Operator,
    State(s): State<AdminState>,
    Path(game_id): Path<String>,
) -> Response {
    match s.projection.replay(&game_id) {
        Some(game) => Json(game).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "game no longer retained in the replay buffer" })),
        )
            .into_response(),
    }
}

/// The live activity feed as Server-Sent Events, served only over the authenticated
/// admin channel. It may carry sensitive span attributes; no player connection ever
/// reaches it.
async fn live(
    _op: Operator,
    State(s): State<AdminState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = s.projection.subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let event = Event::default()
                        .json_data(&ev)
                        .unwrap_or_else(|_| Event::default());
                    return Some((Ok(event), rx));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ---- reveal (any authenticated operator; never a player connection) ----

async fn reveal(_op: Operator, State(s): State<AdminState>, Path(code): Path<String>) -> Response {
    let outcome = s.projection.reveal(&code);
    if matches!(outcome, RevealOutcome::NoSuchRoom) {
        return (StatusCode::NOT_FOUND, Json(outcome)).into_response();
    }
    Json(outcome).into_response()
}

// ---- command plane (Elevated only) ----

/// One content item to enable/disable, mirroring [`ContentSelector`].
#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum SelectorReq {
    Effect(EffectKind),
    Modifier(ModifierKind),
    Emote(u16),
}

impl From<SelectorReq> for ContentSelector {
    fn from(req: SelectorReq) -> Self {
        match req {
            SelectorReq::Effect(k) => ContentSelector::Effect(k),
            SelectorReq::Modifier(k) => ContentSelector::Modifier(k),
            SelectorReq::Emote(id) => ContentSelector::Emote(id),
        }
    }
}

#[derive(Deserialize)]
struct ToggleReq {
    selector: SelectorReq,
    enabled: bool,
}

async fn cmd_reload(Elevated(op): Elevated, State(s): State<AdminState>, body: String) -> Response {
    match s.rooms.reload(&body, &op.name) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn cmd_toggle(
    Elevated(op): Elevated,
    State(s): State<AdminState>,
    Json(req): Json<ToggleReq>,
) -> Response {
    match s
        .rooms
        .toggle_item(req.selector.into(), req.enabled, &op.name)
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn cmd_seed(Elevated(op): Elevated, State(s): State<AdminState>) -> Response {
    let code = s.rooms.seed_room(&op.name);
    Json(json!({ "room_code": code.0 })).into_response()
}

async fn cmd_force_start(
    Elevated(op): Elevated,
    State(s): State<AdminState>,
    Path(code): Path<String>,
) -> Response {
    delivered_response(s.rooms.force_start(&RoomCode(code.clone()), &op.name), code)
}

async fn cmd_kill(
    Elevated(op): Elevated,
    State(s): State<AdminState>,
    Path(code): Path<String>,
) -> Response {
    delivered_response(s.rooms.kill_room(&RoomCode(code.clone()), &op.name), code)
}

/// Shape a room-lifecycle command's delivery result. The authoritative effect is
/// confirmed by telemetry (the room leaving the live registry), not this ack.
fn delivered_response(delivered: bool, code: String) -> Response {
    if delivered {
        Json(json!({ "delivered": true, "room_code": code })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "delivered": false, "error": "no such room" })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;
    use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
    use tower::ServiceExt;

    use crate::admin::{AdminProjection, AdminState, OperatorAuth, OperatorRole};
    use crate::config::ContentConfig;
    use crate::lobby::RoomRegistry;

    const ELEVATED: &str = "elev-token";
    const OBSERVER: &str = "obs-token";

    fn test_state() -> AdminState {
        let config = ContentConfig::from_toml(include_str!("../../content.toml")).unwrap();
        let registry = Arc::new(config.build_registry().unwrap());
        let rooms = Arc::new(RoomRegistry::new(registry, Arc::new(config)));
        let auth = OperatorAuth::new()
            .with_token(ELEVATED, "root", OperatorRole::Elevated)
            .with_token(OBSERVER, "watcher", OperatorRole::Observer);
        AdminState {
            projection: Arc::new(AdminProjection::new()),
            auth: Arc::new(auth),
            rooms,
        }
    }

    async fn status_of(req: Request<Body>) -> StatusCode {
        super::router(test_state())
            .oneshot(req)
            .await
            .unwrap()
            .status()
    }

    fn get(uri: &str, token: Option<&str>) -> Request<Body> {
        let mut b = Request::builder().method("GET").uri(uri);
        if let Some(t) = token {
            b = b.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        b.body(Body::empty()).unwrap()
    }

    fn post(uri: &str, token: Option<&str>, body: &str) -> Request<Body> {
        let mut b = Request::builder()
            .method("POST")
            .uri(uri)
            .header(CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            b = b.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    #[tokio::test]
    async fn missing_or_player_token_is_denied() {
        // No token at all.
        assert_eq!(
            status_of(get("/admin/fleet", None)).await,
            StatusCode::UNAUTHORIZED
        );
        // An anonymous "player session token" is not an operator token.
        assert_eq!(
            status_of(get("/admin/fleet", Some("anon-player-session-uuid"))).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn observer_may_reveal_but_is_denied_control() {
        // The reveal is a read: an observer is authorized (404 = no such live room,
        // but auth/role passed — not 403).
        assert_eq!(
            status_of(get("/admin/rooms/ABCD/reveal", Some(OBSERVER))).await,
            StatusCode::NOT_FOUND
        );
        // Control still requires elevation.
        assert_eq!(
            status_of(post("/admin/commands/rooms/seed", Some(OBSERVER), "")).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status_of(post("/admin/commands/reload", Some(OBSERVER), "")).await,
            StatusCode::FORBIDDEN
        );
    }

    /// 8.2: the reveal (and every admin endpoint) is unreachable on the player
    /// transport — even with a valid elevated token, the player wire exposes no
    /// such route. The hidden-state reveal can only be served on the admin channel.
    #[tokio::test]
    async fn admin_reveal_is_unreachable_on_the_player_transport() {
        use crate::lobby::{MatchQueue, SessionStore};
        use crate::transport::{AppState, app as player_app};

        let config = ContentConfig::from_toml(include_str!("../../content.toml")).unwrap();
        let registry = Arc::new(config.build_registry().unwrap());
        let rooms = Arc::new(RoomRegistry::new(registry, Arc::new(config)));
        let queue = Arc::new(MatchQueue::new(rooms.clone()));
        let state = AppState {
            sessions: Arc::new(SessionStore::new()),
            rooms,
            queue,
            conn_timeout: std::time::Duration::from_secs(90),
        };
        let resp = player_app(state)
            .oneshot(get("/admin/rooms/ABCD/reveal", Some(ELEVATED)))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "the player transport must expose no admin route"
        );
    }

    #[tokio::test]
    async fn observer_may_read_fleet() {
        assert_eq!(
            status_of(get("/admin/fleet", Some(OBSERVER))).await,
            StatusCode::OK
        );
        assert_eq!(
            status_of(get("/admin/rooms", Some(OBSERVER))).await,
            StatusCode::OK
        );
        assert_eq!(
            status_of(get("/admin/balance", Some(OBSERVER))).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn elevated_reaches_control() {
        // The only thing elevation adds over an observer is the command plane:
        // seeding a room is authorized and succeeds.
        assert_eq!(
            status_of(post("/admin/commands/rooms/seed", Some(ELEVATED), "")).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn me_reports_the_operator_role() {
        use axum::body::to_bytes;
        let resp = super::router(test_state())
            .oneshot(get("/admin/me", Some(OBSERVER)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["role"], "observer");
        assert_eq!(json["name"], "watcher");
    }

    #[tokio::test]
    async fn elevated_reload_validates_and_applies() {
        let state = test_state();
        let before = state.rooms.len();
        // A valid reload (the same content) is accepted.
        let resp = super::router(state.clone())
            .oneshot(post(
                "/admin/commands/reload",
                Some(ELEVATED),
                include_str!("../../content.toml"),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        // An invalid reload is rejected with 400 and does not change room count.
        let resp = super::router(state.clone())
            .oneshot(post(
                "/admin/commands/reload",
                Some(ELEVATED),
                "not valid toml {{{",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(state.rooms.len(), before);
    }

    /// 8.3: reading the projection over the API mutates no game or config state.
    #[tokio::test]
    async fn read_endpoints_do_not_mutate_state() {
        let state = test_state();
        let rooms_before = state.rooms.len();
        for uri in [
            "/admin/fleet",
            "/admin/rooms",
            "/admin/balance",
            "/admin/replay",
        ] {
            let resp = super::router(state.clone())
                .oneshot(get(uri, Some(OBSERVER)))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "GET {uri}");
        }
        assert_eq!(
            state.rooms.len(),
            rooms_before,
            "reads must not create/destroy rooms"
        );
    }
}
