//! The client's **player-visible view model** — the entire state the client
//! renders, built *only* from received [`ServerMessage`]s.
//!
//! Like the bot harness, this model is deliberately narrow: it has no field for
//! the boiling point (except [`ViewModel::my_peek`], which only exists because
//! the server privately told *this* player), no other players' hands, and no
//! draw deck. The secret boundary holds by construction — there is nowhere here
//! to store a secret the server never sent.

use boiling_point_protocol::{
    CardId, Color, HandCard, ModifierKind, PlayerId,
    server::{Contribution, DepileEntry, PlayerPublic, PlayerScore, ScoringOutcome, ServerMessage},
};
use serde::Serialize;

/// Which screen the client is showing. `Entry`/`JoinCode` are client-only
/// pre-connection screens; the rest are driven by the server message stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum Phase {
    /// The opening menu: quick-match / create-group / join-by-code + name entry.
    Entry,
    /// Typing an invite code.
    JoinCode,
    /// An entry message was sent; awaiting `GroupJoined` or an `Error`.
    Connecting,
    /// In the auto-match queue, awaiting a table.
    Queue,
    /// In a group, showing the seat roster until the game starts.
    Lobby,
    /// A round is about to begin: the modifier reveal and refilled hand.
    RoundStart,
    /// A wave is open or resolving.
    Playing,
    /// The end-of-round reverse-order reveal.
    Depile,
    /// The round result (safe brew or explosion).
    Scoring,
    /// The game is over.
    GameOver,
}

/// Public, per-player state the client is permitted to know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PlayerView {
    /// Stable id.
    pub(crate) id: PlayerId,
    /// Display name.
    pub(crate) name: String,
    /// Assigned colour.
    pub(crate) color: Color,
    /// Whether currently connected.
    pub(crate) connected: bool,
    /// Whether this player is a matchmaking guest (not a group member).
    pub(crate) guest: bool,
    /// Cumulative score (authoritative via `ScoreUpdate`).
    pub(crate) score: i32,
    /// Cards contributed to the current pot (public political signal).
    pub(crate) contributed: u8,
}

impl PlayerView {
    fn from_public(p: &PlayerPublic) -> Self {
        PlayerView {
            id: p.id,
            name: p.display_name.clone(),
            color: p.color,
            connected: p.connected,
            guest: p.guest,
            score: 0,
            contributed: 0,
        }
    }
}

/// One member's line in the group's live standings (as the client renders it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct StandingView {
    /// The member.
    pub(crate) player: PlayerId,
    /// Games played in the group.
    pub(crate) games_played: u32,
    /// Games won.
    pub(crate) wins: u32,
}

/// The group's standings as the client holds them: per-member rows + guest aggregate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct StandingsView {
    /// Per-member rows.
    pub(crate) members: Vec<StandingView>,
    /// Games that included a guest.
    pub(crate) guest_games: u32,
    /// Games won by a guest.
    pub(crate) guest_wins: u32,
}

/// A captured depile for the reveal screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DepileView {
    /// Revealed cards, last-added first.
    pub(crate) reveals: Vec<DepileEntry>,
    /// Whether the round exploded.
    pub(crate) exploded: bool,
    /// Boiling point — `Some` only on explosion.
    pub(crate) boiling_point: Option<u8>,
    /// Index of the crossing card, if exploded.
    pub(crate) crossing_index: Option<usize>,
    /// Total pot volatility (the running total after the last-played card).
    pub(crate) total_volatility: u8,
}

/// A captured safe-brew scoring result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ScoringView {
    /// Per-colour point totals used to decide dominance.
    pub(crate) color_points: Vec<(Color, u32)>,
    /// The dominance outcome.
    pub(crate) outcome: ScoringOutcome,
    /// Points awarded this round.
    pub(crate) awards: Vec<PlayerScore>,
}

/// A captured explosion result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ExplosionView {
    /// The pot value everyone lost.
    pub(crate) pot_value: u32,
    /// Per-player deltas applied.
    pub(crate) deltas: Vec<PlayerScore>,
    /// Players who were shielded.
    pub(crate) shielded: Vec<PlayerId>,
}

/// Everything the client knows and renders. Built solely from messages.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ViewModel {
    /// This client's player id.
    pub(crate) me: Option<PlayerId>,
    /// This client's colour.
    pub(crate) my_color: Option<Color>,
    /// The group's invite code.
    pub(crate) group_code: Option<String>,
    /// When the group is matchmaking for fill, how many more players it needs
    /// ("looking for a 4th…"); `None` when not searching.
    pub(crate) searching_needed: Option<u8>,
    /// The group's live standings, if received.
    pub(crate) standings: Option<StandingsView>,
    /// Everyone at the table.
    pub(crate) players: Vec<PlayerView>,
    /// Total rounds in the game.
    pub(crate) round_count: u8,
    /// Current 1-based round.
    pub(crate) round_number: u8,
    /// Current 1-based wave within the round.
    pub(crate) wave_number: u8,
    /// Cumulative active cauldron modifiers.
    pub(crate) active_modifiers: Vec<ModifierKind>,
    /// The modifier just revealed this round (for the round-start highlight).
    pub(crate) new_modifier: Option<ModifierKind>,
    /// This client's private hand.
    pub(crate) hand: Vec<HandCard>,
    /// Cards newly drawn into the hand this round (marked in the round-start view).
    pub(crate) new_card_ids: Vec<CardId>,
    /// Total face-down cards in the cauldron.
    pub(crate) cauldron_count: u8,
    /// The boiling point, iff this client peeked this round (private knowledge).
    pub(crate) my_peek: Option<u8>,
    /// The most recent depile.
    pub(crate) last_depile: Option<DepileView>,
    /// The most recent safe-brew scoring.
    pub(crate) last_scoring: Option<ScoringView>,
    /// The most recent explosion.
    pub(crate) last_explosion: Option<ExplosionView>,
    /// Whether the current wave is the one-player final wave.
    pub(crate) final_wave: bool,
    /// Whether the game went to the Deathmatch tiebreaker (set by
    /// `DeathmatchStarted`); the outcome arrives via `GameOver`.
    pub(crate) deathmatch: bool,
    /// The players contesting the Deathmatch, if one was reached.
    pub(crate) dm_participants: Vec<PlayerId>,
    /// Whether the game has ended.
    pub(crate) game_over: bool,
    /// The winner(s) at game over (multiple ⇒ co-champions).
    pub(crate) winners: Vec<PlayerId>,
    /// Final cumulative scores at game over.
    pub(crate) final_scores: Vec<PlayerScore>,
}

impl ViewModel {
    /// Whether a hand card was newly drawn this round.
    pub(crate) fn is_new(&self, id: CardId) -> bool {
        self.new_card_ids.contains(&id)
    }

    /// The player who owns `id`, if known.
    pub(crate) fn player(&self, id: PlayerId) -> Option<&PlayerView> {
        self.players.iter().find(|p| p.id == id)
    }

    /// Fold a single server message into the model. Pure data — no I/O, no
    /// phase decisions (the [`crate::app::App`] derives phase around this).
    pub(crate) fn apply(&mut self, msg: &ServerMessage) {
        match msg {
            ServerMessage::GroupJoined {
                group_code,
                your_player_id,
                your_color,
                players,
                ..
            } => {
                self.me = Some(*your_player_id);
                self.my_color = Some(*your_color);
                self.group_code = Some(group_code.0.clone());
                self.players = players.iter().map(PlayerView::from_public).collect();
            }
            ServerMessage::GameStarting {
                players,
                round_count,
            } => {
                self.round_count = *round_count;
                self.merge_players(players);
            }
            ServerMessage::YourHand { cards } => {
                let old: Vec<CardId> = self.hand.iter().map(|c| c.id).collect();
                self.new_card_ids = cards
                    .iter()
                    .map(|c| c.id)
                    .filter(|id| !old.contains(id))
                    .collect();
                self.hand = cards.clone();
            }
            ServerMessage::WaveOpened {
                round_number,
                wave_number,
                timer_ms: _,
                final_wave,
            } => {
                self.round_number = *round_number;
                self.wave_number = *wave_number;
                if *wave_number == 1 {
                    self.reset_pot();
                }
                self.final_wave = *final_wave;
                self.new_modifier = None;
            }
            ServerMessage::WaveResolved {
                cauldron_card_count,
                contributions,
                ..
            } => {
                self.cauldron_count = *cauldron_card_count;
                self.apply_contributions(contributions);
            }
            ServerMessage::ModifierRevealed {
                modifier,
                round_number,
            } => {
                self.round_number = *round_number;
                self.new_modifier = Some(*modifier);
                if !self.active_modifiers.contains(modifier) {
                    self.active_modifiers.push(*modifier);
                }
            }
            ServerMessage::PeekResult { boiling_point } => {
                self.my_peek = Some(*boiling_point);
            }
            ServerMessage::Depile {
                reveals,
                exploded,
                boiling_point,
                crossing_index,
            } => {
                let total = reveals.first().map(|e| e.running_volatility).unwrap_or(0);
                self.last_depile = Some(DepileView {
                    reveals: reveals.clone(),
                    exploded: *exploded,
                    boiling_point: *boiling_point,
                    crossing_index: *crossing_index,
                    total_volatility: total,
                });
            }
            ServerMessage::RoundScored {
                color_points,
                outcome,
                awards,
            } => {
                self.last_explosion = None;
                self.last_scoring = Some(ScoringView {
                    color_points: color_points.clone(),
                    outcome: outcome.clone(),
                    awards: awards.clone(),
                });
            }
            ServerMessage::Explosion {
                pot_value,
                deltas,
                shielded,
            } => {
                self.last_scoring = None;
                self.last_explosion = Some(ExplosionView {
                    pot_value: *pot_value,
                    deltas: deltas.clone(),
                    shielded: shielded.clone(),
                });
            }
            ServerMessage::ScoreUpdate { scores } => {
                for s in scores {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == s.player) {
                        p.score = s.score;
                    }
                }
            }
            ServerMessage::GameOver {
                final_scores,
                winners,
            } => {
                self.game_over = true;
                self.winners = winners.clone();
                self.final_scores = final_scores.clone();
                for s in final_scores {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == s.player) {
                        p.score = s.score;
                    }
                }
            }
            ServerMessage::PlayerConnectionChanged { player, connected } => {
                if let Some(p) = self.players.iter_mut().find(|p| p.id == *player) {
                    p.connected = *connected;
                }
            }
            ServerMessage::StateSnapshot {
                group_code,
                your_player_id,
                round_number,
                players,
                scores,
                active_modifiers,
                contributions,
                your_hand,
            } => {
                // Rebuild allowed state on rejoin. The snapshot is scoped to what
                // the player may know — it carries no boiling point and no other
                // players' hands, so there is nothing secret to absorb.
                self.me = Some(*your_player_id);
                self.group_code = Some(group_code.0.clone());
                self.round_number = *round_number;
                if !players.is_empty() {
                    self.players = players.iter().map(PlayerView::from_public).collect();
                }
                for s in scores {
                    if let Some(p) = self.players.iter_mut().find(|p| p.id == s.player) {
                        p.score = s.score;
                    }
                }
                self.active_modifiers = active_modifiers.clone();
                self.apply_contributions(contributions);
                self.hand = your_hand.clone();
                if let Some(me) = self.players.iter().find(|p| p.id == *your_player_id) {
                    self.my_color = Some(me.color);
                }
                self.cauldron_count = contributions
                    .iter()
                    .fold(0u8, |a, c| a.saturating_add(c.count));
            }
            ServerMessage::DeathmatchStarted { participants } => {
                self.deathmatch = true;
                self.dm_participants = participants.clone();
            }
            ServerMessage::GroupSearching { needed } => {
                // 0 signals the search stopped (cancelled or filled).
                self.searching_needed = (*needed > 0).then_some(*needed);
            }
            ServerMessage::StandingsUpdate {
                members,
                guest_games,
                guest_wins,
            } => {
                self.standings = Some(StandingsView {
                    members: members
                        .iter()
                        .map(|m| StandingView {
                            player: m.player,
                            games_played: m.games_played,
                            wins: m.wins,
                        })
                        .collect(),
                    guest_games: *guest_games,
                    guest_wins: *guest_wins,
                });
            }
            // Surfaced by the App as transient toasts/modals, or handled as pure
            // phase/connection transitions — nothing to fold into the view model.
            ServerMessage::SomeonePeeked
            | ServerMessage::Exposed { .. }
            | ServerMessage::DeckReshuffled
            | ServerMessage::EmoteBroadcast { .. }
            | ServerMessage::Error { .. }
            | ServerMessage::LeftGroup
            | ServerMessage::Heartbeat => {}
        }
    }

    /// Reset per-game state for a fresh game with the same group (play-again),
    /// keeping identity, colour, group code, roster, and round count.
    pub(crate) fn reset_for_next_game(&mut self) {
        self.round_number = 0;
        self.wave_number = 0;
        self.active_modifiers.clear();
        self.new_modifier = None;
        self.hand.clear();
        self.new_card_ids.clear();
        self.cauldron_count = 0;
        self.my_peek = None;
        self.last_depile = None;
        self.last_scoring = None;
        self.last_explosion = None;
        self.final_wave = false;
        self.deathmatch = false;
        self.dm_participants.clear();
        self.game_over = false;
        self.winners.clear();
        self.final_scores.clear();
        // The fill search ended when the game started; standings persist across games.
        self.searching_needed = None;
        for p in &mut self.players {
            p.contributed = 0;
        }
    }

    fn merge_players(&mut self, players: &[PlayerPublic]) {
        if self.players.is_empty() {
            self.players = players.iter().map(PlayerView::from_public).collect();
            return;
        }
        for p in players {
            if let Some(existing) = self.players.iter_mut().find(|e| e.id == p.id) {
                existing.connected = p.connected;
            } else {
                self.players.push(PlayerView::from_public(p));
            }
        }
    }

    fn apply_contributions(&mut self, contributions: &[Contribution]) {
        for c in contributions {
            if let Some(p) = self.players.iter_mut().find(|p| p.id == c.player) {
                p.contributed = c.count;
            }
        }
    }

    fn reset_pot(&mut self) {
        self.cauldron_count = 0;
        self.my_peek = None;
        self.last_depile = None;
        self.last_scoring = None;
        self.last_explosion = None;
        for p in &mut self.players {
            p.contributed = 0;
        }
    }
}
