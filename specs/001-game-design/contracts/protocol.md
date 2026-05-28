# Protocol Contract: Boiling Point WebSocket API

**Feature**: `001-game-design` | **Date**: 2026-05-28

**Transport**: MessagePack over WebSocket (JSON fallback for debugging)
**Serialization**: serde with `#[serde(tag = "type")]` for enum variants

---

## Connection

- Endpoint: `ws://{host}/game/{game_id}`
- Auth: Anonymous session token in query param (`?token={session_token}`)
- Heartbeat: server sends `Ping` every 15s; client must respond with `Pong` within 5s
- Reconnection: 60s grace period; on reconnect, server sends full `GameState` snapshot

---

## Client → Server Messages

All messages are player intents. The server validates before applying.

### Lobby

```rust
/// Join an existing game lobby
JoinGame { game_id: Uuid }

/// Signal ready to start (auto-starts when all 4 ready)
Ready

/// Leave the lobby (before game starts)
LeaveLobby
```

### Drafting Phase

```rust
/// Discard cards to meet hand limit (10)
DiscardCards { card_ids: Vec<u32> }
```

### Playing Phase

```rust
/// Commit a card face-down to the cauldron
CommitCard { card_id: u32 }

/// Pass — permanently lock out for this round
Pass
```

### Revealing Phase

```rust
/// Acknowledge card reveal (pacing control)
AckReveal
```

### Effect-Specific

```rust
/// Choose which of your cards to retrieve (when Recall resolves)
RecallChoice { card_id: u32 }
```

### Deathmatch

```rust
/// Commit a card in deathmatch (passing not allowed)
DeathmatchCommit { card_id: u32 }
```

### Connection

```rust
Pong
```

---

## Server → Client Messages

The server sends game events. Visibility rules are enforced — each client only receives information they're allowed to see.

### Lobby Events

```rust
/// A player joined the lobby
PlayerJoined { player_id: Uuid, color: PlayerColor, player_count: u8 }

/// A player left the lobby
PlayerLeft { player_id: Uuid, player_count: u8 }

/// A player signaled ready
PlayerReady { player_id: Uuid }

/// Game is starting (all 4 players ready)
GameStarting { game_id: Uuid, players: Vec<PlayerInfo> }
```

### Round Events

```rust
/// New round begins — includes dealt cards (private to recipient)
RoundStarted {
    round: u8,
    threshold_range: (u8, u8),   // public: the possible range
    your_new_cards: Vec<Card>,   // private: cards dealt to this player
    your_hand: Vec<Card>,        // private: full hand after dealing
}

/// Player must discard to 10 (sent only to that player)
MustDiscard { excess_count: u8 }

/// Discards accepted, round proceeds
DiscardsAccepted
```

### Wave Events

```rust
/// Wave begins — timer starts
WaveStarted {
    wave: u8,
    timer_ms: u32,                        // 30000 or 10000
    active_players: Vec<Uuid>,            // who can still play
}

/// Your card commit was accepted (private)
CommitAccepted { card_id: u32 }

/// Wave resolved — all decisions in
WaveResolved {
    wave: u8,
    actions: Vec<PublicAction>,           // who committed, who passed (no card details)
    cauldron_card_count: u16,             // total cards in pot
    locked_out_players: Vec<Uuid>,        // newly locked out this wave
}

/// Effect notification — sent to relevant players
EffectTriggered {
    effect: SpecialEffect,
    player_id: Uuid,                      // who played it
    // Effect-specific payload:
    peek_result: Option<u8>,              // threshold value (only to Peek player)
    exposed_card: Option<RevealedCard>,   // Expose result (to all)
    someone_peeked: Option<bool>,         // broadcast when Peek used
}

/// Recall resolution — choose a card to retrieve (sent to Recall player only)
RecallPrompt { your_cauldron_cards: Vec<Card> }
```

### Explosion & Reveal Events

```rust
/// Cauldron exploded — triggers reveal
Explosion

/// Card revealed during depile (sent one at a time, last-added first)
CardRevealed {
    position: u8,                         // reveal order (0 = last card added)
    card: RevealedCard,
    cumulative_volatility: u8,            // running total as cards are revealed
    threshold_crossed: bool,              // true on the card that caused explosion
}

/// All cards revealed, threshold shown
RevealComplete {
    threshold: u8,                        // the hidden threshold value
    exploded: bool,
}
```

### Scoring Events

```rust
/// Round scoring breakdown
RoundScored {
    round: u8,
    outcome: ScoringOutcome,              // Domination/Alliance/Commune/Explosion
    color_totals: Vec<(PlayerColor, u16)>,// points per color in pot
    total_pot_points: u16,
    score_changes: Vec<(Uuid, i32)>,      // per-player delta
    scores: Vec<(Uuid, i32)>,             // updated totals
    shield_holders: Vec<Uuid>,            // players who were shielded
}
```

### Game End Events

```rust
/// Game is over (no tie)
GameOver {
    winner_id: Uuid,
    final_scores: Vec<(Uuid, i32)>,
}

/// Deathmatch begins (tied scores)
DeathmatchStarted {
    tied_players: Vec<Uuid>,
    threshold_range: (u8, u8),
}

/// Deathmatch resolved
DeathmatchResolved {
    outcome: DeathmatchOutcome,           // Winner or SharedVictory
    final_scores: Vec<(Uuid, i32)>,
}
```

### Connection Events

```rust
/// Player disconnected (broadcast)
PlayerDisconnected { player_id: Uuid, grace_period_ms: u32 }

/// Player reconnected (broadcast)
PlayerReconnected { player_id: Uuid }

/// Full game state snapshot (sent on reconnect)
GameState {
    game: GameSnapshot,                   // everything the player is allowed to see
}

/// Heartbeat
Ping

/// Server error (invalid action)
Error { code: ErrorCode, message: String }
```

---

## Shared Types

```rust
struct PlayerInfo {
    id: Uuid,
    color: PlayerColor,
    display_name: String,
}

struct Card {
    id: u32,
    color: CardColor,
    volatility: u8,
    points: u8,
    effect: Option<SpecialEffect>,
}

struct RevealedCard {
    id: u32,
    color: CardColor,
    volatility: u8,
    points: u8,
    effect: Option<SpecialEffect>,
    player_id: Uuid,
    effective_volatility: i8,
    effective_points: u16,
    effective_color: CardColor,
}

struct PublicAction {
    player_id: Uuid,
    did_commit: bool,        // true = committed a card, false = passed
    was_timeout: bool,       // true = auto-passed due to timer
}

enum PlayerColor { Red, Blue, Green, Purple }
enum CardColor { Red, Blue, Green, Purple, Wild }
enum SpecialEffect { Peek, Dampen, VolatileSurge, Shield, Expose, Copycat, Recall, DoubleDown }

enum ScoringOutcome {
    Domination { winner: PlayerColor },
    Alliance { winners: Vec<PlayerColor> },
    Commune { winners: Vec<PlayerColor> },
    Explosion,
}

enum DeathmatchOutcome {
    Winner { player_id: Uuid },
    SharedVictory { player_ids: Vec<Uuid> },
}

enum ErrorCode {
    NotYourTurn,
    InvalidCard,
    AlreadyLockedOut,
    HandLimitExceeded,
    GameNotStarted,
    NotInDeathmatch,
    CardNotInHand,
    CardNotInCauldron,
}
```

---

## Information Visibility Matrix

| Data | During Round | After Round (Reveal) |
|---|---|---|
| Player's own hand | Private to player | Private to player |
| Other players' hand sizes | Public | Public |
| Cards in cauldron | Hidden | Fully revealed |
| Who committed/passed | Public (per wave) | Public |
| Card count in cauldron | Public | Public |
| Cumulative volatility | Hidden | Revealed during depile |
| Threshold value | Hidden (unless Peek) | Revealed |
| Player scores | Public | Public |
| "Someone peeked" | Public (when Peek used) | N/A |

---

## Error Handling

All client messages receive either:
- A success event (e.g., `CommitAccepted`, `DiscardsAccepted`)
- An `Error` message with code and human-readable message

Invalid actions produce no state change. The server never trusts client-provided game state.
