//! The admin surface: a **read projection** of the span stream plus a **narrow
//! command channel** — deployed on routes/a port distinct from the player
//! WebSocket and never sharing the player `protocol/` wire (`admin-auth`,
//! Constitution I).
//!
//! - [`projection`] is the read model, built solely by consuming the span
//!   lifecycle (`admin-span-projection`). It is read-only by construction.
//! - [`auth`] gates every capability by operator role, separate from player
//!   session tokens.
//! - [`api`] serves the read endpoints (fleet/rooms/reveal/replay/live) and the
//!   command endpoints (reload/toggle/room lifecycle) over isolated routes.
//!
//! All reads — including the hidden-state reveal — are reachable by any
//! authenticated operator over this admin channel (never a player connection); only
//! the control actions require the elevated role.

pub mod api;
pub mod auth;
pub mod projection;

use std::sync::Arc;

use axum::Router;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::lobby::RoomRegistry;

pub use auth::{Operator, OperatorAuth, OperatorRole};
pub use projection::AdminProjection;

/// Shared state for the admin API. Distinct from the player `AppState`: it carries
/// the read projection, the operator auth policy, and the command-plane handle to
/// the room registry — but **no** player session/transport state.
#[derive(Clone)]
pub struct AdminState {
    /// The span-sourced read projection.
    pub projection: Arc<AdminProjection>,
    /// The operator auth policy.
    pub auth: Arc<OperatorAuth>,
    /// The room registry — the command plane's authoritative target (never the
    /// player wire). Used only by the elevated command endpoints.
    pub rooms: Arc<RoomRegistry>,
}

/// Build the admin Router. Mount this on a listener **separate** from the player
/// WebSocket (a different port), so a player connection can never reach it.
pub fn app(state: AdminState) -> Router {
    api::router(state)
}

/// Extract the bearer token from the `Authorization` header, if present.
fn bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

impl FromRequestParts<AdminState> for Operator {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AdminState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts)
            .ok_or((StatusCode::UNAUTHORIZED, "missing operator bearer token"))?;
        state
            .auth
            .authenticate(token)
            .ok_or((StatusCode::UNAUTHORIZED, "invalid operator token"))
    }
}

/// An extractor that additionally requires the elevated role — the control actions
/// use it, so an observer token is rejected with `403`.
pub struct Elevated(pub Operator);

impl FromRequestParts<AdminState> for Elevated {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AdminState,
    ) -> Result<Self, Self::Rejection> {
        let operator =
            <Operator as FromRequestParts<AdminState>>::from_request_parts(parts, state).await?;
        if operator.is_elevated() {
            Ok(Elevated(operator))
        } else {
            Err((StatusCode::FORBIDDEN, "elevated operator role required"))
        }
    }
}
