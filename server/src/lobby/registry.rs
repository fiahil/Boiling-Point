//! The group registry: a concurrent map from invite code to the group's command
//! channel, with collision-safe group creation, the swappable content bundle, and
//! the server-owned admin command primitives.
//!
//! The active content (config + derived registry + emote palette) lives behind a
//! single [`ArcSwap`] so a reload/toggle swaps all three **atomically**; groups
//! snapshot the current bundle when they are created, so a swap affects groups
//! created afterward (matching `admin-control`: "subsequent deals exclude that
//! card"). The command primitives ([`reload`](GroupRegistry::reload),
//! [`toggle_item`](GroupRegistry::toggle_item), [`seed_group`](GroupRegistry::seed_group),
//! [`force_start`](GroupRegistry::force_start), [`kill_group`](GroupRegistry::kill_group))
//! are server-owned and exposed for the admin command plane; none is reachable from
//! the player wire. Each emits an `admin.command` audit span.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock, Weak};

use arc_swap::ArcSwap;
use dashmap::DashMap;
use sqlx::PgPool;
use tokio::sync::mpsc;

use boiling_point_protocol::GroupCode;
use boiling_point_protocol::vocab::{ModifierKind, SpellKind};

use crate::config::{ConfigError, ContentConfig};
use crate::content::ContentRegistry;

use super::codes::generate_code;
use super::group::{GroupCommand, spawn};
use super::matchmaking::MatchQueue;

/// The active content snapshot groups run against: the validated config, its derived
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

/// Names a single content item to enable/disable. Spells are addressed by their
/// kind; plain ingredient archetypes carry no stable id.
#[derive(Debug, Clone, Copy)]
pub enum ContentSelector {
    /// All grimoire entries of this spell.
    Spell(SpellKind),
    /// A modifier pool entry.
    Modifier(ModifierKind),
    /// A preset emote, by id.
    Emote(u16),
}

impl ContentSelector {
    /// Set the `enabled` flag on the selected item(s) within `config`.
    fn apply(self, config: &mut ContentConfig, enabled: bool) {
        match self {
            ContentSelector::Spell(kind) => {
                for s in config.spell.iter_mut().filter(|s| s.kind == kind) {
                    s.enabled = enabled;
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

/// Concurrent registry of live groups. Holds the shared content the groups need to
/// run games.
pub struct GroupRegistry {
    groups: DashMap<GroupCode, mpsc::Sender<GroupCommand>>,
    content: ArcSwap<Content>,
    /// Optional persistence pool, threaded into every spawned group for the
    /// post-game completion write. `None` ⇒ persistence is disabled.
    pool: Option<PgPool>,
    /// The auto-match queue, for group fill. A `Weak` to avoid a reference cycle
    /// (the queue holds an `Arc<GroupRegistry>`); set once after both are built via
    /// [`set_queue`](GroupRegistry::set_queue). Unset in tests that don't matchmake,
    /// so fill is a no-op there.
    queue: OnceLock<Weak<MatchQueue>>,
}

impl GroupRegistry {
    /// Create an empty registry sharing `registry`/`config` with every group it
    /// spawns; the emote palette is derived from the config's enabled emotes.
    /// Persistence is off by default — attach a pool with [`with_pool`](Self::with_pool).
    pub fn new(registry: Arc<ContentRegistry>, config: Arc<ContentConfig>) -> Self {
        let palette = Arc::new(
            config
                .emote
                .iter()
                .filter(|e| e.enabled)
                .map(|e| e.id)
                .collect::<HashSet<u16>>(),
        );
        GroupRegistry {
            groups: DashMap::new(),
            content: ArcSwap::from_pointee(Content {
                config,
                registry,
                palette,
            }),
            pool: None,
            queue: OnceLock::new(),
        }
    }

    /// Attach the persistence pool threaded into every group this registry spawns
    /// for the post-game write. `None` leaves persistence disabled.
    pub fn with_pool(mut self, pool: Option<PgPool>) -> Self {
        self.pool = pool;
        self
    }

    /// Wire the auto-match queue (for group fill) after both are constructed. Held
    /// as a `Weak` to avoid a registry⇄queue reference cycle.
    pub fn set_queue(&self, queue: &Arc<MatchQueue>) {
        let _ = self.queue.set(Arc::downgrade(queue));
    }

    /// Open `code` for matchmaking fill, needing `needed` more players. A no-op if
    /// no queue is wired (e.g. in unit tests).
    pub async fn open_fill(&self, code: GroupCode, tx: mpsc::Sender<GroupCommand>, needed: usize) {
        if let Some(queue) = self.queue.get().and_then(Weak::upgrade) {
            queue.open_fill(code, tx, needed).await;
        }
    }

    /// Stop filling `code` (game started or search cancelled). A no-op without a queue.
    pub fn close_fill(&self, code: &GroupCode) {
        if let Some(queue) = self.queue.get().and_then(Weak::upgrade) {
            queue.close_fill(code);
        }
    }

    /// Create a fresh group with a unique invite code; returns the code and its
    /// command channel. Takes `Arc<Self>` so the group can deregister itself when
    /// it ends (idle timeout or game over). The group snapshots the *current*
    /// content bundle.
    pub fn create(self: &Arc<Self>) -> (GroupCode, mpsc::Sender<GroupCommand>) {
        let content = self.content.load_full();
        loop {
            let code = generate_code();
            if !self.groups.contains_key(&code) {
                let handle = spawn(
                    code.clone(),
                    Arc::clone(self),
                    content.registry.clone(),
                    content.config.clone(),
                    content.palette.clone(),
                    self.pool.clone(),
                );
                self.groups.insert(code.clone(), handle.tx.clone());
                crate::observability::metric::group_created();
                tracing::info!(code = %code.0, "group created");
                return (code, handle.tx);
            }
        }
    }

    /// Look up an existing group's command channel by code.
    pub fn get(&self, code: &GroupCode) -> Option<mpsc::Sender<GroupCommand>> {
        self.groups.get(code).map(|r| r.clone())
    }

    /// Remove a group from the registry (called by a group when it ends).
    pub fn remove(&self, code: &GroupCode) {
        self.groups.remove(code);
    }

    /// Number of live groups (for metrics/tests).
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Whether there are no live groups.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    // ---- Admin command primitives (server-owned; never on the player wire) ----

    /// Validate a new content config from TOML and swap it in atomically. On a
    /// validation failure the running config is unchanged and the `ConfigError` is
    /// returned. Affects groups created after the swap.
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
    /// size invariants are kept in step with the enabled items so disabling a spell
    /// is accepted while a toggle that breaks a rule (e.g. emptying a pool or
    /// removing a required spell kind) is rejected with the running config unchanged.
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
        config.grimoire.size = config
            .spell
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.copies)
            .sum();
        let outcome = Content::build(config).map(|content| self.content.store(Arc::new(content)));
        record_config_outcome(&span, &outcome);
        outcome
    }

    /// Seed (create) a fresh group and return its invite code.
    pub fn seed_group(self: &Arc<Self>, operator: &str) -> GroupCode {
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

    /// Force-start a group that has not yet auto-started. Returns whether the command
    /// was delivered (the group exists and accepted it).
    pub fn force_start(&self, code: &GroupCode, operator: &str) -> bool {
        self.deliver(code, operator, "force_start", GroupCommand::ForceStart)
    }

    /// Kill a group (idle or stuck): the group's task tears it down and its
    /// `group.lifetime` span ends. Returns whether the command was delivered.
    pub fn kill_group(&self, code: &GroupCode, operator: &str) -> bool {
        self.deliver(code, operator, "kill", GroupCommand::Shutdown)
    }

    /// Deliver a lifecycle command to a group's task, auditing the action/outcome.
    fn deliver(&self, code: &GroupCode, operator: &str, action: &str, cmd: GroupCommand) -> bool {
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
        span.record("outcome", if ok { "ok" } else { "no_such_group" });
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

    fn registry_with(config: ContentConfig) -> Arc<GroupRegistry> {
        let reg = Arc::new(config.build_registry().unwrap());
        Arc::new(GroupRegistry::new(reg, Arc::new(config)))
    }

    fn default_registry() -> Arc<GroupRegistry> {
        registry_with(default_config())
    }

    #[test]
    fn valid_reload_applies() {
        let reg = default_registry();
        assert!(
            reg.reload(include_str!("../../content.toml"), "tester")
                .is_ok()
        );
        assert_eq!(reg.content.load().registry.pantry_size(), 30);
        assert_eq!(reg.content.load().registry.grimoire_size(), 20);
    }

    #[test]
    fn invalid_reload_is_rejected_and_config_unchanged() {
        let reg = default_registry();
        let before = reg.content.load().registry.pantry_size();
        assert!(
            reg.reload("this is not valid config {{{", "tester")
                .is_err()
        );
        assert_eq!(
            reg.content.load().registry.pantry_size(),
            before,
            "a rejected reload must leave the running config unchanged"
        );
    }

    #[test]
    fn disabling_a_spell_takes_effect_for_new_groups() {
        let reg = default_registry();
        let outcome = reg.toggle_item(ContentSelector::Spell(SpellKind::Quench), false, "tester");
        assert!(outcome.is_ok(), "disabling one spell should validate");
        let content = reg.content.load();
        assert!(
            !content
                .registry
                .spells()
                .iter()
                .any(|s| s.kind == SpellKind::Quench),
            "Quench should be gone from the running registry"
        );
        assert_eq!(
            content.registry.grimoire_size(),
            19,
            "grimoire shrank by the disabled spell"
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
    async fn seed_group_creates_a_group() {
        let reg = default_registry();
        let before = reg.len();
        let code = reg.seed_group("tester");
        assert!(reg.get(&code).is_some());
        assert!(reg.len() > before);
    }

    #[tokio::test]
    async fn kill_group_tears_it_down() {
        let reg = default_registry();
        let (code, _tx) = reg.create();
        assert!(reg.get(&code).is_some());
        assert!(reg.kill_group(&code, "tester"), "kill should be delivered");
        // The group task processes Shutdown and deregisters (ending group.lifetime).
        let mut gone = false;
        for _ in 0..200 {
            if reg.get(&code).is_none() {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(gone, "a killed group must be deregistered");
        // Killing an unknown group is a no-op that reports failure.
        assert!(!reg.kill_group(&code, "tester"));
    }

    #[tokio::test]
    async fn force_start_begins_the_game() {
        let reg = default_registry();
        let (code, tx) = reg.create();
        let (out_tx, mut out_rx) = mpsc::channel(64);
        tx.send(GroupCommand::Join {
            player: PlayerId(Uuid::new_v4()),
            name: "solo".into(),
            session_token: String::new(),
            guest: false,
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
        tx.send(GroupCommand::Join {
            player,
            name: "solo".into(),
            session_token: String::new(),
            guest: false,
            out: out_tx,
        })
        .await
        .unwrap();
        // Drain the GroupJoined ack.
        let _ = tokio::time::timeout(Duration::from_secs(1), out_rx.recv()).await;

        // Every gameplay/table ClientMessage routes through GroupCommand::Action and
        // must not start a game or tear the group down.
        for msg in [
            ClientMessage::LockIn,
            ClientMessage::CommitPass,
            ClientMessage::Heartbeat,
        ] {
            tx.send(GroupCommand::Action { player, msg }).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            reg.get(&code).is_some(),
            "player messages must not kill the group"
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
