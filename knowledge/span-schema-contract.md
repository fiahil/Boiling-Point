# Span-Schema Contract (v1)

The admin read surface (`admin-ui`) is a **projection of the server's OTEL span
stream**. That makes the span schema a *contract*: the projection and the
privileged reveal depend on the span names, their nesting, and their attributes.
This document is the human-readable companion to the authoritative source of truth
in code: `server/src/observability/span_schema.rs` (`SPAN_SCHEMA_VERSION`).

> **Versioning.** `SPAN_SCHEMA_VERSION` (currently **1**) is stamped on the root
> `game` span as the public `schema.version` attribute. Bump it on any breaking
> change to names/attributes. The projection is **forward/backward tolerant**: it
> ignores span names and attribute keys it does not recognize rather than failing
> (`admin-span-projection`: "Unknown span is ignored").

> **No export redaction.** Spans carry sensitive game state and export **as-is** to
> the trusted, operator-only trace backend. The trust boundary that matters is the
> **player wire**, which never carries these attributes — the admin channel is a
> separate transport. (Earlier the design redacted at the OTLP boundary; that was
> dropped for a simpler path.)

## Span hierarchy

```
room.lifetime        {room.code}                       — root; one per room, live-registry key
lobby.wait           {player.id}                        — root; one per queued player (queue depth)
├─ game              {game.id, players.count,
│  │                  schema.version, deck_seed°}        — child of room.lifetime
│  ├─ round          {round.number, boiling_point°,
│  │  │               volatility_total°, modifiers,
│  │  │               round.exploded}                   — child of game
│  │  ├─ hand         {player.id, hand°}                  — child of round (one per seated player)
│  │  ├─ wave         {wave.number, wave.timer_ms,
│  │  │  │            wave.timed_out}                    — child of round
│  │  │  ├─ commit    {player.id, committed_card°}        — child of wave (one per committed card)
│  │  │  └─ resolve   {pot.card_count}                   — child of wave
│  │  └─ score        {round.exploded, pot.value,
│  │                   dominant_color}                   — child of round
│  ├─ reconnect       {player.id}                        — child of game
│  └─ db.write        {db.rows}                          — child of game
├─ ws.message         {ws.message_kind}                  — connection-scoped root
└─ admin.command      {operator, action, target,
                       outcome}                          — command-plane audit root
```

`° = sensitive attribute` — hidden from players in-flight and surfaced only by the
admin reveal (and the operator-only trace backend); never carried on the player
wire. Open spans are *live state* and feed the reveal; closed spans fold into
aggregates and the replay buffer.

## Attributes

Stable attribute keys live in `span_schema::attr`. Most are plain operational
context (`room.code`, `game.id`, `round.number`, `wave.number`, `wave.timer_ms`,
`wave.timed_out`, `players.count`, `round.exploded`, `pot.card_count`, `pot.value`,
`dominant_color`, `modifiers`, `player.id`, `ws.message_kind`, `db.rows`,
`schema.version`, plus the `admin.command` audit fields `operator`/`action`/
`target`/`outcome`).

### Sensitive attributes (admin-reveal-only — never on the player wire)

| Key | Meaning |
|---|---|
| `boiling_point` | The round's post-modifier boiling point (revealed to players only on explosion). |
| `committed_card` | A committed card's identity before public resolution. |
| `hand` | A player's hand contents. |
| `volatility_total` | Mid-round running cauldron volatility (hidden until depile). |
| `deck_seed` | The game seed (derives the boiling point and the whole deck order). |

These ride in spans so the projection can serve them through the reveal (which any
authenticated operator may read over the admin channel) and so the operator trace
backend can record them. The only hard boundary is the **player wire**: a player
connection can never reach the admin channel, so it never sees these.

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
- **`lobby.wait`** spans are open while a player waits in the auto-match queue; the
  count of open `lobby.wait` spans is the live queue depth.

Any game state **not** represented by a span is, by design, invisible to the
admin surface — that surfaces the instrumentation gap rather than reaching around
the projection (`admin-span-projection`: "Untraced state is invisible").
