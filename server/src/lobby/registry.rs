//! The room registry: a concurrent map from invite code to the room's command
//! channel, with collision-safe room creation, the swappable content bundle, and
//! the server-owned admin command primitives.
//!
//! The active content (config + derived registry + emote palette) lives behind a
//! single [`ArcSwap`] so a reload/toggle swaps all three **atomically**; rooms
//! snapshot the current bundle when they are created, so a swap affects rooms
//! created afterward (matching `admin-control`: "subsequent deals exclude that
//! card"). The command primitives ([`reload`](RoomRegistry::reload),
//! [`toggle_item`](RoomRegistry::toggle_item), [`seed_room`](RoomRegistry::seed_room),
//! [`force_start`](RoomRegistry::force_start), [`kill_room`](RoomRegistry::kill_room))
//! are server-owned and exposed for the admin command plane; none is reachable from
//! the player wire. Each emits an `admin.command` audit span.

use std::collections::HashSet;
use std::sync::Arc;

use arc_swap::ArcSwap;
use dashmap::DashMap;
use tokio::sync::mpsc;

use boiling_point_protocol::RoomCode;
use boiling_point_protocol::vocab::{EffectKind, ModifierKind};

use crate::config::{ConfigError, ContentConfig};
use crate::content::ContentRegistry;

use super::codes::generate_code;
use super::room::{RoomCommand, spawn};

/// The active content snapshot rooms run against: the validated config, its derived
/// registry, and the enabled-emote palette. Swapped atomically as a unit.
struct Content {
    config: Arc<ContentConfig>,
    registry: Arc<ContentRegistry>,
    palette: Arc<HashSet<u16>>,
}

impl Content {
    /// Validate `config` (via `build_registry`) and assemble the bundle. Returns the
    /// config's own `ConfigError` if it is invalid — nothing is partially built.
    fn build(config: ContentConfig) -> Result<Self, ConfigError> {
        let registry = config.build_registry()?;
        let palette = config
            .emote
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.id)
            .collect::<HashSet<u16>>();
        Ok(Content {
            config: Arc::new(config),
            registry: Arc::new(registry),
            palette: Arc::new(palette),
        })
    }
}

/// Names a single content item to enable/disable. Cards are addressed by their
/// effect (the deck's special cards); plain colour cards carry no stable id.
#[derive(Debug, Clone, Copy)]
pub enum ContentSelector {
    /// All cards carrying this effect.
    Effect(EffectKind),
    /// A modifier pool entry.
    Modifier(ModifierKind),
    /// A preset emote, by id.
    Emote(u16),
}

impl ContentSelector {
    /// Set the `enabled` flag on the selected item(s) within `config`.
    fn apply(self, config: &mut ContentConfig, enabled: bool) {
        match self {
            ContentSelector::Effect(kind) => {
                for c in config.card.iter_mut().filter(|c| c.effect == Some(kind)) {
                    c.enabled = enabled;
                }
            }
            ContentSelector::Modifier(kind) => {
                for m in config.modifier.iter_mut().filter(|m| m.kind == kind) {
                    m.enabled = enabled;
                }
            }
            ContentSelector::Emote(id) => {
                for e in config.emote.iter_mut().filter(|e| e.id == id) {
                    e.enabled = enabled;
                }
            }
        }
    }
}

/// Concurrent registry of live rooms. Holds the shared content the rooms need to
/// run games.
pub struct RoomRegistry {
    rooms: DashMap<RoomCode, mpsc::Sender<RoomCommand>>,
    content: ArcSwap<Content>,
}

impl RoomRegistry {
    /// Create an empty registry sharing `registry`/`config` with every room it
    /// spawns; the emote palette is derived from the config's enabled emotes.
    pub fn new(registry: Arc<ContentRegistry>, config: Arc<ContentConfig>) -> Self {
        let palette = Arc::new(
            config
                .emote
                .iter()
                .filter(|e| e.enabled)
                .map(|e| e.id)
                .collect::<HashSet<u16>>(),
        );
        RoomRegistry {
            rooms: DashMap::new(),
            content: ArcSwap::from_pointee(Content {
                config,
                registry,
                palette,
            }),
        }
    }

    /// Create a fresh room with a unique invite code; returns the code and its
    /// command channel. Takes `Arc<Self>` so the room can deregister itself when
    /// it ends (idle timeout or game over). The room snapshots the *current*
    /// content bundle.
    pub fn create(self: &Arc<Self>) -> (RoomCode, mpsc::Sender<RoomCommand>) {
        let content = self.content.load_full();
        loop {
            let code = generate_code();
            if !self.rooms.contains_key(&code) {
                let handle = spawn(
                    code.clone(),
                    Arc::clone(self),
                    content.registry.clone(),
                    content.config.clone(),
                    content.palette.clone(),
                );
                self.rooms.insert(code.clone(), handle.tx.clone());
                crate::observability::metric::room_created();
                tracing::info!(code = %code.0, "room created");
                return (code, handle.tx);
            }
        }
    }

    /// Look up an existing room's command channel by code.
    pub fn get(&self, code: &RoomCode) -> Option<mpsc::Sender<RoomCommand>> {
        self.rooms.get(code).map(|r| r.clone())
    }

    /// Remove a room from the registry (called by a room when it ends).
    pub fn remove(&self, code: &RoomCode) {
        self.rooms.remove(code);
    }

    /// Number of live rooms (for metrics/tests).
    pub fn len(&self) -> usize {
        self.rooms.len()
    }

    /// Whether there are no live rooms.
    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }

    // ---- Admin command primitives (server-owned; never on the player wire) ----

    /// Validate a new content config from TOML and swap it in atomically. On a
    /// validation failure the running config is unchanged and the `ConfigError` is
    /// returned. Affects rooms created after the swap.
    pub fn reload(&self, toml: &str, operator: &str) -> Result<(), ConfigError> {
        let span = tracing::info_span!(
            "admin.command",
            operator,
            action = "reload",
            target = "config",
            outcome = tracing::field::Empty,
        );
        let _enter = span.enter();
        let outcome = ContentConfig::from_toml(toml)
            .and_then(Content::build)
            .map(|content| self.content.store(Arc::new(content)));
        record_config_outcome(&span, &outcome);
        outcome
    }

    /// Enable/disable a single content item, re-validating exactly as a reload. The
    /// deck-size invariant is kept in step with the enabled cards so disabling a
    /// card is accepted while a toggle that breaks a rule (e.g. emptying a pool) is
    /// rejected with the running config unchanged.
    pub fn toggle_item(
        &self,
        selector: ContentSelector,
        enabled: bool,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let span = tracing::info_span!(
            "admin.command",
            operator,
            action = "toggle",
            target = ?selector,
            outcome = tracing::field::Empty,
        );
        let _enter = span.enter();
        let mut config = (*self.content.load_full().config).clone();
        selector.apply(&mut config, enabled);
        config.deck.size = config
            .card
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.copies)
            .sum();
        let outcome = Content::build(config).map(|content| self.content.store(Arc::new(content)));
        record_config_outcome(&span, &outcome);
        outcome
    }

    /// Seed (create) a fresh room and return its invite code.
    pub fn seed_room(self: &Arc<Self>, operator: &str) -> RoomCode {
        let span = tracing::info_span!(
            "admin.command",
            operator,
            action = "seed",
            target = tracing::field::Empty,
            outcome = "ok",
        );
        let _enter = span.enter();
        let (code, _tx) = self.create();
        span.record("target", code.0.as_str());
        code
    }

    /// Force-start a room that has not yet auto-started. Returns whether the command
    /// was delivered (the room exists and accepted it).
    pub fn force_start(&self, code: &RoomCode, operator: &str) -> bool {
        self.deliver(code, operator, "force_start", RoomCommand::ForceStart)
    }

    /// Kill a room (idle or stuck): the room's task tears it down and its
    /// `room.lifetime` span ends. Returns whether the command was delivered.
    pub fn kill_room(&self, code: &RoomCode, operator: &str) -> bool {
        self.deliver(code, operator, "kill", RoomCommand::Shutdown)
    }

    /// Deliver a lifecycle command to a room's task, auditing the action/outcome.
    fn deliver(&self, code: &RoomCode, operator: &str, action: &str, cmd: RoomCommand) -> bool {
        let span = tracing::info_span!(
            "admin.command",
            operator,
            action,
            target = %code.0,
            outcome = tracing::field::Empty,
        );
        let _enter = span.enter();
        let ok = match self.get(code) {
            Some(tx) => tx.try_send(cmd).is_ok(),
            None => false,
        };
        span.record("outcome", if ok { "ok" } else { "no_such_room" });
        ok
    }
}

/// Record the `outcome` attribute on a command audit span from a config result.
fn record_config_outcome(span: &tracing::Span, outcome: &Result<(), ConfigError>) {
    match outcome {
        Ok(()) => {
            span.record("outcome", "ok");
        }
        Err(e) => {
            span.record("outcome", tracing::field::display(e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use boiling_point_protocol::{ClientMessage, PlayerId, ServerMessage};
    use uuid::Uuid;

    /// The checked-in default config, with short wave timers so async tests don't
    /// wait out the real 30s/10s budgets.
    fn default_config() -> ContentConfig {
        let mut c = ContentConfig::from_toml(include_str!("../../content.toml")).unwrap();
        c.timing.wave1_ms = 200;
        c.timing.wave_ms = 150;
        c
    }

    fn registry_with(config: ContentConfig) -> Arc<RoomRegistry> {
        let reg = Arc::new(config.build_registry().unwrap());
        Arc::new(RoomRegistry::new(reg, Arc::new(config)))
    }

    fn default_registry() -> Arc<RoomRegistry> {
        registry_with(default_config())
    }

    #[test]
    fn valid_reload_applies() {
        let reg = default_registry();
        assert!(
            reg.reload(include_str!("../../content.toml"), "tester")
                .is_ok()
        );
        assert_eq!(reg.content.load().registry.deck_size(), 90);
    }

    #[test]
    fn invalid_reload_is_rejected_and_config_unchanged() {
        let reg = default_registry();
        let before = reg.content.load().registry.deck_size();
        assert!(
            reg.reload("this is not valid config {{{", "tester")
                .is_err()
        );
        assert_eq!(
            reg.content.load().registry.deck_size(),
            before,
            "a rejected reload must leave the running config unchanged"
        );
    }

    #[test]
    fn disabling_a_card_takes_effect_for_new_rooms() {
        let reg = default_registry();
        let outcome = reg.toggle_item(ContentSelector::Effect(EffectKind::Shield), false, "tester");
        assert!(outcome.is_ok(), "disabling one effect card should validate");
        let content = reg.content.load();
        assert!(
            content.registry.effect(EffectKind::Shield).is_none(),
            "Shield should be gone from the running registry"
        );
        assert_eq!(
            content.registry.deck_size(),
            89,
            "deck shrank by the disabled card"
        );
    }

    #[test]
    fn invalid_toggle_is_rejected_and_config_unchanged() {
        // A config whose modifier pool sits exactly at the minimum (Residue's 4
        // copies), so disabling it would empty the pool below the draw requirement.
        let mut config = default_config();
        for m in config.modifier.iter_mut() {
            m.enabled = m.kind == ModifierKind::Residue;
        }
        let reg = registry_with(config);

        let outcome = reg.toggle_item(
            ContentSelector::Modifier(ModifierKind::Residue),
            false,
            "tester",
        );
        assert!(
            matches!(outcome, Err(ConfigError::ModifierPoolTooSmall { .. })),
            "emptying the modifier pool must be rejected, got {outcome:?}"
        );
        assert!(
            reg.content
                .load()
                .registry
                .modifier_pool()
                .iter()
                .any(|(k, _)| *k == ModifierKind::Residue),
            "a rejected toggle must leave the running config unchanged"
        );
    }

    #[tokio::test]
    async fn seed_room_creates_a_room() {
        let reg = default_registry();
        let before = reg.len();
        let code = reg.seed_room("tester");
        assert!(reg.get(&code).is_some());
        assert!(reg.len() > before);
    }

    #[tokio::test]
    async fn kill_room_tears_it_down() {
        let reg = default_registry();
        let (code, _tx) = reg.create();
        assert!(reg.get(&code).is_some());
        assert!(reg.kill_room(&code, "tester"), "kill should be delivered");
        // The room task processes Shutdown and deregisters (ending room.lifetime).
        let mut gone = false;
        for _ in 0..200 {
            if reg.get(&code).is_none() {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(gone, "a killed room must be deregistered");
        // Killing an unknown room is a no-op that reports failure.
        assert!(!reg.kill_room(&code, "tester"));
    }

    #[tokio::test]
    async fn force_start_begins_the_game() {
        let reg = default_registry();
        let (code, tx) = reg.create();
        let (out_tx, mut out_rx) = mpsc::channel(64);
        tx.send(RoomCommand::Join {
            player: PlayerId(Uuid::new_v4()),
            name: "solo".into(),
            out: out_tx,
        })
        .await
        .unwrap();

        assert!(
            reg.force_start(&code, "tester"),
            "force-start should be delivered"
        );

        // A GameStarting message should arrive without the table ever filling.
        let mut started = false;
        while let Ok(Some(msg)) = tokio::time::timeout(Duration::from_secs(3), out_rx.recv()).await
        {
            if matches!(msg, ServerMessage::GameStarting { .. }) {
                started = true;
                break;
            }
        }
        assert!(
            started,
            "force-start should begin the game for the seated player"
        );
    }

    #[tokio::test]
    async fn player_messages_never_force_start_or_kill() {
        let reg = default_registry();
        let (code, tx) = reg.create();
        let (out_tx, mut out_rx) = mpsc::channel(64);
        let player = PlayerId(Uuid::new_v4());
        tx.send(RoomCommand::Join {
            player,
            name: "solo".into(),
            out: out_tx,
        })
        .await
        .unwrap();
        // Drain the RoomJoined ack.
        let _ = tokio::time::timeout(Duration::from_secs(1), out_rx.recv()).await;

        // Every gameplay/table ClientMessage routes through RoomCommand::Action and
        // must not start a game or tear the room down.
        for msg in [
            ClientMessage::LockIn,
            ClientMessage::CommitPass,
            ClientMessage::Heartbeat,
        ] {
            tx.send(RoomCommand::Action { player, msg }).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            reg.get(&code).is_some(),
            "player messages must not kill the room"
        );
        let mut saw_game_starting = false;
        while let Ok(Some(m)) = tokio::time::timeout(Duration::from_millis(50), out_rx.recv()).await
        {
            if matches!(m, ServerMessage::GameStarting { .. }) {
                saw_game_starting = true;
            }
        }
        assert!(!saw_game_starting, "player messages must not start a game");
    }
}
