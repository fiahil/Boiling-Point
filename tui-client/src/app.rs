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
    ClientMessage, PROTOCOL_VERSION,
    ids::{CardId, EmoteId, GroupCode},
    server::ServerMessage,
    vocab::{EffectKind, HandCard},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Frame, Terminal, backend::TestBackend, buffer::Buffer};

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
    /// The session token learned from `GroupJoined`, replayed on every entry
    /// message so this connection's identity (and a held seat) survives a socket
    /// drop. `None` until the first join.
    pub(crate) session_token: Option<String>,
    pub(crate) menu_index: usize,
    pub(crate) cursor: usize,
    pub(crate) committed: Selection,
    pub(crate) locked_in: bool,
    pub(crate) countdown_ms: Option<u32>,
    pub(crate) my_pot: Vec<HandCard>,
    pub(crate) recall: Option<RecallPrompt>,
    pub(crate) emote_open: bool,
    /// Whether the `?` effect/modifier Codex overlay is open.
    pub(crate) codex_open: bool,
    /// Free-running animation clock (ms), advanced by [`App::on_tick`]. Drives the
    /// ambient cauldron motion; pinned in snapshot tests for determinism.
    pub(crate) anim_ms: u32,
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
            session_token: None,
            menu_index: 0,
            cursor: 0,
            committed: Selection::None,
            locked_in: false,
            countdown_ms: None,
            my_pot: Vec::new(),
            recall: None,
            emote_open: false,
            codex_open: false,
            anim_ms: 0,
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

    /// Build the matchmaking-queue entry intent for a scripted (non-interactive)
    /// launch, advancing past the entry menu exactly as choosing "auto-match"
    /// would. Uses the pre-filled display name, falling back to a generic label.
    pub fn auto_enqueue(&mut self) -> Vec<ClientMessage> {
        let name = match self.name_input.trim() {
            "" => "Player".to_string(),
            n => n.to_string(),
        };
        self.phase = Phase::Queue;
        vec![ClientMessage::EnqueueMatch {
            protocol_version: PROTOCOL_VERSION,
            display_name: name,
            session_token: self.session_token.clone(),
        }]
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
            ServerMessage::GroupJoined { session_token, .. } => {
                // Persist the session token so our identity survives a socket drop.
                self.session_token = Some(session_token.clone());
                self.phase = Phase::Lobby;
            }
            ServerMessage::GameStarting { .. } => {
                // A fresh game (first or play-again): clear the previous game's
                // per-game state so a replayed table doesn't render stale results.
                self.begin_next_game();
                self.phase = Phase::RoundStart;
            }
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
                if let (Selection::Card(id), Some(me)) = (self.committed, self.vm.me)
                    && played.contains(&me)
                    && let Some(pos) = self.vm.hand.iter().position(|c| c.id == id)
                {
                    let card = self.vm.hand.remove(pos);
                    self.my_pot.push(card);
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
            ServerMessage::StateSnapshot { round_number, .. } => {
                // Resume on rejoin. A disconnected player auto-passes each wave,
                // so on a mid-round rejoin they are locked out for the round.
                self.phase = if *round_number >= 1 {
                    Phase::Playing
                } else {
                    Phase::Lobby
                };
                self.committed = Selection::Pass;
                self.locked_in = true;
                self.toast("reconnected — locked out of this round while away");
            }
            ServerMessage::DeathmatchStarted { .. } => {
                self.toast("⚔ Deathmatch — tie for the lead!");
            }
            ServerMessage::LeftGroup => {
                // The server confirmed we left; drop the table and return to the
                // main menu (the connection stays open — identity is kept).
                self.return_to_menu();
                self.toast("left the group");
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
        // Free-running clock for ambient (information-free) animation. Wraps so a
        // long session never overflows; rendering only ever reads it modulo a phase.
        self.anim_ms = self.anim_ms.wrapping_add(dt_ms);
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
        if self.phase == Phase::Depile
            && let Some(total) = self.depile_total()
            && self.depile_shown < total
        {
            self.depile_accum_ms += dt_ms;
            while self.depile_accum_ms >= DEPILE_STEP_MS && self.depile_shown < total {
                self.depile_shown += 1;
                self.depile_accum_ms -= DEPILE_STEP_MS;
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
        // Once the seat is abandoned there is nothing to play; any key returns to
        // the entry menu (which also clears the connection state) rather than
        // leaving the player stranded behind the overlay.
        if matches!(self.conn, Conn::Abandoned) {
            self.reset_for_new_game();
            return vec![];
        }
        // A Recall picker, when open, captures input.
        if self.recall.is_some() {
            return self.key_recall(key.code);
        }
        // The Codex overlay captures input while open; `?`/Esc dismiss it.
        if self.codex_open {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.codex_open = false;
            }
            return vec![];
        }
        // `?` opens the Codex from any in-game screen (not the text-entry menus,
        // where `?` is literal input into the name/code field).
        if key.code == KeyCode::Char('?') && !matches!(self.phase, Phase::Entry | Phase::JoinCode) {
            self.codex_open = true;
            return vec![];
        }
        match self.phase {
            Phase::Entry => self.key_entry(key.code),
            Phase::JoinCode => self.key_joincode(key.code),
            // Queue/connecting: nothing to do but wait for the table to fill.
            Phase::Connecting | Phase::Queue => vec![],
            // In a group lobby, waiting for the table to fill — but the player may
            // leave back to the menu on the same connection.
            Phase::Lobby => self.key_lobby(key.code),
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
                            session_token: self.session_token.clone(),
                        }]
                    }
                    1 => {
                        self.phase = Phase::Connecting;
                        vec![ClientMessage::CreateGroup {
                            protocol_version: PROTOCOL_VERSION,
                            display_name: name,
                            session_token: self.session_token.clone(),
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
                vec![ClientMessage::JoinGroup {
                    protocol_version: PROTOCOL_VERSION,
                    display_name: self.name_input.trim().to_string(),
                    session_token: self.session_token.clone(),
                    group_code: GroupCode(self.code_input.trim().to_string()),
                }]
            }
            _ => vec![],
        }
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
        if matches!(code, KeyCode::Enter | KeyCode::Char(' '))
            && let Some(total) = self.depile_total()
        {
            self.depile_shown = total;
        }
        vec![]
    }

    /// In a group lobby (waiting for the table), `Esc`/`q` leaves the group and
    /// returns to the main menu on the same connection.
    fn key_lobby(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => vec![ClientMessage::LeaveGroup],
            _ => vec![],
        }
    }

    fn key_gameover(&mut self, code: KeyCode) -> Vec<ClientMessage> {
        match code {
            // Play again with the same table: opt in. The server re-deals once all
            // four seats have opted in; we stay on this screen until `GameStarting`.
            KeyCode::Char('r') | KeyCode::Enter => {
                self.toast("ready — waiting for the table to play again");
                vec![ClientMessage::PlayAgain]
            }
            // Leave the group and return to the main menu (the server frees the seat
            // and replies `LeftGroup`, which resets us to the entry screen).
            KeyCode::Char('m') | KeyCode::Esc => vec![ClientMessage::LeaveGroup],
            KeyCode::Char('q') => {
                self.should_quit = true;
                vec![]
            }
            _ => vec![],
        }
    }

    /// Clear the previous game's per-game state ahead of a fresh game with the same
    /// group (play-again), keeping the roster, identity, and group code.
    fn begin_next_game(&mut self) {
        self.vm.reset_for_next_game();
        self.my_pot.clear();
        self.committed = Selection::None;
        self.locked_in = false;
        self.recall = None;
        self.emote_open = false;
        self.codex_open = false;
        self.depile_shown = 0;
        self.depile_accum_ms = 0;
        self.boom_ms = 0;
        self.peek_modal_ms = 0;
    }

    /// Return to the main (unbound) menu, keeping the session token so identity is
    /// retained. Used after `LeftGroup`.
    fn return_to_menu(&mut self) {
        self.vm = ViewModel::default();
        self.phase = Phase::Entry;
        self.conn = Conn::Connected;
        self.code_input.clear();
        self.menu_index = 0;
        self.begin_next_game();
    }

    fn reset_for_new_game(&mut self) {
        self.vm = ViewModel::default();
        self.phase = Phase::Entry;
        self.conn = Conn::Connected;
        self.committed = Selection::None;
        self.locked_in = false;
        self.countdown_ms = None;
        self.my_pot.clear();
        self.recall = None;
        self.emote_open = false;
        self.codex_open = false;
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

    /// Move the hand cursor (test/mock) so the inspector can be exercised without
    /// synthesising key events. A value at or past the hand length selects Pass.
    pub fn set_cursor(&mut self, i: usize) {
        self.cursor = i;
    }

    /// Open the `?` Codex overlay (test/mock).
    pub fn open_codex(&mut self) {
        self.codex_open = true;
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
        ServerMessage::GroupJoined { .. } => "GroupJoined",
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
        ServerMessage::StateSnapshot { .. } => "StateSnapshot",
        ServerMessage::DeathmatchStarted { .. } => "DeathmatchStarted",
        ServerMessage::LeftGroup => "LeftGroup",
        ServerMessage::Heartbeat => "Heartbeat",
    }
}

/// A short tag for a client message, for the debug log.
fn client_tag(m: &ClientMessage) -> &'static str {
    match m {
        ClientMessage::JoinGroup { .. } => "JoinGroup",
        ClientMessage::CreateGroup { .. } => "CreateGroup",
        ClientMessage::EnqueueMatch { .. } => "EnqueueMatch",
        ClientMessage::CommitCard { .. } => "CommitCard",
        ClientMessage::CommitPass => "CommitPass",
        ClientMessage::LockIn => "LockIn",
        ClientMessage::Emote { .. } => "Emote",
        ClientMessage::PlayAgain => "PlayAgain",
        ClientMessage::LeaveGroup => "LeaveGroup",
        ClientMessage::Heartbeat => "Heartbeat",
    }
}
