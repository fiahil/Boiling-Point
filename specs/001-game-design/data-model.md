# Data Model: Boiling Point — Complete Game Design

**Feature**: `001-game-design` | **Date**: 2026-05-28

---

## Entity Overview

```
Game 1──* Round 1──* Wave 1──* PlayerAction
  │                    │
  │                    └──1 Cauldron 1──* CauldronCard
  │
  ├──1 Deck (CardDefinition[])
  │
  └──* PlayerSession ──* HandCard
```

---

## Entities

### Game

A single game session from lobby to completion.

| Field | Type | Description |
|---|---|---|
| `id` | UUID | Unique game identifier |
| `status` | GameStatus | `Lobby`, `InProgress`, `Finished` |
| `round_count` | u8 | Fixed at 5 (or 7 for extended mode) |
| `current_round` | u8 | 1-indexed, 0 = not started |
| `players` | PlayerSession[4] | Exactly 4 players |
| `deck` | Deck | Shared draw/discard pile |
| `created_at` | DateTime | Lobby creation time |
| `finished_at` | DateTime? | Game completion time |
| `winner_ids` | UUID[]? | Winner(s) — multiple for shared victory |

**State transitions**:
```
Lobby ──[4 players joined]──> InProgress ──[round 5 scored]──> Finished
                                  │                                │
                                  └──[deathmatch if tied]──────────┘
```

### PlayerSession

A player's state within a game.

| Field | Type | Description |
|---|---|---|
| `id` | UUID | Unique player identifier |
| `game_id` | UUID | FK → Game |
| `color` | PlayerColor | `Red`, `Blue`, `Green`, `Purple` |
| `score` | i32 | Current score (can go negative) |
| `hand` | HandCard[] | Cards in hand (max 10) |
| `is_connected` | bool | WebSocket connection status |
| `disconnected_at` | DateTime? | For 60s reconnection grace period |

### Round

One of 5 (or 7) rounds in a game.

| Field | Type | Description |
|---|---|---|
| `number` | u8 | 1-indexed round number |
| `game_id` | UUID | FK → Game |
| `threshold` | u8 | Hidden volatility threshold (rolled from range) |
| `threshold_range` | (u8, u8) | Min–max for this round (from schedule) |
| `status` | RoundStatus | `Drafting`, `Playing`, `Revealing`, `Scoring`, `Done` |
| `cauldron` | Cauldron | The shared pot |
| `waves` | Wave[] | Waves played this round |
| `active_players` | UUID[] | Players not yet locked out |
| `peek_holders` | UUID[] | Players who have seen the threshold |

**Threshold schedule** (from research):

| Round | Range |
|---|---|
| 1 | 10–16 |
| 2 | 9–15 |
| 3 | 8–13 |
| 4 | 7–12 |
| 5 | 6–10 |

### Wave

A simultaneous decision window within a round.

| Field | Type | Description |
|---|---|---|
| `number` | u8 | 1-indexed wave number within the round |
| `round_number` | u8 | FK → Round |
| `timer_duration_ms` | u32 | 30000 for wave 1, 10000 for subsequent |
| `actions` | PlayerAction[] | One per active player |
| `started_at` | DateTime | Wave start time |
| `resolved_at` | DateTime? | When all actions received or timer expired |

### PlayerAction

A single player's decision in a wave.

| Field | Type | Description |
|---|---|---|
| `player_id` | UUID | FK → PlayerSession |
| `wave_number` | u8 | FK → Wave |
| `action_type` | ActionType | `Commit` or `Pass` |
| `card_id` | u32? | Card played (null if Pass) |
| `submitted_at` | DateTime | When the action was submitted |
| `was_timeout` | bool | True if auto-passed due to timer expiry |

### Cauldron

The shared pot for a round, holding committed cards face-down.

| Field | Type | Description |
|---|---|---|
| `round_number` | u8 | FK → Round |
| `cards` | CauldronCard[] | Cards committed (in wave order) |
| `total_volatility` | u8 | Running sum of all card volatilities (including effect modifiers) |
| `total_points` | u16 | Running sum of all card points (including effect modifiers) |
| `has_exploded` | bool | Whether volatility exceeded threshold |

### CauldronCard

A card in the cauldron, tracking who played it and when.

| Field | Type | Description |
|---|---|---|
| `card_id` | u32 | FK → card instance |
| `player_id` | UUID | Who played this card |
| `wave_number` | u8 | Which wave it was committed in |
| `definition` | CardDefinition | The card's template |
| `effective_volatility` | i8 | After effect modifiers (e.g., Dampen: -1, Surge: +5) |
| `effective_points` | u16 | After effect modifiers (e.g., Double Down) |
| `effective_color` | CardColor? | After Copycat resolution |

### CardDefinition

A template defining a type of card in the deck.

| Field | Type | Description |
|---|---|---|
| `id` | u32 | Unique card type ID |
| `color` | CardColor | `Red`, `Blue`, `Green`, `Purple`, `Wild` |
| `base_volatility` | u8 | 1, 2, or 3 |
| `base_points` | u8 | 0, 1, 2, or 3 |
| `effect` | SpecialEffect? | Optional special effect |

### Deck

The shared draw pile and discard pile.

| Field | Type | Description |
|---|---|---|
| `draw_pile` | u32[] | Card IDs remaining to draw (shuffled) |
| `discard_pile` | u32[] | Card IDs from completed rounds |

When `draw_pile` is empty and cards need to be drawn, `discard_pile` is shuffled into `draw_pile`.

### HandCard

A card in a player's hand.

| Field | Type | Description |
|---|---|---|
| `card_id` | u32 | Card instance ID |
| `definition` | CardDefinition | The card's template |
| `drawn_round` | u8 | Which round this card was drawn in |

---

## Enums

### GameStatus
```
Lobby | InProgress | Finished
```

### RoundStatus
```
Drafting → Playing → Revealing → Scoring → Done
```

State machine:
```
Drafting ──[cards dealt, hands trimmed]──> Playing
Playing ──[all locked out OR explosion OR last-player done]──> Revealing
Revealing ──[all cards shown]──> Scoring
Scoring ──[points awarded/deducted]──> Done
```

### PlayerColor
```
Red | Blue | Green | Purple
```

### CardColor
```
Red | Blue | Green | Purple | Wild
```

### ActionType
```
Commit | Pass
```

### SpecialEffect
```
Peek | Dampen | VolatileSurge | Shield | Expose | Copycat | Recall | DoubleDown
```

### ScoringOutcome
```
Domination { winner: PlayerColor, points: i32 }
Alliance { winners: PlayerColor[], points_each: i32 }
Commune { winners: PlayerColor[], points_each: i32 }
Explosion { penalty: i32 }
```

---

## Validation Rules

### Hand Management
- Max hand size: 10 cards
- Cards dealt per round: 5 per player
- If hand would exceed 10 after dealing, player must discard down to 10 before the round's Playing phase begins
- Unplayed cards persist between rounds

### Wave Rules
- Each active player submits exactly one `PlayerAction` per wave
- A `Pass` action permanently removes the player from `active_players`
- Timer expiry with no action = auto-Pass (lock out)
- When only 1 active player remains, they get one final wave, then round ends

### Scoring Rules
- Color dominance = highest total effective_points among player colors in cauldron
- Wild cards contribute to total pot value but not to any color's dominance
- Floor division for alliance/commune splits (remainder lost)
- Brew: dominant player(s) receive total pot points
- Explosion: ALL players lose total pot points (including non-participants)
- Shield holders: immune to explosion penalty AND excluded from brew scoring

### Effect Resolution (wave resolve order)
1. All cards from the wave are added to cauldron
2. **Dampen**: reduce `total_volatility` by 2 (net card contribution: -1)
3. **Volatile Surge**: add +2 extra to `total_volatility` (net card contribution: +5)
4. **Expose**: reveal one random face-down card to all players
5. **Peek**: privately reveal threshold to the Peek player; broadcast "someone peeked"
6. **Copycat**: resolve color to current highest-points color in pot from prior waves
7. **Double Down**: double points of all same-color cards from prior waves
8. **Recall**: player retrieves one of their own prior cards; Recall card stays
9. Check if `total_volatility > threshold` → explosion

### Deathmatch Rules
- Only tied players participate
- New threshold generated from Round 5 range (6–10)
- No passing — each player must commit 1 card per wave
- Brew = standard color dominance scoring
- Explosion or cards exhausted = shared victory

---

## State Transition: Full Round Lifecycle

```
┌─────────────────────────────────────────────────────┐
│                    DRAFTING                          │
│  1. Generate threshold from round's range           │
│  2. Deal 5 cards to each player                     │
│  3. Players discard to 10 if over hand limit        │
│  4. All players marked active                       │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│                    PLAYING                           │
│  ┌───────────────────────────────────────────────┐  │
│  │              WAVE N                           │  │
│  │  1. Start timer (30s wave 1, 10s otherwise)   │  │
│  │  2. Each active player: Commit or Pass        │  │
│  │  3. Timer expires → auto-pass uncommitted     │  │
│  │  4. Add committed cards to cauldron           │  │
│  │  5. Resolve effects                           │  │
│  │  6. Lock out players who passed               │  │
│  │  7. Broadcast: who played, who passed, count  │  │
│  │  8. Check explosion (volatility > threshold)  │  │
│  └──────────┬───────────────────┬────────────────┘  │
│             │                   │                    │
│     [no explosion,         [explosion OR             │
│      active > 1]       all locked out OR             │
│             │          last-player done]              │
│             ▼                   │                    │
│        WAVE N+1                 │                    │
└─────────────────────────────────┼────────────────────┘
                                  ▼
┌─────────────────────────────────────────────────────┐
│                   REVEALING                          │
│  1. Cards revealed one-by-one, last-added first     │
│  2. Each reveal: color, points, volatility, effect, │
│     player, effective values after modifiers         │
│  3. Threshold value revealed to all                  │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│                    SCORING                           │
│  If explosion:                                       │
│    penalty = total_pot_points                        │
│    all players: score -= penalty                     │
│    Shield holders: exempt                            │
│  If brew:                                            │
│    determine color dominance                         │
│    dominant player(s): score += total_pot_points     │
│    floor-divide for ties                             │
│    Shield holders: excluded (score += 0)             │
│  Broadcast full scoring breakdown                    │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│                      DONE                            │
│  1. Move cauldron cards to discard pile              │
│  2. If round < 5: advance to next round             │
│  3. If round = 5: check for ties → Deathmatch or    │
│     declare winner                                   │
└─────────────────────────────────────────────────────┘
```
