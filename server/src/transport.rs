//! WebSocket transport: the Axum app, the per-connection read/write tasks, the
//! protocol-version handshake, and per-connection rate limiting.
//!
//! Each connection owns an outbound mpsc channel the group writes to; a writer
//! task serialises those messages to the socket as MessagePack. The first client
//! message must be an entry message (`CreateGroup`/`JoinGroup`) carrying a
//! compatible protocol version, after which the connection is bridged to its
//! group and subsequent messages are forwarded as actions.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};

use boiling_point_protocol::client::PROTOCOL_VERSION;
use boiling_point_protocol::server::ErrorCode;
use boiling_point_protocol::{ClientMessage, PlayerId, ServerMessage, codec};

use crate::lobby::accounts::{Account, AccountError, SignInCredential};
use crate::lobby::group::GroupCommand;
use crate::lobby::{AccountStore, GroupRegistry, MatchQueue, SessionStore};
use crate::rating::RatingStore;

/// Minimum spacing between accepted actions on one connection.
const RATE_LIMIT: Duration = Duration::from_millis(100);

/// Shared application state handed to every connection.
#[derive(Clone)]
pub struct AppState {
    /// Anonymous session authentication.
    pub sessions: Arc<SessionStore>,
    /// The live-group registry.
    pub groups: Arc<GroupRegistry>,
    /// The auto-match queue.
    pub queue: Arc<MatchQueue>,
    /// Max silence on a connection before it's treated as disconnected (clients
    /// keep it alive with heartbeats).
    pub conn_timeout: Duration,
    /// Optional persistence pool. `None` ⇒ persistence is disabled (the server
    /// plays games normally and skips the post-game write). The groups receive
    /// it via [`GroupRegistry::with_pool`].
    pub pool: Option<PgPool>,
    /// The shared account store (capability `player-accounts`). The same `Arc`
    /// the registry and queue hold, so identity is consistent everywhere.
    pub accounts: Arc<AccountStore>,
    /// The shared rating store (capability `player-rating`).
    pub ratings: Arc<RatingStore>,
}

/// Build the Axum router for the WebSocket endpoint.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Read the next decodable client message from the socket, skipping non-binary
/// frames and returning `None` on close or error.
async fn next_client_message(stream: &mut SplitStream<WebSocket>) -> Option<ClientMessage> {
    while let Some(frame) = stream.next().await {
        match frame {
            Ok(Message::Binary(data)) => {
                if let Ok(msg) = codec::decode::<ClientMessage>(data.as_ref()) {
                    return Some(msg);
                }
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            _ => continue, // ping/pong/text are ignored
        }
    }
    None
}

/// The authenticated, durable identity of one connection (the "session"). It is
/// established once, on the first entry message, and outlives any single group the
/// connection binds to.
struct Session {
    /// Stable player identity for this connection.
    player: PlayerId,
    /// The session token, replayed to the client in `GroupJoined` so it can resume
    /// this identity after a socket drop.
    token: String,
    /// The group this connection is currently bound to, or `None` in the unbound
    /// "menu" state (after a `LeaveGroup`, or before the first bind).
    binding: Option<mpsc::Sender<GroupCommand>>,
}

/// Whether a message is a group **entry** message (binds the connection to a group).
fn is_entry(msg: &ClientMessage) -> bool {
    matches!(
        msg,
        ClientMessage::CreateGroup { .. }
            | ClientMessage::JoinGroup { .. }
            | ClientMessage::EnqueueMatch { .. }
    )
}

/// Whether a table/game action requires the connection to be bound to a group and
/// is subject to the per-connection rate limit.
fn is_table_action(msg: &ClientMessage) -> bool {
    matches!(
        msg,
        ClientMessage::CommitIngredient { .. }
            | ClientMessage::CastSpell { .. }
            | ClientMessage::CommitPass
            | ClientMessage::LockIn
            | ClientMessage::Emote { .. }
    )
}

/// Drive one WebSocket connection end to end as a durable **session** (group-model
/// D5): the connection authenticates once, then acts as a small router — entry
/// messages set its group binding, `LeaveGroup` clears it (back to the menu state),
/// table actions forward to the bound group, and the socket survives a game or
/// group ending. It closes only on transport drop / client close.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<ServerMessage>(64);

    // Writer task: serialise server messages onto the socket.
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let Ok(bytes) = codec::encode(&msg) else {
                break;
            };
            if sink.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
    });

    // Handshake: the first message must be an entry message; it negotiates the
    // version, authenticates the identity, and makes the first binding. On a
    // rejected handshake, drop the sender and let the writer drain (so the client
    // actually receives the `Error`) rather than aborting.
    let Some(entry) = next_client_message(&mut stream).await else {
        drop(out_tx);
        let _ = writer.await;
        return;
    };
    let Some(mut session) = handle_first_entry(&state, entry, &out_tx).await else {
        drop(out_tx);
        let _ = writer.await;
        return;
    };
    // An authenticated player connection is live from here until the socket
    // closes (the connected-players fleet gauge).
    crate::observability::metric::connection_opened();

    // Router loop with a heartbeat-driven idle timeout (a connection silent past
    // `conn_timeout` is treated as dropped). Table actions are rate-limited; entry,
    // leave, play-again, and heartbeat are not (they must not be silently dropped).
    let conn_timeout = state.conn_timeout;
    let mut last = Instant::now()
        .checked_sub(RATE_LIMIT)
        .unwrap_or_else(Instant::now);
    loop {
        let msg = match tokio::time::timeout(conn_timeout, next_client_message(&mut stream)).await {
            Ok(Some(msg)) => msg,
            Ok(None) => break, // client closed the connection
            Err(_) => break,   // no heartbeat within the window → disconnect
        };
        if is_table_action(&msg) {
            let now = Instant::now();
            if now.duration_since(last) < RATE_LIMIT {
                continue; // silently drop excess
            }
            last = now;
        }
        // `ws.message` span (span_schema::span::WS_MESSAGE): only the message *kind*
        // (the variant name) rides as a public attribute — never the payload.
        let _msg_span = tracing::info_span!("ws.message", ws.message_kind = message_kind(&msg));

        match msg {
            // Heartbeat is serviced in any state (bound or unbound), keeping a menu
            // connection alive without forwarding to a group.
            ClientMessage::Heartbeat => {
                let _ = out_tx.send(ServerMessage::Heartbeat).await;
            }
            // Account upgrades are serviced in any state and never forwarded to a
            // group: they bind the current player id without disrupting the session.
            _ if is_account_op(&msg) => {
                handle_account_op(&state, session.player, &msg, &out_tx).await;
            }
            // Re-entry: bind to a (different) group on the same socket. Rejected
            // while already bound — the client must `LeaveGroup` first.
            _ if is_entry(&msg) => {
                if session.binding.is_some() {
                    send_error(
                        &out_tx,
                        ErrorCode::WrongPhase,
                        "already in a group — leave it before joining another",
                    )
                    .await;
                } else if let Some(group_tx) = bind_entry(&state, msg, &session, &out_tx).await {
                    session.binding = Some(group_tx);
                }
            }
            // Leave the current group and return to the unbound menu state, keeping
            // the socket open.
            ClientMessage::LeaveGroup => match session.binding.take() {
                Some(group_tx) => {
                    let _ = group_tx
                        .send(GroupCommand::Leave {
                            player: session.player,
                        })
                        .await;
                    let _ = out_tx.send(ServerMessage::LeftGroup).await;
                }
                None => {
                    send_error(&out_tx, ErrorCode::WrongPhase, "not in a group").await;
                }
            },
            // Table/game actions (incl. `PlayAgain`) require a bound group.
            _ => match &session.binding {
                Some(group_tx) => {
                    if group_tx
                        .send(GroupCommand::Action {
                            player: session.player,
                            msg,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                None => {
                    send_error(&out_tx, ErrorCode::WrongPhase, "join a group first").await;
                }
            },
        }
    }

    if let Some(group_tx) = session.binding {
        let _ = group_tx
            .send(GroupCommand::Leave {
                player: session.player,
            })
            .await;
    }
    crate::observability::metric::connection_closed();
    writer.abort();
}

/// Handle the first entry message: validate the version, authenticate the identity
/// (establishing the durable session), and bind to the first group. Returns the new
/// [`Session`], or `None` after sending an error.
async fn handle_first_entry(
    state: &AppState,
    entry: ClientMessage,
    out: &mpsc::Sender<ServerMessage>,
) -> Option<Session> {
    let version = match &entry {
        ClientMessage::CreateGroup {
            protocol_version, ..
        }
        | ClientMessage::JoinGroup {
            protocol_version, ..
        }
        | ClientMessage::EnqueueMatch {
            protocol_version, ..
        } => *protocol_version,
        _ => {
            send_error(
                out,
                ErrorCode::WrongPhase,
                "expected CreateGroup, JoinGroup, or EnqueueMatch",
            )
            .await;
            return None;
        }
    };
    if !version_ok(version, out).await {
        return None;
    }
    let session_token = match &entry {
        ClientMessage::CreateGroup { session_token, .. }
        | ClientMessage::JoinGroup { session_token, .. }
        | ClientMessage::EnqueueMatch { session_token, .. } => session_token.as_deref(),
        _ => None,
    };
    let credential = match &entry {
        ClientMessage::CreateGroup {
            account_credential, ..
        }
        | ClientMessage::JoinGroup {
            account_credential, ..
        }
        | ClientMessage::EnqueueMatch {
            account_credential, ..
        } => account_credential.clone(),
        _ => None,
    };

    // Resolve the identity: a presented account credential **signs in** (adopting
    // the account's durable player id); otherwise authenticate anonymously (the
    // default). An anonymous player who already owns an account is recognised too.
    let (player, token, account) = match credential {
        Some(cred) => match state.accounts.sign_in(&SignInCredential::from(cred)).await {
            Ok(account) => {
                // Bind a session token to the account's durable player so a later
                // token-only reconnect still resolves to the same identity.
                let token = state.sessions.issue(account.player_id);
                (account.player_id, token, Some(account))
            }
            Err(AccountError::AuthFailed(msg)) => {
                send_error(out, ErrorCode::AuthFailed, &msg).await;
                return None;
            }
            Err(AccountError::Conflict) => {
                send_error(out, ErrorCode::AccountConflict, "account conflict").await;
                return None;
            }
        },
        None => {
            let (player, token) = state.sessions.authenticate(session_token);
            (player, token, state.accounts.account_for_player(player))
        }
    };

    // If this connection has an account, confirm it (no token to re-reveal on a
    // sign-in/resume) and send the rating readout — before binding, so a queued
    // (enqueue) connection gets it without waiting for a table to fill.
    if let Some(account) = account {
        send_account_intro(out, account, &state.ratings, None).await;
    }

    let session = Session {
        player,
        token,
        binding: None,
    };
    let group_tx = bind_entry(state, entry, &session, out).await?;
    Some(Session {
        binding: Some(group_tx),
        ..session
    })
}

/// Send the account confirmation and the rating readout to a connection. The
/// `account_token` is `Some` only for a freshly minted device account.
async fn send_account_intro(
    out: &mpsc::Sender<ServerMessage>,
    account: Account,
    ratings: &RatingStore,
    account_token: Option<String>,
) {
    let _ = out
        .send(ServerMessage::AccountEstablished {
            account_id: account.id,
            account_type: account.kind,
            player_id: account.player_id,
            account_token,
        })
        .await;
    let _ = out
        .send(ServerMessage::RatingUpdate {
            rating: ratings.view(account.id),
        })
        .await;
}

/// Handle an in-session account upgrade (`CreateDeviceAccount` / `LinkOAuth`):
/// bind the connection's **current** player id to a durable account (never
/// changing the identity), then confirm + send the rating readout. Errors are
/// reported and the connection continues anonymously.
async fn handle_account_op(
    state: &AppState,
    player: PlayerId,
    msg: &ClientMessage,
    out: &mpsc::Sender<ServerMessage>,
) {
    match msg {
        ClientMessage::CreateDeviceAccount => {
            match state.accounts.create_device_account(player).await {
                Ok((account, token)) => {
                    send_account_intro(out, account, &state.ratings, Some(token)).await;
                }
                Err(AccountError::Conflict) => {
                    send_error(
                        out,
                        ErrorCode::AccountConflict,
                        "this player already has an account",
                    )
                    .await;
                }
                Err(AccountError::AuthFailed(msg)) => {
                    send_error(out, ErrorCode::AuthFailed, &msg).await;
                }
            }
        }
        ClientMessage::LinkOAuth {
            provider,
            access_token,
        } => match state
            .accounts
            .link_oauth(player, *provider, access_token)
            .await
        {
            Ok(account) => {
                send_account_intro(out, account, &state.ratings, None).await;
            }
            Err(AccountError::AuthFailed(msg)) => {
                send_error(out, ErrorCode::AuthFailed, &msg).await;
            }
            Err(AccountError::Conflict) => {
                send_error(
                    out,
                    ErrorCode::AccountConflict,
                    "that account is already linked elsewhere — sign in instead",
                )
                .await;
            }
        },
        _ => {}
    }
}

/// Whether a message is an in-session account upgrade (handled regardless of
/// group binding, never forwarded to a group).
fn is_account_op(msg: &ClientMessage) -> bool {
    matches!(
        msg,
        ClientMessage::CreateDeviceAccount | ClientMessage::LinkOAuth { .. }
    )
}

/// Bind an established session to a group via an entry message (no version
/// renegotiation; identity reused). Returns the group's command channel, or `None`
/// after sending an error.
async fn bind_entry(
    state: &AppState,
    entry: ClientMessage,
    session: &Session,
    out: &mpsc::Sender<ServerMessage>,
) -> Option<mpsc::Sender<GroupCommand>> {
    match entry {
        ClientMessage::CreateGroup { display_name, .. } => {
            let (_code, group_tx) = state.groups.create();
            send_join(out, group_tx, session, display_name).await
        }
        ClientMessage::JoinGroup {
            display_name,
            group_code,
            ..
        } => {
            let Some(group_tx) = state.groups.get(&group_code) else {
                send_error(out, ErrorCode::UnknownGroup, "no such group").await;
                return None;
            };
            send_join(out, group_tx, session, display_name).await
        }
        ClientMessage::EnqueueMatch { display_name, .. } => {
            // Park until the queue assembles a table and hands back the group (the
            // queue sends the `Join` itself, so we only await the binding).
            let (notify_tx, notify_rx) = oneshot::channel();
            state
                .queue
                .enqueue(
                    session.player,
                    display_name,
                    session.token.clone(),
                    out.clone(),
                    notify_tx,
                )
                .await;
            notify_rx.await.ok()
        }
        _ => None,
    }
}

/// Reject an incompatible protocol version, returning `false` after erroring.
async fn version_ok(version: u16, out: &mpsc::Sender<ServerMessage>) -> bool {
    if version != PROTOCOL_VERSION {
        send_error(
            out,
            ErrorCode::VersionMismatch,
            &format!("server speaks protocol version {PROTOCOL_VERSION}"),
        )
        .await;
        false
    } else {
        true
    }
}

/// Send a `Join` (carrying the session token) to a group and return its channel.
/// A direct create/join entry always joins as a **member** (`guest: false`); guests
/// are only ever placed by the matchmaking fill queue.
async fn send_join(
    out: &mpsc::Sender<ServerMessage>,
    group_tx: mpsc::Sender<GroupCommand>,
    session: &Session,
    name: String,
) -> Option<mpsc::Sender<GroupCommand>> {
    if group_tx
        .send(GroupCommand::Join {
            player: session.player,
            name,
            session_token: session.token.clone(),
            guest: false,
            out: out.clone(),
        })
        .await
        .is_err()
    {
        return None;
    }
    Some(group_tx)
}

/// The variant name of an inbound message, for the `ws.message` span. This is a
/// public label only — it deliberately carries none of the message's payload.
fn message_kind(msg: &ClientMessage) -> &'static str {
    match msg {
        ClientMessage::CreateGroup { .. } => "CreateGroup",
        ClientMessage::JoinGroup { .. } => "JoinGroup",
        ClientMessage::EnqueueMatch { .. } => "EnqueueMatch",
        ClientMessage::CommitIngredient { .. } => "CommitIngredient",
        ClientMessage::CastSpell { .. } => "CastSpell",
        ClientMessage::CommitPass => "CommitPass",
        ClientMessage::CommitDefer => "CommitDefer",
        ClientMessage::PickBrewer { .. } => "PickBrewer",
        ClientMessage::SubmitRecipe { .. } => "SubmitRecipe",
        ClientMessage::LockIn => "LockIn",
        ClientMessage::Emote { .. } => "Emote",
        ClientMessage::PlayAgain => "PlayAgain",
        ClientMessage::FillGroup => "FillGroup",
        ClientMessage::CancelFill => "CancelFill",
        ClientMessage::LeaveGroup => "LeaveGroup",
        ClientMessage::CreateDeviceAccount => "CreateDeviceAccount",
        ClientMessage::LinkOAuth { .. } => "LinkOAuth",
        ClientMessage::Heartbeat => "Heartbeat",
    }
}

async fn send_error(out: &mpsc::Sender<ServerMessage>, code: ErrorCode, message: &str) {
    let _ = out
        .send(ServerMessage::Error {
            code,
            message: message.to_string(),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use boiling_point_protocol::client::PROTOCOL_VERSION;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as TMsg;

    async fn start_server() -> String {
        // Generous connection timeout so quiet test clients aren't dropped.
        start_server_with(std::time::Duration::from_secs(60)).await
    }

    async fn start_server_with(conn_timeout: std::time::Duration) -> String {
        let mut config =
            crate::config::ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
        // Short wave timers so tests don't wait out the real 30s/10s budgets.
        config.timing.wave1_ms = 250;
        config.timing.wave_ms = 200;
        // These scripted clients never answer the pre-game brewer pick or the
        // Apothecary draft: short timers let the server's deterministic
        // auto-pick / suggested quick-pick close both phases (the straggler
        // paths get live coverage for free).
        config.timing.brewer_pick_ms = 250;
        config.timing.draft_ms = 250;
        let registry = Arc::new(config.build_registry().unwrap());
        let config = Arc::new(config);
        // Shared identity stores (the same `Arc`s reach the registry, queue, and
        // transport) — so a sign-in resolves, and a rated game's update flows back.
        let accounts = Arc::new(AccountStore::new());
        let ratings = Arc::new(RatingStore::default());
        let groups = Arc::new(
            GroupRegistry::new(registry, config).with_identity(accounts.clone(), ratings.clone()),
        );
        let queue = Arc::new(MatchQueue::new(groups.clone()));
        groups.set_queue(&queue);
        let state = AppState {
            sessions: Arc::new(SessionStore::new()),
            groups,
            queue,
            conn_timeout,
            pool: None,
            accounts,
            ratings,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app(state)).await.unwrap();
        });
        format!("ws://{addr}/ws")
    }

    fn create(name: &str, version: u16) -> ClientMessage {
        ClientMessage::CreateGroup {
            protocol_version: version,
            display_name: name.into(),
            session_token: None,
            account_credential: None,
        }
    }

    /// A helper that connects and returns the socket.
    type Ws = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    async fn send(ws: &mut Ws, msg: &ClientMessage) {
        ws.send(TMsg::Binary(codec::encode(msg).unwrap()))
            .await
            .unwrap();
    }

    async fn recv(ws: &mut Ws) -> ServerMessage {
        loop {
            match ws.next().await.unwrap().unwrap() {
                TMsg::Binary(b) => return codec::decode(b.as_ref()).unwrap(),
                _ => continue,
            }
        }
    }

    /// Non-panicking receive: `None` once the connection closes.
    async fn recv_opt(ws: &mut Ws) -> Option<ServerMessage> {
        loop {
            match ws.next().await {
                Some(Ok(TMsg::Binary(b))) => return codec::decode(b.as_ref()).ok(),
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => return None,
            }
        }
    }

    fn join(name: &str, code: boiling_point_protocol::GroupCode) -> ClientMessage {
        ClientMessage::JoinGroup {
            protocol_version: PROTOCOL_VERSION,
            display_name: name.into(),
            session_token: None,
            account_credential: None,
            group_code: code,
        }
    }

    fn enqueue(name: &str) -> ClientMessage {
        ClientMessage::EnqueueMatch {
            protocol_version: PROTOCOL_VERSION,
            display_name: name.into(),
            session_token: None,
            account_credential: None,
        }
    }

    #[tokio::test]
    async fn matchmaking_assembles_a_table_of_four() {
        let url = start_server().await;
        let mut tasks = Vec::new();
        for i in 0..4 {
            let url = url.clone();
            tasks.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &enqueue(&format!("q{i}"))).await;
                let (mut joined, mut started) = (false, false);
                while !(joined && started) {
                    match recv_opt(&mut ws).await {
                        Some(ServerMessage::GroupJoined { .. }) => joined = true,
                        Some(ServerMessage::GameStarting { .. }) => started = true,
                        Some(_) => continue,
                        None => break,
                    }
                }
                joined && started
            }));
        }
        let all = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let mut ok = true;
            for t in tasks {
                ok &= t.await.unwrap();
            }
            ok
        })
        .await
        .expect("matchmaking assembled a table before timeout");
        assert!(all, "all four queued players reached a started game");
    }

    /// A trivial auto-player: each wave it plays the first ingredient of its
    /// (freshly topped-up) hand as a Vote, else passes; it stops at `GameOver`.
    /// Returns whether it saw GameOver.
    async fn play_loop(ws: &mut Ws) -> bool {
        let mut hand: Vec<boiling_point_protocol::CardId> = Vec::new();
        loop {
            let Some(msg) = recv_opt(ws).await else {
                return false;
            };
            match msg {
                ServerMessage::YourHand { ingredients, .. } => {
                    hand = ingredients.iter().map(|c| c.id).collect();
                }
                ServerMessage::WaveOpened { .. } => {
                    // Rely on the (short, in tests) wave timer to close; sending a
                    // LockIn here would be dropped by the 100ms rate limit anyway.
                    if let Some(&card) = hand.first() {
                        hand.remove(0);
                        send(
                            ws,
                            &ClientMessage::CommitIngredient {
                                card,
                                colorless: false,
                            },
                        )
                        .await;
                    } else {
                        send(ws, &ClientMessage::CommitPass).await;
                    }
                }
                ServerMessage::GameOver { .. } => return true,
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn emotes_validate_broadcast_and_lobby_carries_no_secret() {
        use boiling_point_protocol::EmoteId;
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("p1", PROTOCOL_VERSION)).await;
        let joined = recv(&mut ws).await;
        assert!(matches!(joined, ServerMessage::GroupJoined { .. }));
        // A lobby message must not carry any secret (e.g. the boiling point).
        assert!(
            !codec::encode_json(&joined)
                .unwrap()
                .contains("boiling_point")
        );

        // A palette emote is echoed back as a broadcast.
        send(&mut ws, &ClientMessage::Emote { emote: EmoteId(1) }).await;
        match recv(&mut ws).await {
            ServerMessage::EmoteBroadcast { emote, .. } => assert_eq!(emote.0, 1),
            other => panic!("expected EmoteBroadcast, got {other:?}"),
        }

        // An off-palette emote is rejected (spaced past the rate limit).
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        send(
            &mut ws,
            &ClientMessage::Emote {
                emote: EmoteId(999),
            },
        )
        .await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::Error {
                code: ErrorCode::InvalidEmote,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn missing_heartbeat_disconnects_the_connection() {
        // A short connection timeout: an idle (non-heartbeating) client is dropped.
        let url = start_server_with(std::time::Duration::from_millis(300)).await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("idle", PROTOCOL_VERSION)).await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::GroupJoined { .. }
        ));
        // Stay silent — the server should close the connection within the window.
        let closed = tokio::time::timeout(std::time::Duration::from_secs(3), recv_opt(&mut ws))
            .await
            .expect("server acted within the timeout");
        assert!(closed.is_none(), "idle connection was disconnected");
    }

    #[tokio::test]
    async fn join_and_leave_are_broadcast() {
        let url = start_server().await;
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };

        let (mut ws2, _) = connect_async(url).await.unwrap();
        send(&mut ws2, &join("p2", code)).await;

        // The first player sees the second connect.
        let connected = loop {
            if let ServerMessage::PlayerConnectionChanged { connected, .. } = recv(&mut ws1).await {
                break connected;
            }
        };
        assert!(connected);

        // When the second disconnects, the first sees the drop.
        drop(ws2);
        let saw_disconnect = loop {
            match recv_opt(&mut ws1).await {
                Some(ServerMessage::PlayerConnectionChanged {
                    connected: false, ..
                }) => break true,
                Some(_) => continue,
                None => break false,
            }
        };
        assert!(saw_disconnect);
    }

    #[tokio::test]
    async fn abandoned_player_does_not_stall_the_game() {
        let url = start_server().await;
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };

        // Players 2 and 3 play to the end.
        let mut players = Vec::new();
        for i in 2..=3 {
            let url = url.clone();
            let code = code.clone();
            players.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(&format!("p{i}"), code)).await;
                while !matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {}
                play_loop(&mut ws).await
            }));
        }
        // Player 4 joins, sees the game start, then abandons (drops the socket).
        let abandoner = {
            let url = url.clone();
            let code = code.clone();
            tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join("p4", code)).await;
                loop {
                    match recv_opt(&mut ws).await {
                        Some(ServerMessage::GameStarting { .. }) => break true,
                        Some(_) => continue,
                        None => break false,
                    }
                } // ws dropped here → disconnect
            })
        };
        let creator = tokio::spawn(async move { play_loop(&mut ws1).await });

        let ok = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let mut all = creator.await.unwrap();
            for p in players {
                all &= p.await.unwrap();
            }
            let abandoned_saw_start = abandoner.await.unwrap();
            all && abandoned_saw_start
        })
        .await
        .expect("game completed despite an abandonment");
        assert!(ok, "remaining players reached GameOver after one abandoned");
    }

    /// Like `play_loop`, but records every frame the client receives so the whole
    /// game's stream can be scanned for leaked secrets.
    async fn play_and_capture(ws: &mut Ws) -> Vec<ServerMessage> {
        let mut hand: Vec<boiling_point_protocol::CardId> = Vec::new();
        let mut frames = Vec::new();
        loop {
            let Some(msg) = recv_opt(ws).await else {
                return frames;
            };
            frames.push(msg.clone());
            match msg {
                ServerMessage::YourHand { ingredients, .. } => {
                    hand = ingredients.iter().map(|c| c.id).collect();
                }
                ServerMessage::WaveOpened { .. } => {
                    if let Some(&card) = hand.first() {
                        hand.remove(0);
                        send(
                            ws,
                            &ClientMessage::CommitIngredient {
                                card,
                                colorless: false,
                            },
                        )
                        .await;
                    } else {
                        send(ws, &ClientMessage::CommitPass).await;
                    }
                }
                ServerMessage::GameOver { .. } => return frames,
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn full_game_broadcasts_never_leak_secrets() {
        let url = start_server().await;

        // Player 1 creates the room; players 2–4 join; the fourth join starts it.
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };
        let mut joiners = Vec::new();
        for i in 2..=4 {
            let url = url.clone();
            let code = code.clone();
            joiners.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(&format!("p{i}"), code)).await;
                while !matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {}
                play_and_capture(&mut ws).await
            }));
        }
        let creator = tokio::spawn(async move { play_and_capture(&mut ws1).await });

        let all_frames: Vec<Vec<ServerMessage>> =
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                let mut v = vec![creator.await.unwrap()];
                for j in joiners {
                    v.push(j.await.unwrap());
                }
                v
            })
            .await
            .expect("game completed before timeout");

        // Scan every frame any client received. The boiling point may appear ONLY
        // in a private `PeekResult` or the post-round `Depile` (which reveals it
        // every round, boom and safe); no other message carries it. (Opponents'
        // hands, the decks, and primed Actives have no wire field at all, so they
        // cannot be broadcast by construction — see `protocol::server`.)
        let mut saw_game_over = false;
        for frames in &all_frames {
            for msg in frames {
                match msg {
                    ServerMessage::PeekResult { .. } => {} // legitimate private disclosure
                    ServerMessage::Depile { .. } => {}     // post-round reveal, every round
                    ServerMessage::GameOver { .. } => saw_game_over = true,
                    other => {
                        let json = codec::encode_json(other).unwrap();
                        assert!(
                            !json.contains("boiling_point"),
                            "a secret leaked in a broadcast frame: {json}"
                        );
                    }
                }
            }
        }
        assert!(saw_game_over, "the game reached GameOver");
    }

    #[tokio::test]
    async fn four_clients_play_a_full_game_to_game_over() {
        let url = start_server().await;

        // Player 1 creates the group and learns the invite code.
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };

        // Players 2–4 join by code; the fourth join starts the game.
        let mut joiners = Vec::new();
        for i in 2..=4 {
            let url = url.clone();
            let code = code.clone();
            joiners.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(&format!("p{i}"), code)).await;
                loop {
                    if matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {
                        break;
                    }
                }
                play_loop(&mut ws).await
            }));
        }
        let creator = tokio::spawn(async move { play_loop(&mut ws1).await });

        // The whole game should finish well within the timeout (waves close early
        // once everyone locks in).
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let mut all = creator.await.unwrap();
            for j in joiners {
                all &= j.await.unwrap();
            }
            all
        })
        .await
        .expect("game completed before timeout");
        assert!(outcome, "every client saw GameOver");
    }

    /// Like [`play_loop`], but plays through `n` games on one connection, opting in
    /// with `PlayAgain` after each `GameOver` until the last. Returns how many
    /// `GameOver`s it saw.
    async fn play_n_games(ws: &mut Ws, n: usize) -> usize {
        let mut hand: Vec<boiling_point_protocol::CardId> = Vec::new();
        let mut games = 0usize;
        loop {
            let Some(msg) = recv_opt(ws).await else {
                return games;
            };
            match msg {
                ServerMessage::YourHand { ingredients, .. } => {
                    hand = ingredients.iter().map(|c| c.id).collect();
                }
                ServerMessage::WaveOpened { .. } => {
                    if let Some(&card) = hand.first() {
                        hand.remove(0);
                        send(
                            ws,
                            &ClientMessage::CommitIngredient {
                                card,
                                colorless: false,
                            },
                        )
                        .await;
                    } else {
                        send(ws, &ClientMessage::CommitPass).await;
                    }
                }
                ServerMessage::GameOver { .. } => {
                    games += 1;
                    if games >= n {
                        return games;
                    }
                    // Opt in to another game with the same group.
                    send(ws, &ClientMessage::PlayAgain).await;
                }
                _ => {}
            }
        }
    }

    /// A persistent group plays two back-to-back games via "play again": after the
    /// first `GameOver` every seat opts in with `PlayAgain` and the same table is
    /// re-dealt without re-queuing (group-model D2/D3; tasks.md 6.2).
    #[tokio::test]
    async fn group_plays_two_games_via_play_again() {
        let url = start_server().await;
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };

        let mut joiners = Vec::new();
        for i in 2..=4 {
            let url = url.clone();
            let code = code.clone();
            joiners.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(&format!("p{i}"), code)).await;
                while !matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {}
                play_n_games(&mut ws, 2).await
            }));
        }
        let creator = tokio::spawn(async move { play_n_games(&mut ws1, 2).await });

        let outcome = tokio::time::timeout(std::time::Duration::from_secs(25), async {
            let mut all = creator.await.unwrap() == 2;
            for j in joiners {
                all &= j.await.unwrap() == 2;
            }
            all
        })
        .await
        .expect("two back-to-back games completed before timeout");
        assert!(outcome, "every client saw two GameOvers via play-again");
    }

    /// The session connection outlives the group: on one socket a player plays a
    /// game, leaves to the menu (`LeaveGroup` → `LeftGroup`), is rejected for a
    /// table action while unbound, then joins a *second*, different group — all
    /// without reconnecting (group-model D5; tasks.md 6.3).
    #[tokio::test]
    async fn one_socket_plays_leaves_then_joins_another_group() {
        let url = start_server().await;
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code_a = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };

        // Fill the table so a game runs; the joiners play to GameOver then drop.
        let mut joiners = Vec::new();
        for i in 2..=4 {
            let url = url.clone();
            let code = code_a.clone();
            joiners.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(&format!("p{i}"), code)).await;
                while !matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {}
                play_loop(&mut ws).await
            }));
        }

        let body = async {
            // p1 plays the first group's game to completion on this socket.
            assert!(play_loop(&mut ws1).await, "p1 should reach GameOver");
            for j in joiners {
                let _ = j.await;
            }

            // Leave to the menu — the socket stays open.
            send(&mut ws1, &ClientMessage::LeaveGroup).await;
            let saw_left = loop {
                match recv_opt(&mut ws1).await {
                    Some(ServerMessage::LeftGroup) => break true,
                    Some(_) => continue,
                    None => break false,
                }
            };
            assert!(saw_left, "LeaveGroup should be acknowledged with LeftGroup");

            // A table action while unbound is rejected (it changes no state).
            send(&mut ws1, &ClientMessage::CommitPass).await;
            let rejected = loop {
                match recv_opt(&mut ws1).await {
                    Some(ServerMessage::Error { .. }) => break true,
                    Some(_) => continue,
                    None => break false,
                }
            };
            assert!(
                rejected,
                "a table action in the menu state must be rejected"
            );

            // Join a second group on the SAME socket (re-entry, no reconnect).
            send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
            let code_b = loop {
                match recv_opt(&mut ws1).await {
                    Some(ServerMessage::GroupJoined { group_code, .. }) => break Some(group_code),
                    Some(_) => continue,
                    None => break None,
                }
            };
            let code_b = code_b.expect("rejoined a new group on the same socket");
            assert_ne!(code_a.0, code_b.0, "the second group is a distinct group");
        };
        tokio::time::timeout(std::time::Duration::from_secs(20), body)
            .await
            .expect("the leave/rejoin flow completed before timeout");
    }

    /// A partial group (3 members) fills with one matchmaking guest, plays a game,
    /// and returns to 3 members — the guest is dropped (group-fill-and-standings
    /// tasks.md 7.2). Verified end-to-end over the wire.
    #[tokio::test]
    async fn partial_group_fills_with_a_guest_then_drops_it() {
        let url = start_server().await;
        // Three friends form the group.
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("a", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };
        // The friends return their socket after the game so they stay connected as
        // members (a dropped socket would leave the group, skewing the second fill).
        let mut friends = Vec::new();
        for n in ["b", "c"] {
            let url = url.clone();
            let code = code.clone();
            friends.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(n, code)).await;
                while !matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {}
                let ok = play_loop(&mut ws).await;
                (ok, ws)
            }));
        }

        // Wait until both friends have actually joined (the host sees each connect)
        // before requesting fill, so the group is at 3 members.
        let mut connected = 0;
        while connected < 2 {
            if let ServerMessage::PlayerConnectionChanged {
                connected: true, ..
            } = recv(&mut ws1).await
            {
                connected += 1;
            }
        }

        // A member requests fill → the group announces it is searching for 1 more.
        send(&mut ws1, &ClientMessage::FillGroup).await;
        let needed = loop {
            match recv(&mut ws1).await {
                ServerMessage::GroupSearching { needed } => break needed,
                _ => continue,
            }
        };
        assert_eq!(needed, 1, "a 3-member group searches for exactly one guest");

        // A solo quick-matcher backfills the seat as a guest; the game starts with a
        // table of 4 that includes exactly one guest.
        let guest = {
            let url = url.clone();
            tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &enqueue("guest")).await;
                play_loop(&mut ws).await
            })
        };
        let started = loop {
            match recv(&mut ws1).await {
                ServerMessage::GameStarting { players, .. } => break players,
                _ => continue,
            }
        };
        assert_eq!(started.len(), 4, "the table filled to four");
        assert_eq!(
            started.iter().filter(|p| p.guest).count(),
            1,
            "exactly one seat is a matchmaking guest"
        );

        // Everyone plays to the end.
        assert!(play_loop(&mut ws1).await, "the host reached GameOver");
        // Hold the friends' sockets open so they remain members.
        let mut held = Vec::new();
        for f in friends {
            let (ok, ws) = f.await.unwrap();
            assert!(ok);
            held.push(ws);
        }
        assert!(guest.await.unwrap());

        // Back in the lobby the group is down to its 3 members: a fresh fill again
        // needs exactly one (the guest was dropped, the members persisted).
        send(&mut ws1, &ClientMessage::FillGroup).await;
        let again = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match recv_opt(&mut ws1).await {
                    Some(ServerMessage::GroupSearching { needed }) => break Some(needed),
                    Some(_) => continue,
                    None => break None,
                }
            }
        })
        .await
        .expect("got a searching response");
        assert_eq!(
            again,
            Some(1),
            "after the game the group is back to 3 members (guest dropped)"
        );
    }

    #[tokio::test]
    async fn create_group_handshake_returns_group_joined() {
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("alice", PROTOCOL_VERSION)).await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::GroupJoined { .. }
        ));
    }

    #[tokio::test]
    async fn incompatible_version_is_rejected() {
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("bob", PROTOCOL_VERSION + 1)).await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::Error {
                code: ErrorCode::VersionMismatch,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn heartbeat_round_trips_after_join() {
        let url = start_server().await;
        let (mut ws, _) = connect_async(url).await.unwrap();
        send(&mut ws, &create("cara", PROTOCOL_VERSION)).await;
        assert!(matches!(
            recv(&mut ws).await,
            ServerMessage::GroupJoined { .. }
        ));
        send(&mut ws, &ClientMessage::Heartbeat).await;
        assert!(matches!(recv(&mut ws).await, ServerMessage::Heartbeat));
    }

    /// Create-group entry presenting a device account credential (sign-in).
    fn create_with_device(token: &str) -> ClientMessage {
        ClientMessage::CreateGroup {
            protocol_version: PROTOCOL_VERSION,
            display_name: "resumed".into(),
            session_token: None,
            account_credential: Some(boiling_point_protocol::AccountCredential::Device {
                account_token: token.into(),
            }),
        }
    }

    /// `boom2-identity` (capability `player-accounts`): a device-bound account
    /// upgrade binds the current player id, and presenting its token on a fresh
    /// connection resumes the same durable identity (the scenario "device-bound
    /// account survives a session"). Verified end-to-end over the wire.
    #[tokio::test]
    async fn device_account_resumes_identity_across_connections() {
        let url = start_server().await;
        // Connect anonymously, create a group, then upgrade to a device account.
        let (mut ws, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws, &create("solo", PROTOCOL_VERSION)).await;
        let player1 = loop {
            if let ServerMessage::GroupJoined { your_player_id, .. } = recv(&mut ws).await {
                break your_player_id;
            }
        };
        send(&mut ws, &ClientMessage::CreateDeviceAccount).await;
        let (token, est_player) = loop {
            match recv(&mut ws).await {
                ServerMessage::AccountEstablished {
                    account_token,
                    player_id,
                    ..
                } => break (account_token, player_id),
                _ => continue,
            }
        };
        assert_eq!(
            est_player, player1,
            "the account binds the current player id"
        );
        let token = token.expect("a device account returns a durable token");
        drop(ws);

        // Reconnect on a fresh socket presenting the device credential → same id.
        let (mut ws2, _) = connect_async(url).await.unwrap();
        send(&mut ws2, &create_with_device(&token)).await;
        // The account intro arrives, and the join confirms the SAME player id.
        let mut saw_account = false;
        let player2 = loop {
            match recv(&mut ws2).await {
                ServerMessage::AccountEstablished { player_id, .. } => {
                    assert_eq!(player_id, player1);
                    saw_account = true;
                }
                ServerMessage::GroupJoined { your_player_id, .. } => break your_player_id,
                _ => continue,
            }
        };
        assert!(saw_account, "a resumed account is confirmed on sign-in");
        assert_eq!(
            player2, player1,
            "the device account resumed the durable identity on a new connection"
        );
    }

    /// Like [`play_loop`], but after `GameOver` it waits briefly for the private
    /// `RatingUpdate` the server sends to a rated seat. Returns the games_played
    /// of that update, or `None` if the seat was unrated (no update arrived).
    async fn play_then_rating(ws: &mut Ws) -> Option<u32> {
        let mut hand: Vec<boiling_point_protocol::CardId> = Vec::new();
        let mut over = false;
        loop {
            let next = if over {
                // After GameOver, the rating readout follows almost immediately.
                tokio::time::timeout(std::time::Duration::from_secs(2), recv_opt(ws))
                    .await
                    .ok()
                    .flatten()
            } else {
                recv_opt(ws).await
            };
            let msg = next?;
            match msg {
                ServerMessage::YourHand { ingredients, .. } => {
                    hand = ingredients.iter().map(|c| c.id).collect();
                }
                ServerMessage::WaveOpened { .. } => {
                    if let Some(&card) = hand.first() {
                        hand.remove(0);
                        send(
                            ws,
                            &ClientMessage::CommitIngredient {
                                card,
                                colorless: false,
                            },
                        )
                        .await;
                    } else {
                        send(ws, &ClientMessage::CommitPass).await;
                    }
                }
                ServerMessage::GameOver { .. } => over = true,
                // The post-game readout (only after GameOver); the establishment
                // readout sent on account creation (games_played 0) is ignored.
                ServerMessage::RatingUpdate { rating } if over => {
                    return Some(rating.games_played);
                }
                _ => {}
            }
        }
    }

    /// `boom2-identity` (capability `player-rating`): a full four-player game in
    /// which every seat holds an account updates all four ratings and sends each
    /// seat its fresh readout (games_played == 1). Verified end-to-end over the
    /// wire — the rating update is server-authoritative and rides only the
    /// finished result.
    #[tokio::test]
    async fn rated_game_emits_rating_updates_to_every_account() {
        let url = start_server().await;

        // Upgrade the current (entered) connection to a device account, draining
        // up to the AccountEstablished confirmation.
        async fn upgrade(ws: &mut Ws) {
            send(ws, &ClientMessage::CreateDeviceAccount).await;
            loop {
                match recv(ws).await {
                    ServerMessage::AccountEstablished { .. } => break,
                    _ => continue,
                }
            }
        }

        // Set up all four sockets sequentially BEFORE any play task: p1 creates,
        // p2–p4 join, and each upgrades to an account. The fourth join auto-starts
        // the game, but every account is registered during setup, so all four are
        // rated by GameOver regardless of timing.
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };
        upgrade(&mut ws1).await;

        let mut sockets = vec![ws1];
        for i in 2..=4 {
            let (mut ws, _) = connect_async(url.clone()).await.unwrap();
            send(&mut ws, &join(&format!("p{i}"), code.clone())).await;
            while !matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {}
            upgrade(&mut ws).await;
            sockets.push(ws);
        }

        // Now play all four to GameOver and collect each seat's rating readout.
        let mut tasks = Vec::new();
        for mut ws in sockets {
            tasks.push(tokio::spawn(async move { play_then_rating(&mut ws).await }));
        }
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            let mut all = Vec::new();
            for t in tasks {
                all.push(t.await.unwrap());
            }
            all
        })
        .await
        .expect("the rated game completed before timeout");
        assert_eq!(outcome.len(), 4);
        for rated in &outcome {
            assert_eq!(
                *rated,
                Some(1),
                "every account seat received a rating readout for its first rated game"
            );
        }
    }

    use crate::observability::lifecycle::{SpanConsumer, SpanEvent, SpanEventKind};

    /// Records every lifecycle event for the span-tree assertion.
    #[derive(Default)]
    struct SpanCapture(std::sync::Mutex<Vec<SpanEvent>>);
    impl SpanCapture {
        fn events(&self) -> Vec<SpanEvent> {
            self.0.lock().unwrap().clone()
        }
    }
    impl SpanConsumer for SpanCapture {
        fn on_event(&self, event: SpanEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    /// Install a process-global subscriber feeding a [`SpanCapture`] (once). A
    /// global subscriber is required so spans emitted on spawned group/session tasks
    /// are observed; the per-thread `with_default` used by unit tests would miss
    /// them.
    fn install_span_capture() -> std::sync::Arc<SpanCapture> {
        use crate::observability::lifecycle::LifecycleHandle;
        use std::sync::OnceLock;
        use tracing_subscriber::layer::SubscriberExt;
        static CAP: OnceLock<std::sync::Arc<SpanCapture>> = OnceLock::new();
        CAP.get_or_init(|| {
            let cap = std::sync::Arc::new(SpanCapture::default());
            let handle = LifecycleHandle::new();
            handle.register(cap.clone());
            // The subscriber (stored globally) keeps a clone of the handle alive, so
            // the lifecycle channel/drain thread outlive this function.
            let subscriber = tracing_subscriber::registry().with(handle.layer());
            let _ = tracing::subscriber::set_global_default(subscriber);
            cap
        })
        .clone()
    }

    /// Spin until `f` is true or a deadline passes (the drain thread is async).
    fn wait_until(mut f: impl FnMut() -> bool) -> bool {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if f() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        f()
    }

    #[tokio::test]
    async fn span_tree_is_emitted_during_a_full_game() {
        let cap = install_span_capture();

        // Run a full four-player game to GameOver.
        let url = start_server().await;
        let (mut ws1, _) = connect_async(url.clone()).await.unwrap();
        send(&mut ws1, &create("p1", PROTOCOL_VERSION)).await;
        let code = loop {
            if let ServerMessage::GroupJoined { group_code, .. } = recv(&mut ws1).await {
                break group_code;
            }
        };
        let mut joiners = Vec::new();
        for i in 2..=4 {
            let url = url.clone();
            let code = code.clone();
            joiners.push(tokio::spawn(async move {
                let (mut ws, _) = connect_async(url).await.unwrap();
                send(&mut ws, &join(&format!("p{i}"), code)).await;
                while !matches!(recv(&mut ws).await, ServerMessage::GroupJoined { .. }) {}
                play_loop(&mut ws).await
            }));
        }
        let creator = tokio::spawn(async move { play_loop(&mut ws1).await });
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let mut all = creator.await.unwrap();
            for j in joiners {
                all &= j.await.unwrap();
            }
            all
        })
        .await
        .expect("game completed before timeout");
        assert!(outcome, "every client saw GameOver");

        // The documented v2 span tree should now be visible to the lifecycle
        // consumer (spell.cast is absent only because the scripted bots never
        // cast; the session tests cover it).
        let expected = [
            "group.lifetime",
            "game",
            "brewer.pick",
            "round",
            "wave",
            "resolve",
            "score",
            "commit",
            "depile",
            "hand",
        ];
        let has = |name: &str| cap.events().iter().any(|e| e.name == name);
        assert!(
            wait_until(|| expected.iter().all(|n| has(n))),
            "not all documented spans were emitted"
        );

        let events = cap.events();
        // No v1-only emission survives: the retired v1 attribute keys never appear,
        // and the planned-but-unimplemented pre-game spans are not emitted.
        for e in events.iter() {
            assert!(
                !e.attributes.contains_key("round.exploded")
                    && !e.attributes.contains_key("dominant_color"),
                "v1-only attribute emitted on span {}",
                e.name
            );
            assert!(
                !crate::observability::span_schema::is_planned_span(e.name),
                "planned span {} must not be emitted before its change lands",
                e.name
            );
        }
        // The schema version is stamped on the game span.
        assert!(
            events.iter().any(|e| e.name == "game"
                && e.attributes.get("schema.version").map(String::as_str)
                    == Some(&crate::observability::span_schema::SPAN_SCHEMA_VERSION.to_string())),
            "game span missing schema.version = {}",
            crate::observability::span_schema::SPAN_SCHEMA_VERSION
        );
        // Tree nesting matches the contract: every closed span with a documented
        // parent nests under a span of exactly that name.
        {
            use std::collections::HashMap;
            let names: HashMap<u64, &'static str> = events
                .iter()
                .filter(|e| e.kind == SpanEventKind::Start)
                .map(|e| (e.id, e.name))
                .collect();
            for e in events.iter().filter(|e| e.kind == SpanEventKind::End) {
                if let Some(expected_parent) = crate::observability::span_schema::parent_of(e.name)
                    && let Some(parent_id) = e.parent_id
                    && let Some(parent_name) = names.get(&parent_id)
                {
                    assert_eq!(
                        *parent_name, expected_parent,
                        "span {} nested under {} instead of {}",
                        e.name, parent_name, expected_parent
                    );
                }
            }
        }
        // The round closes with its public v2 outcome.
        assert!(
            events.iter().any(|e| e.name == "round"
                && e.kind == SpanEventKind::End
                && e.attributes.contains_key("round.boomed")),
            "round spans must close with the round.boomed outcome"
        );
        // The depile carries the (now public) boiling point and the sorted reveal.
        assert!(
            events.iter().any(|e| e.name == "depile"
                && e.attributes.contains_key("boiling_point")
                && e.attributes.contains_key("reveals")),
            "depile spans must carry the boiling point and the reveal order"
        );
        // Live-registry key attributes are present on the right spans.
        assert!(
            events
                .iter()
                .any(|e| e.name == "group.lifetime" && e.attributes.contains_key("group.code")),
            "group.lifetime missing group.code"
        );
        assert!(
            events
                .iter()
                .any(|e| e.name == "round" && e.attributes.contains_key("round.number")),
            "round missing round.number"
        );
        assert!(
            events
                .iter()
                .any(|e| e.name == "wave" && e.attributes.contains_key("wave.number")),
            "wave missing wave.number"
        );
        // Sensitive state rides on the round/hand spans (admin-only; never on the
        // player wire) under the schema's marked sensitive keys.
        assert!(
            events
                .iter()
                .any(|e| e.name == "round" && e.attributes.contains_key("boiling_point")),
            "boiling_point (secret) not carried in-process on a round span"
        );
        assert!(
            events.iter().any(|e| e.name == "hand"
                && e.attributes.contains_key("hand.pantry")
                && e.attributes.contains_key("hand.spells")),
            "hand spans must carry the pantry and spell hands (secret)"
        );
        assert!(
            events
                .iter()
                .any(|e| e.name == "commit" && e.attributes.contains_key("vote.color")),
            "commit spans must carry the Vote colour (secret until the depile)"
        );
        // group.lifetime both opens and (after teardown) closes — asserted on
        // THIS game's group code, so a sibling test's group can never satisfy it
        // (the capture is process-global across the lib tests).
        let this_group = |e: &crate::observability::lifecycle::SpanEvent, kind: SpanEventKind| {
            e.name == "group.lifetime"
                && e.kind == kind
                && e.attributes.get("group.code").map(String::as_str) == Some(code.0.as_str())
        };
        assert!(
            events.iter().any(|e| this_group(e, SpanEventKind::Start)),
            "no group.lifetime Start observed for this game's group"
        );
        // The teardown needs the runtime to keep turning: the clients' sockets
        // have closed, but the socket handlers must observe it and the group task
        // must process the Leaves — all tasks on this (current-thread) test
        // runtime. A blocking wait would starve them, so the wait must yield.
        let group_ended = || {
            cap.events()
                .iter()
                .any(|e| this_group(e, SpanEventKind::End))
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !group_ended() && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            group_ended(),
            "group.lifetime never ended after the game completed"
        );
    }
}
