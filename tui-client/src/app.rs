//! The application state machine: the view model plus all interaction state,
//! and the pure reducers over it (`on_server`, `on_key`, `on_tick`).
//!
//! `App` owns **no** game logic and validates nothing — it renders server state
//! and turns key presses into [`ClientMessage`] intents for the server to judge
//! (Constitution I). All three reducers are deterministic and side-effect-free
//! (they return intents rather than sending), which is what makes the client
//! testable with neither a terminal nor a server present.

use std::collections::VecDeque;

use boiling_point_protocol::{
    ids::{CardId, EmoteId, RoomCode},
    server::ServerMessage,
    vocab::{EffectKind, HandCard},
    ClientMessage, PROTOCOL_VERSION,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{backend::TestBackend, buffer::Buffer, Frame, Terminal};

use crate::view::{Phase, ViewModel};

/// Maximum debug message-log entries retained.
const MSG_LOG_CAP: usize = 200;
/// Milliseconds each depile card stays before the next is revealed.
const DEPILE_STEP_MS: u32 = 600;
/// How long the boom overlay holds before the scoring screen.
const BOOM_MS: u32 = 1500;
/// How long a private Peek result modal stays up.
const PEEK_MODAL_MS: u32 = 6000;
/// Default toast lifetime.
const TOAST_MS: u32 = 3500;
/// Reconnection grace window (mirrors the server's 60 s).
const GRACE_MS: u32 = 60_000;

/// The connection state, rendered as an overlay independent of the game phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Conn {
    /// Live.
    Connected,
    /// Dropped; counting down the grace window while reconnecting.
    Reconnecting {
        /// Milliseconds left before the seat is abandoned.
        remaining_ms: u32,
    },
    /// Grace elapsed; the server will have abandoned the seat.
    Abandoned,
}

/// What the player currently intends to commit this wave (changeable until close).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selection {
    /// Nothing chosen yet.
    None,
    /// Committed to passing (locks out for the round).
    Pass,
    /// Committed a specific card.
    Card(CardId),
}

/// A transient on-screen notice (peek, expose, emote, reshuffle, error).
#[derive(Debug, Clone)]
pub(crate) struct Toast {
    /// The text shown.
    pub(crate) text: String,
    /// Remaining lifetime in milliseconds.
    pub(crate) ttl_ms: u32,
}

/// The private Recall target picker, opened at commit time.
#[derive(Debug, Clone)]
pub(crate) struct RecallPrompt {
    /// The Recall card being committed.
    pub(crate) recall_card: CardId,
    /// The player's own pot cards they may retrieve.
    pub(crate) targets: Vec<HandCard>,
    /// Highlighted target.
    pub(crate) cursor: usize,
}

/// The whole client application state.
pub struct App {
    pub(crate) vm: ViewModel,
    pub(crate) phase: Phase,
    pub(crate) conn: Conn,
    pub(crate) name_input: String,
    pub(crate) code_input: String,
    pub(crate) menu_index: usize,
    pub(crate) cursor: usize,
    pub(crate) committed: Selection,
    pub(crate) locked_in: bool,
    pub(crate) countdown_ms: Option<u32>,
    pub(crate) my_pot: Vec<HandCard>,
    pub(crate) recall: Option<RecallPrompt>,
    pub(crate) emote_open: bool,
    pub(crate) depile_shown: usize,
    pub(crate) depile_accum_ms: u32,
    pub(crate) boom_ms: u32,
    pub(crate) peek_modal_ms: u32,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) debug: bool,
    pub(crate) msg_log: VecDeque<String>,
    pub(crate) in_count: u64,
    pub(crate) out_count: u64,
    pub(crate) should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// A fresh client at the entry menu.
    pub fn new() -> Self {
        App {
            vm: ViewModel::default(),
            phase: Phase::Entry,
            conn: Conn::Connected,
            name_input: String::new(),
            code_input: String::new(),
            menu_index: 0,
            cursor: 0,
            committed: Selection::None,
            locked_in: false,
            countdown_ms: None,
            my_pot: Vec::new(),
            recall: None,
            emote_open: false,
            depile_shown: 0,
            depile_accum_ms: 0,
            boom_ms: 0,
            peek_modal_ms: 0,
            toasts: Vec::new(),
            debug: false,
            msg_log: VecDeque::new(),
            in_count: 0,
            out_count: 0,
            should_quit: false,
        }
    }

    /// Whether the user has asked to quit.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Ask the loop to exit (e.g. the input stream closed).
    pub fn request_quit(&mut self) {
        self.should_quit = true;
    }

    /// Pre-fill the display-name field (from a `--name` argument).
    pub fn set_name(&mut self, name: String) {
        self.name_input = name;
    }

    /// The protocol version this client speaks.
    pub fn protocol_version(&self) -> u16 {
        PROTOCOL_VERSION
    }

    // ---- server messages -------------------------------------------------

    /// Fold a server message into the view model and advance the phase. Any
    /// inbound message also clears a reconnecting overlay (we are clearly live).
    pub fn on_server(&mut self, msg: &ServerMessage) {
        self.in_count += 1;
        self.push_log(format!("◀ {}", server_tag(msg)));
        if !matches!(self.conn, Conn::Connected) {
            self.conn = Conn::Connected;
        }
        self.vm.apply(msg);
        self.react(msg);
    }

    fn react(&mut self, msg: &ServerMessage) {
        match msg {
            ServerMessage::RoomJoined { .. } => self.phase = Phase::Lobby,
            ServerMessage::GameStarting { .. } => self.phase = Phase::RoundStart,
            ServerMessage::ModifierRevealed { .. } => self.phase = Phase::RoundStart,
            ServerMessage::YourHand { .. } => {
                if self.phase != Phase::Playing {
                    self.phase = Phase::RoundStart;
                }
            }
            ServerMessage::WaveOpened {
                wave_number,
                timer_ms,
                ..
            } => {
                self.phase = Phase::Playing;
                self.cursor = 0;
                self.committed = Selection::None;
                self.locked_in = false;
                self.recall = None;
                self.countdown_ms = Some(*timer_ms);
                if *wave_number == 1 {
                    self.my_pot.clear();
                }
            }
            ServerMessage::WaveResolved { played, .. } => {
                // Track my own play locally: the committed card leaves my hand and
                // joins my pot (the client legitimately knows its own plays).
                if let (Selection::Card(id), Some(me)) = (self.committed, self.vm.me) {
                    if played.contains(&me) {
                        if let Some(pos) = self.vm.hand.iter().position(|c| c.id == id) {
                            let card = self.vm.hand.remove(pos);
                            self.my_pot.push(card);
                        }
                    }
                }
                self.committed = Selection::None;
                self.locked_in = false;
            }
            ServerMessage::Depile { .. } => {
                self.phase = Phase::Depile;
                self.depile_shown = 0;
                self.depile_accum_ms = 0;
            }
            ServerMessage::RoundScored { .. } => self.phase = Phase::Scoring,
            ServerMessage::Explosion { .. } => {
                self.phase = Phase::Scoring;
                self.boom_ms = BOOM_MS;
            }
            ServerMessage::GameOver { .. } => self.phase = Phase::GameOver,
            ServerMessage::PeekResult { .. } => self.peek_modal_ms = PEEK_MODAL_MS,
            ServerMessage::SomeonePeeked => self.toast("👀 someone peeked at the cauldron"),
            ServerMessage::Exposed { card } => {
                self.toast(format!("🔦 exposed: {}", crate::ui::card_label(card)))
            }
            ServerMessage::DeckReshuffled => self.toast("♻ deck reshuffled — counts reset"),
            ServerMessage::EmoteBroadcast { from, emote } => {
                let who = self
                    .vm
                    .player(*from)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "someone".into());
                let (icon, _) = emote_label(emote.0);
                self.toast(format!("{who}: {icon}"));
            }
            ServerMessage::Error { message, .. } => {
                self.toast(format!("⚠ {message}"));
                if matches!(self.phase, Phase::Connecting | Phase::Queue) {
                    self.phase = Phase::Entry;
                }
            }
            ServerMessage::ScoreUpdate { .. }
            | ServerMessage::PlayerConnectionChanged { .. }
            | ServerMessage::Heartbeat => {}
        }
    }

    // ---- connection lifecycle (driven by the transport) ------------------

    /// Note a transport disconnect mid-session and begin the grace countdown.
    pub fn on_disconnect(&mut self) {
        if matches!(self.conn, Conn::Connected) {
            self.conn = Conn::Reconnecting {
                remaining_ms: GRACE_MS,
            };
        }
    }

    // ---- timed updates ---------------------------------------------------

    /// Advance time-based state by `dt_ms` (animations, timers, toasts).
    pub fn on_tick(&mut self, dt_ms: u32) {
        if let Some(ms) = self.countdown_ms.as_mut() {
            *ms = ms.saturating_sub(dt_ms);
        }
        self.boom_ms = self.boom_ms.saturating_sub(dt_ms);
        self.peek_modal_ms = self.peek_modal_ms.saturating_sub(dt_ms);
        for t in &mut self.toasts {
            t.ttl_ms = t.ttl_ms.saturating_sub(dt_ms);
        }
        self.toasts.retain(|t| t.ttl_ms > 0);
        if let Conn::Reconnecting { remaining_ms } = &mut self.conn {
            *remaining_ms = remaining_ms.saturating_sub(dt_ms);
            if *remaining_ms == 0 {
                self.conn = Conn::Abandoned;
            }
        }
        if self.phase == Phase::Depile {
            if let Some(total) = self.depile_total() {
                if self.depile_shown < total {
                    self.depile_accum_ms += dt_ms;
                    while self.depile_accum_ms >= DEPILE_STEP_MS && self.depile_shown < total {
                        self.depile_shown += 1;
                        self.depile_accum_ms -= DEPILE_STEP_MS;
                    }
                }
            }
        }
    }

    fn depile_total(&self) -> Option<usize> {
        self.vm.last_depile.as_ref().map(|d| d.reveals.len())
    }

    // ---- input -----------------------------------------------------------

    /// Handle a key press, returning the [`ClientMessage`] intents to send.
    pub fn on_key(&mut self, key: KeyEvent) -> Vec<ClientMessage> {
        if key.kind != KeyEventKind::Press {
            return vec![];
        }
        // Universal quit.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return vec![];
        }
        if key.code == KeyCode::F(12) {
            self.debug = !self.debug;
            return vec![];
        }
        // A Recall picker, when open, captures input.
        if self.recall.is_some() {
            return self.key_recall(key.code);
        }
        match self.phase {
            Phase::Entry => self.key_entry(key.code),
            Phase::JoinCode => self.key_joincode(key.code),
            Phase::Connecting | Phase::Queue | Phase::Lobby => self.key_lobby(key.code),
            Phase::RoundStart => vec![],
            Phase::Playing => self.key_playing(key.code),
            Phase::Depile => self.key_depile(key.code),
            Phase::Scoring => vec![],
            Phase::GameOver => self.key_gameover(key.code),
        }
    }

    fn key_entry(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        match code {
            KeyCode::Up => {
                self.menu_index = self.menu_index.saturating_sub(1);
                vec![]
            }
            KeyCode::Down => {
                self.menu_index = (self.menu_index + 1).min(2);
                vec![]
            }
            KeyCode::Char(c) => {
                self.name_input.push(c);
                vec![]
            }
            KeyCode::Backspace => {
                self.name_input.pop();
                vec![]
            }
            KeyCode::Enter => {
                if self.name_input.trim().is_empty() {
                    self.toast("enter a display name first");
                    return vec![];
                }
                let name = self.name_input.trim().to_string();
                match self.menu_index {
                    0 => {
                        self.phase = Phase::Queue;
                        vec![ClientMessage::EnqueueMatch {
                            protocol_version: PROTOCOL_VERSION,
                            display_name: name,
                            session_token: None,
                        }]
                    }
                    1 => {
                        self.phase = Phase::Connecting;
                        vec![ClientMessage::CreateRoom {
                            protocol_version: PROTOCOL_VERSION,
                            display_name: name,
                            session_token: None,
                        }]
                    }
                    _ => {
                        self.phase = Phase::JoinCode;
                        vec![]
                    }
                }
            }
            KeyCode::Esc => {
                self.should_quit = true;
                vec![]
            }
            _ => vec![],
        }
    }

    fn key_joincode(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        match code {
            KeyCode::Char(c) => {
                self.code_input.push(c.to_ascii_uppercase());
                vec![]
            }
            KeyCode::Backspace => {
                self.code_input.pop();
                vec![]
            }
            KeyCode::Esc => {
                self.phase = Phase::Entry;
                vec![]
            }
            KeyCode::Enter => {
                if self.code_input.trim().is_empty() {
                    self.toast("enter an invite code");
                    return vec![];
                }
                self.phase = Phase::Connecting;
                vec![ClientMessage::JoinRoom {
                    protocol_version: PROTOCOL_VERSION,
                    display_name: self.name_input.trim().to_string(),
                    session_token: None,
                    room_code: RoomCode(self.code_input.trim().to_string()),
                }]
            }
            _ => vec![],
        }
    }

    fn key_lobby(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        if code == KeyCode::Char('c') {
            if let Some(rc) = self.vm.room_code.clone() {
                match crate::clipboard::copy(&format!("boilingpoint.gg/r/{rc}")) {
                    Ok(()) => self.toast("invite link copied"),
                    Err(_) => self.toast("copy unavailable in this terminal"),
                }
            }
        }
        vec![]
    }

    fn key_playing(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        let pass_slot = self.vm.hand.len();
        if self.emote_open {
            if let KeyCode::Char(c @ '1'..='6') = code {
                self.emote_open = false;
                return vec![ClientMessage::Emote {
                    emote: EmoteId(c as u16 - '0' as u16),
                }];
            }
            if code == KeyCode::Esc {
                self.emote_open = false;
            }
            return vec![];
        }
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.cursor = if self.cursor == 0 {
                    pass_slot
                } else {
                    self.cursor - 1
                };
                vec![]
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.cursor = if self.cursor >= pass_slot {
                    0
                } else {
                    self.cursor + 1
                };
                vec![]
            }
            KeyCode::Char(d @ '1'..='9') => {
                let idx = (d as usize - '1' as usize).min(pass_slot);
                if idx < self.vm.hand.len() {
                    self.cursor = idx;
                }
                vec![]
            }
            KeyCode::Char('p') => {
                self.cursor = pass_slot;
                self.commit_cursor()
            }
            KeyCode::Char('e') => {
                self.emote_open = true;
                vec![]
            }
            KeyCode::Char('L') | KeyCode::Tab => {
                if matches!(self.committed, Selection::None) {
                    self.toast("choose a card or pass before locking in");
                    vec![]
                } else {
                    self.locked_in = true;
                    vec![ClientMessage::LockIn]
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.commit_cursor(),
            _ => vec![],
        }
    }

    /// Commit whatever the cursor points at (a card, possibly opening the Recall
    /// picker, or the Pass slot). Returns the intent to send.
    fn commit_cursor(&mut self) -> Vec<ClientMessage> {
        let pass_slot = self.vm.hand.len();
        if self.cursor >= pass_slot {
            self.committed = Selection::Pass;
            self.toast("passing locks you out of the round");
            return vec![ClientMessage::CommitPass];
        }
        let card = self.vm.hand[self.cursor];
        if card.view.effect == Some(EffectKind::Recall) && !self.my_pot.is_empty() {
            self.recall = Some(RecallPrompt {
                recall_card: card.id,
                targets: self.my_pot.clone(),
                cursor: 0,
            });
            return vec![];
        }
        self.committed = Selection::Card(card.id);
        vec![ClientMessage::CommitCard { card: card.id }]
    }

    fn key_recall(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        let Some(prompt) = self.recall.as_mut() else {
            return vec![];
        };
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                prompt.cursor = prompt.cursor.saturating_sub(1);
                vec![]
            }
            KeyCode::Right | KeyCode::Char('l') => {
                prompt.cursor = (prompt.cursor + 1).min(prompt.targets.len().saturating_sub(1));
                vec![]
            }
            KeyCode::Esc => {
                self.recall = None;
                vec![]
            }
            KeyCode::Enter => {
                let recall_card = prompt.recall_card;
                self.recall = None;
                self.committed = Selection::Card(recall_card);
                // PROTOCOL GAP: `CommitCard` carries no Recall target, so the
                // chosen card cannot be transmitted yet. We send the Recall card
                // and flag that the target is not wired.
                self.toast("recall target not yet carried by the wire");
                vec![ClientMessage::CommitCard { card: recall_card }]
            }
            _ => vec![],
        }
    }

    fn key_depile(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        if matches!(code, KeyCode::Enter | KeyCode::Char(' ')) {
            if let Some(total) = self.depile_total() {
                self.depile_shown = total;
            }
        }
        vec![]
    }

    fn key_gameover(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        match code {
            KeyCode::Char('r') => {
                let name = self.name_input.trim().to_string();
                self.reset_for_new_game();
                self.phase = Phase::Queue;
                vec![ClientMessage::EnqueueMatch {
                    protocol_version: PROTOCOL_VERSION,
                    display_name: name,
                    session_token: None,
                }]
            }
            KeyCode::Enter | KeyCode::Esc => {
                self.reset_for_new_game();
                vec![]
            }
            _ => vec![],
        }
    }

    fn reset_for_new_game(&mut self) {
        self.vm = ViewModel::default();
        self.phase = Phase::Entry;
        self.committed = Selection::None;
        self.locked_in = false;
        self.countdown_ms = None;
        self.my_pot.clear();
        self.recall = None;
        self.emote_open = false;
        self.depile_shown = 0;
        self.boom_ms = 0;
        self.peek_modal_ms = 0;
    }

    // ---- helpers / debug -------------------------------------------------

    fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast {
            text: text.into(),
            ttl_ms: TOAST_MS,
        });
    }

    fn push_log(&mut self, line: String) {
        if self.msg_log.len() >= MSG_LOG_CAP {
            self.msg_log.pop_front();
        }
        self.msg_log.push_back(line);
    }

    /// Record an outgoing intent in the debug log (called by the transport).
    pub fn log_outgoing(&mut self, msg: &ClientMessage) {
        self.out_count += 1;
        self.push_log(format!("▶ {}", client_tag(msg)));
    }

    /// The current view model serialized as pretty JSON (for the debug overlay
    /// and replay tooling). Contains only player-visible data.
    pub fn view_json(&self) -> String {
        serde_json::to_string_pretty(&self.vm).unwrap_or_else(|_| "{}".into())
    }

    // ---- rendering -------------------------------------------------------

    /// Draw the current screen into `frame`.
    pub fn render(&self, frame: &mut Frame) {
        crate::ui::draw(frame, self);
    }

    /// Render once into an in-memory buffer at the given size (test helper).
    pub fn render_to_buffer(&self, width: u16, height: u16) -> Buffer {
        let mut terminal =
            Terminal::new(TestBackend::new(width, height)).expect("test terminal init");
        terminal.draw(|f| self.render(f)).expect("draw");
        terminal.backend().buffer().clone()
    }

    // ---- test/mock-only helpers -----------------------------------------

    /// Force the reconnecting overlay with a given remaining grace (test/mock).
    pub fn set_reconnecting(&mut self, remaining_ms: u32) {
        self.conn = Conn::Reconnecting { remaining_ms };
    }

    /// Force the Deathmatch flag (test/mock) until the wire carries a marker.
    pub fn set_deathmatch(&mut self, on: bool) {
        self.vm.deathmatch = on;
    }
}

/// The preset emote palette: id → (icon, label). Mirrors the design's six emotes.
pub(crate) fn emote_label(id: u16) -> (&'static str, &'static str) {
    match id {
        1 => ("🤝", "truce"),
        2 => ("😈", "scheming"),
        3 => ("😱", "fear"),
        4 => ("😂", "taunt"),
        5 => ("👀", "watching"),
        6 => ("💀", "you're done"),
        _ => ("·", "emote"),
    }
}

/// A short tag for a server message, for the debug log.
fn server_tag(m: &ServerMessage) -> &'static str {
    match m {
        ServerMessage::RoomJoined { .. } => "RoomJoined",
        ServerMessage::GameStarting { .. } => "GameStarting",
        ServerMessage::YourHand { .. } => "YourHand",
        ServerMessage::WaveOpened { .. } => "WaveOpened",
        ServerMessage::WaveResolved { .. } => "WaveResolved",
        ServerMessage::ModifierRevealed { .. } => "ModifierRevealed",
        ServerMessage::SomeonePeeked => "SomeonePeeked",
        ServerMessage::Exposed { .. } => "Exposed",
        ServerMessage::DeckReshuffled => "DeckReshuffled",
        ServerMessage::EmoteBroadcast { .. } => "EmoteBroadcast",
        ServerMessage::PeekResult { .. } => "PeekResult",
        ServerMessage::Depile { .. } => "Depile",
        ServerMessage::RoundScored { .. } => "RoundScored",
        ServerMessage::Explosion { .. } => "Explosion",
        ServerMessage::ScoreUpdate { .. } => "ScoreUpdate",
        ServerMessage::GameOver { .. } => "GameOver",
        ServerMessage::Error { .. } => "Error",
        ServerMessage::PlayerConnectionChanged { .. } => "PlayerConnectionChanged",
        ServerMessage::Heartbeat => "Heartbeat",
    }
}

/// A short tag for a client message, for the debug log.
fn client_tag(m: &ClientMessage) -> &'static str {
    match m {
        ClientMessage::JoinRoom { .. } => "JoinRoom",
        ClientMessage::CreateRoom { .. } => "CreateRoom",
        ClientMessage::EnqueueMatch { .. } => "EnqueueMatch",
        ClientMessage::CommitCard { .. } => "CommitCard",
        ClientMessage::CommitPass => "CommitPass",
        ClientMessage::LockIn => "LockIn",
        ClientMessage::Emote { .. } => "Emote",
        ClientMessage::Heartbeat => "Heartbeat",
    }
}
