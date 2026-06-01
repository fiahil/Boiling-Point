# Span-Schema Contract (v1)

The admin read surface (`admin-ui`) is a **projection of the server's OTEL span
stream**. That makes the span schema a *contract*: the projection, the redacting
exporter, and the privileged reveal all depend on the span names, their nesting,
and which attributes are public vs. secret. This document is the human-readable
companion to the authoritative source of truth in code:
`server/src/observability/span_schema.rs` (`SPAN_SCHEMA_VERSION`).

> **Versioning.** `SPAN_SCHEMA_VERSION` (currently **1**) is stamped on the root
> `game` span as the public `schema.version` attribute. Bump it on any breaking
> change to names/attributes. The projection is **forward/backward tolerant**: it
> ignores span names and attribute keys it does not recognize rather than failing
> (`admin-span-projection`: "Unknown span is ignored").

## Span hierarchy

```
room.lifetime        {room.code}                       — root; one per room, live-registry key
├─ game              {game.id, players.count,
│  │                  schema.version, deck_seed*}       — child of room.lifetime
│  ├─ round          {round.number, boiling_point*,
│  │  │               volatility_total*, modifiers,
│  │  │               round.exploded}                   — child of game
│  │  ├─ hand         {player.id, hand*}                 — child of round (one per seated player)
│  │  ├─ wave         {wave.number, wave.timer_ms,
│  │  │  │            wave.timed_out}                    — child of round
│  │  │  ├─ commit    {player.id, committed_card*}       — child of wave (one per committed card)
│  │  │  └─ resolve   {pot.card_count}                   — child of wave
│  │  └─ score        {round.exploded, pot.value,
│  │                   dominant_color}                   — child of round
│  ├─ reconnect       {player.id}                        — child of game
│  └─ db.write        {db.rows}                          — child of game
├─ ws.message         {ws.message_kind}                  — connection-scoped root
└─ admin.command      {operator, action, target,
                       outcome}                          — command-plane audit root
```

`* = secret attribute` — carried in spans **in-process only** and stripped at the
OTLP export boundary (`server/src/observability/redact.rs`). Open spans are *live
state* and feed the reveal; closed spans fold into aggregates and the replay
buffer.

## Public attributes (export allow-list)

These keys MAY leave the process. Anything not on this list is dropped at export
(fail-closed allow-list, not a deny-list):

`room.code`, `game.id`, `round.number`, `wave.number`, `wave.timer_ms`,
`wave.timed_out`, `players.count`, `round.exploded`, `pot.card_count`,
`pot.value`, `dominant_color`, `modifiers`, `player.id`, `ws.message_kind`,
`db.rows`, `schema.version`, `operator`, `action`, `target`, `outcome`.

## Secret attributes (authoritative set — never exported)

These keys ride spans in-process so the projection can hold them behind admin
auth for the reveal, and are **guaranteed never to be exported**. Adding a key
here without also allow-listing it keeps it non-exporting by default:

| Key | Meaning |
|---|---|
| `boiling_point` | The round's post-modifier boiling point (revealed to players only on explosion). |
| `committed_card` | A committed card's identity before public resolution. |
| `hand` | A player's hand contents. |
| `volatility_total` | Mid-round running cauldron volatility (hidden until depile). |
| `deck_seed` | The game seed (derives the boiling point and the whole deck order). |

## Live-state semantics (for the reveal / open-span registry)

- **`room.lifetime` open** ⇒ the room is live. Its deepest open descendant gives
  the current phase (`game` → `round` → `wave`).
- **`boiling_point`, `modifiers`** are set on the `round` span at round start and
  visible for the whole open round.
- **`volatility_total`** is *recorded onto the open `round` span after each wave*
  and surfaced live through the lifecycle hook's **Update** event, so the reveal
  shows the current running volatility — not just the end-of-round value.
- **`hand`** spans are held open for the duration of a round (one per seated
  player), so the reveal can read each player's hand from an open span.
- **`committed_card`** rides momentary `commit` spans created at wave resolution;
  it is the in-process trace of what was played that wave (publicly revealed at
  the depile anyway).

Any game state **not** represented by a span is, by design, invisible to the
admin surface — that surfaces the instrumentation gap rather than reaching around
the projection (`admin-span-projection`: "Untraced state is invisible").
