# Span-Schema Contract (v2)

The admin command center's read surface is a **projection of the server's OTEL
span stream**. That makes the span schema a *contract*: the projection and the
privileged reveal depend on the span names, their nesting, and their attributes.
This document is the human-readable companion to the authoritative source of truth
in code: `server/src/observability/span_schema.rs` (`SPAN_SCHEMA_VERSION`).

> **Versioning.** `SPAN_SCHEMA_VERSION` (currently **2**) is stamped on the `game`
> span as the public `schema.version` attribute. Bump it on any **breaking** change
> to names/attributes; additive growth (new spans/attributes, including the
> [planned spans](#planned-spans) below) does **not** bump it. The projection is
> **forward/backward tolerant**: it ignores span names and attribute keys it does
> not recognize rather than failing (`admin-span-projection`: "Unknown span is
> ignored").
>
> **v2** is the boom2 combat-core rebase: the `room`→`group` rename
> (`group.lifetime`, `group.code`) and the v2 game subtree — waves carry hidden
> `commit` and optional `spell.cast` leaves, every round ends with the `depile`
> boiling-point reveal, and the v1-only `round.exploded` / `dominant_color`
> attributes retired with the v1 scoring model.

> **No export redaction.** Spans carry sensitive game state and export **as-is** to
> the trusted, operator-only trace backend. The trust boundary that matters is the
> **player wire**, which never carries these attributes — the admin channel is a
> separate transport.

## Span hierarchy

```
group.lifetime       {group.code}                       — root; one per group, live-registry key
lobby.wait           {player.id}                         — root; one per queued player (queue depth)
├─ game              {game.id, players.count,
│  │                  schema.version, deck_seed°}         — child of group.lifetime
│  ├─ brewer.pick    {brewer.offers, brewer.picks}        — child of game (pre-game; public)
│  ├─ round          {round.number, boiling_point°,
│  │  │               volatility_total°, effects.active°,
│  │  │               modifiers, round.boomed,
│  │  │               round.frozen}                      — child of game
│  │  ├─ hand         {player.id, hand.pantry°,
│  │  │                hand.spells°}                      — child of round (one per seated player)
│  │  ├─ wave         {wave.number, wave.timer_ms,
│  │  │  │             wave.timed_out, wave.commits,
│  │  │  │             wave.passes}                       — child of round
│  │  │  ├─ commit     {player.id, committed_card°,
│  │  │  │              vote.color°}                       — child of wave (one per hidden commit)
│  │  │  ├─ spell.cast {player.id, spell.kind,
│  │  │  │              spell.target}                      — child of wave (one per visible cast)
│  │  │  └─ resolve    {pot.card_count, pot.value†,
│  │  │                 detonators†}                       — child of wave († fatal wave only)
│  │  ├─ depile       {boiling_point, reveals,
│  │  │                crossing_index}                    — child of round (every round)
│  │  └─ score        {round.boomed, pot.value,
│  │                   detonators}                        — child of round
│  ├─ reconnect       {player.id}                         — child of game
│  └─ db.write        {db.rows}                           — child of game
├─ ws.message         {ws.message_kind}                   — connection-scoped root
└─ admin.command      {operator, action, target,
                       outcome}                           — command-plane audit root
```

`° = sensitive attribute` — hidden from players in-flight and surfaced only by the
admin reveal (and the operator-only trace backend); never carried on the player
wire. Open spans are *live state* and feed the reveal; closed spans fold into
the `boom-balance-metrics` aggregates and the replay buffer.

### Planned spans

Documented up front so the whole intended tree is visible once; they land
**additively, without a schema bump**, gated on their content changes. Until
then the server does not emit them and the projection's ignore-unknown tolerance
covers any skew (`span_schema::PLANNED_SPANS`). `brewer.pick` landed exactly
this way with `boom2-brewers` — no schema bump — and now lives in the tree above.

| Span | Parent | Lands with | Notes |
|---|---|---|---|
| `draft` | `game` | `boom2-apothecary` | The Apothecary draft; buckets taken are public, realized decks stay sensitive. |

`boom2-compounding` adds no span: compounding triggers ride as additive
attributes on `resolve`/`depile`.

## Attributes

Stable attribute keys live in `span_schema::attr`. Most are plain operational
context (`group.code`, `game.id`, `round.number`, `wave.number`, `wave.timer_ms`,
`wave.timed_out`, `wave.commits`, `wave.passes`, `players.count`, `round.boomed`,
`round.frozen`, `pot.card_count`, `pot.value`, `detonators`, `reveals`,
`crossing_index`, `spell.kind`, `spell.target`, `modifiers`, `player.id`,
`brewer.offers`, `brewer.picks`, `ws.message_kind`, `db.rows`, `schema.version`,
plus the `admin.command` audit fields `operator`/`action`/`target`/`outcome`).

The v2 outcome attributes worth naming:

- **`round.boomed`** — the detonator-only boom (v2's explosion). Replaces v1's
  `round.exploded`; the v1 everyone-loses explosion no longer exists.
- **`round.frozen`** — the round settled with an empty pot (everyone passed); the
  freeze-rate metric's numerator (target: never).
- **`detonators`** — the players who split −P, comma-joined in **fatal-wave sort
  order**. Rides the `score` span every boom and the fatal wave's `resolve` span.
- **`reveals`** — the depile's fuse climb in ascending effective-volatility order:
  `player:Color(vV,pP)@wN` entries joined by commas, `~` marking a colorless play
  and `!` an entry liable for the boom.
- **`crossing_index`** — where the sorted climb crossed the boiling point (boom
  rounds only).

### Sensitive attributes (admin-reveal-only — never on the player wire)

The single authoritative list is `span_schema::SENSITIVE_ATTRS`
(`is_sensitive(key)`); the reveal surfaces exactly this state:

| Key | Meaning |
|---|---|
| `boiling_point` | The round's post-modifier boiling point. Hidden in-flight; revealed to everyone at the depile (the same key rides publicly on the `depile` span, where the reveal has already happened). |
| `committed_card` | A committed card's identity before the depile reveals it. |
| `vote.color` | A commit's Vote colour — the card's colour, or `colorless` — hidden until the depile. |
| `hand.pantry` | A player's pantry (ingredient) hand contents. |
| `hand.spells` | A player's spell (grimoire) hand contents. |
| `volatility_total` | Mid-round running cauldron volatility (hidden until the depile). |
| `effects.active` | Active spell effects: unfired primed Actives (e.g. `Hex(caster→target)`) and a pending `Quench(next-wave)` shield. |
| `deck_seed` | The game seed (derives the boiling points and the whole deck order). |

These ride in spans so the projection can serve them through the reveal (which any
authenticated operator may read over the admin channel) and so the operator trace
backend can record them. The only hard boundary is the **player wire**: a player
connection can never reach the admin channel, so it never sees these.

## Live-state semantics (for the reveal / open-span registry)

- **`group.lifetime` open** ⇒ the group is live. Its deepest open descendant gives
  the current phase (`game` → `round` → `wave`).
- **`boiling_point`, `modifiers`** are set on the `round` span at round start and
  visible for the whole open round.
- **`volatility_total`** and **`effects.active`** are *recorded onto the open
  `round` span after each wave* and surfaced live through the lifecycle hook's
  **Update** event, so the reveal shows the current running volatility and the
  standing spell effects — not just end-of-round values.
- **`hand`** spans are held open for the duration of a round (one per seated
  player) and refreshed at every top-up, commit, cast, and Forage draw, so the
  reveal reads each player's *current* pantry and spell hands.
- **`commit`** spans open the moment a hidden commit is accepted during the wave's
  collection window and close when the wave resolves — the reveal's
  committed-but-unrevealed plays. A revised commit updates its span; a revision to
  pass closes it.
- **`resolve`** closes with the wave; the **fatal** wave's resolve span is held
  through settlement so it additionally carries the pot value P and the detonator
  split.
- **`lobby.wait`** spans are open while a player waits in the auto-match queue; the
  count of open `lobby.wait` spans is the live queue depth.

Any game state **not** represented by a span is, by design, invisible to the
admin surface — that surfaces the instrumentation gap rather than reaching around
the projection (`admin-span-projection`: "Untraced state is invisible").

## Metrics derived from this contract

The balance metrics are **not** computed from raw spans ad hoc: completed spans
map onto `boom-balance-metrics` events
(`balance_metrics::event_from_span`), and every metric — boom rate, freeze rate,
detonator distribution, fold rate, wave depth/duration, round/game duration,
per-spell cast rate, timeout and reconnection rates — is a named definition in
`server/src/observability/balance_metrics.rs`, evaluated identically by the live
pipeline and the benchmarking suite's balance studies. Targets there are seeded
from the decision log (`docs/06_boom2/02_toward-a-v2-core.md`) and are
`[needs playtesting]` hypotheses until studies validate them.
