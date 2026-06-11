# boom2-ai-client — Design

## Context

Constitution v2.0.0 archived the v1 harnesses (`archive/bot-harness/`,
`archive/agent-harness/`, Rust + TypeScript respectively — each carried its own
hand-built client layer over the same wire protocol) and mandated that at-scale
automated playtesting be **reinstated before boom2 ships** (Principle IV). The v2 game
also wants AI seats as a **product feature**: a 4-player political game needs full
tables, and v2 pacing (~25s/15s waves, ~90s draft) fits LLM latency where v1's 10s
waves did not.

Decisions already made with the user in explore mode:

- One Rust AI client serving **both** playtest and product.
- **Two brains as two distinct implementations with their own settings**, sharing one
  client core behind a `Brain` trait.
- **Strict firewall**: client and server share *only* the `protocol/` crate; data
  structures stay distinct at all times; even in-process transport exchanges
  wire-protocol frames, never domain objects.
- The brain interface is `f(view, decision_frame) → action`, with the server
  enumerating the pending decision + legal actions.

## Goals / Non-Goals

**Goals:**

- A `clients/ai/` cargo workspace member that fulfils the Principle IV reinstatement
  (seeded batch runs, thousands of games, persona × Brewer × deck-archetype matrix).
- The same binary fills seats in real rooms (WebSocket) without ever stalling a wave.
- Brains are swappable and independently configurable; adding a third brain (e.g. a
  search brain) later touches no client-core code.
- The decision-frame contract benefits all clients (TUI revival, web) — legality is
  computed once, on the server.

**Non-Goals:**

- A search/RL brain (the `Brain` trait is the seam; not built now).
- Reviving the TUI or the v1 harness code as-is (the archive is reference material).
- Matchmaking/lobby UX for "add a bot" buttons (server seam only; product UX is a
  follow-up with `boom2-identity`/lobby work).
- v1 game support — this client speaks v2 (`boom2-combat-core` shapes) only.
- Localized bot table-talk (ships with `boom2-localization` discipline later; English
  identifiers/emotes first).

## Decisions

### D1 — One Rust client core, brains behind a trait

**Chosen:** a single `clients/ai/` crate: connection/session lifecycle, secret-free
view model, decision loop, and a `Brain` trait with two implementations (bot, agent).

**Rejected — revive both v1 stacks:** every v2 protocol change would land twice (Rust
and a hand-mirrored TS layer); the v2 view-model rewrite is exactly the kind of large
duplicated work where drift starts. **Rejected — one TS client:** loses in-process
determinism and the batch throughput Principle IV needs.

### D2 — Firewall mechanics: frames on a channel, distinct types everywhere

The client depends on `protocol/` and **never** on `server/` internals. Its view model
is rebuilt from received messages and structurally **cannot** hold a secret (no
boiling-point field, no opponents' hands, no own-deck realization — same discipline as
the v1 `model.rs`). The in-process transport for batch runs is a pair of channels
carrying **encoded wire frames (MessagePack bytes)** — the exact bytes the WebSocket
would carry — so the codec is exercised on every batch game and transport parity is
structural, not aspirational.

**Rejected — channels of protocol enum values (skip serialization):** marginally
faster, but loses codec coverage and makes it tempting to "just pass" richer types;
serialization cost is noise next to game logic. **Rejected — linking server domain
types for speed:** violates the firewall outright; one accidental field is a silent
secret leak that invalidates every balance stat.

The batch runner binary *does* link the server crate — to boot in-process games — but
the boundary between any seat and the server is the frame channel. The server side
needs a small seam: boot a room headless and expose per-seat frame channels (analogous
to the v1 in-process transport).

### D3 — Decision frames: the server enumerates legality

The server sends, per player owing an action, a **decision frame**: the pending
decision kind (Brewer pick, draft, wave commit, …) and the **complete legal action
set** (playable ingredients, pass availability, castable spells with their legal
targets). Action spaces are small (≤4 hand cards, 15 spell kinds, ≤3 player targets, 4
colors), so enumeration is cheap and bounded. This is Principle I inverted: the server
already knows what's legal because it validates everything.

Brains become pure choosers over enumerated options: heuristic bots cannot desync from
v2 rules, the agent brain's tool schema is derived from the frame, and rendering
clients get affordances (what's clickable) for free.

**Rejected — clients re-derive legality from rules (the v1 approach):** duplicated
rules logic in every client, desync risk on every balance patch, and the agent brain
would burn tokens reasoning about legality instead of strategy.

**Coordination:** the frame shapes belong to the `protocol/` crate alongside the
`boom2-combat-core` message work. Combat-core ships first; this change owns the
`boom-decision-frame` spec and lands the enumeration either inside combat-core's
protocol tasks (preferred if timing aligns) or as an immediate additive follow-up.

### D4 — Agent brain talks to Claude directly from Rust

**Chosen:** the agent brain calls the Anthropic Messages API from Rust. One decision =
one tool-forced call: the decision frame becomes the tool schema, the persona/difficulty
prompt and a running game transcript (events the player legitimately saw) provide
context. API-key billing with hard spend caps and per-game budgets.

**Rejected — revive the TS sidecar (Agent SDK over stdio):** the v1 reason for
TypeScript was the Agent SDK + subscription billing for one developer's playtests. A
*product* seat-filler can't run on a personal subscription anyway — it bills API either
way — and a Node sidecar adds an IPC seam plus a second runtime to ops for no
capability we still need (the SDK's persistent session is replaced by the in-context
transcript). The `Brain` trait keeps the sidecar door open if that calculus changes.

### D5 — Draft ownership is a host policy, not a brain property

The host wraps the brain with a per-decision-kind policy:
`Scripted(value) | Delegated`. Harness mode scripts the Apothecary draft (deck-archetype
is an **experimental variable** on the matrix axis) and typically the Brewer pick too;
seat-filler mode delegates both to the brain (the synergy hunt is the skill
expression). This keeps "what is controlled" out of brain implementations entirely.

### D6 — Timeliness: every decision races a budget, bot brain is the floor

Each decision runs against a configured latency budget (derived from the wave timer
minus a safety margin). The bot brain answers in microseconds and is always the
fallback: if the agent brain misses its budget, the fallback answer is committed and
the agent's late answer is discarded. A seat therefore **never** stalls a wave, in
either mode. Fallback rate is reported per game (a high rate means the budget or model
choice is wrong, and in harness terms the seat degenerates to a bot seat).

### D7 — Determinism: seeded RNG tree, agent brain explicitly outside it

Harness mode revives the v1 guarantee: same seed → identical outcomes, via a
deterministic RNG tree (root seed → per-game → per-seat). Bot-brain settings (epsilon
blunders, tie-breaks) draw only from their seat RNG. The agent brain is inherently
non-deterministic and is **excluded from reproducibility claims**; batch runs default
to bot brains, and mixed runs are marked non-reproducible in the report.

### D8 — Seat-filler entry: parity first, server-summoned later

Start with v1 parity: the CLI joins by invite code or enqueues into matchmaking, N
seats per process. A server-initiated "fill this room" flow (lobby button, autofill on
timeout) needs a server/ops seam and product UX — designed as a follow-up; this change
only keeps the client side shaped so summoning is an entry-mode addition, not a rework.

### D9 — Reports: diffable artifacts, matrix-aware

Revive the v1 report shape (human markdown + machine JSON, diffable across config
versions) and extend the axes: explosion rate, detonator distribution, per-Brewer and
per-archetype win rates, spell fire rates (Peek economy), fold-to-safety rates, freeze
detection, fallback rates. Full factorial persona × Brewer(12) × archetype is too large
to brute-force per run — the runner takes an explicit matrix sample spec (which cells,
how many games), and CI runs a pinned small sample.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | The AI client is an untrusted client like any other: it sends intents from server-enumerated legal sets and the server still validates every action. Decision frames don't move authority — they are the validator's verdict published *before* the action instead of only after. The firewall (D2) makes secret-leak structurally impossible on the client side. |
| **II — Agent-driven** | All Rust + config files, fully agent-writable. Restores the headless balance layer and the Claude-as-player layer that v2.0.0 archived, rebuilt for v2 against the in-process and real-wire transports. |
| **III — Start simple** | One crate, one `Brain` trait, two brains; direct API call over a sidecar (D4); CLI entry over server-summoning (D8); heuristics over search (seam only). **Rejected simpler alternative:** revive the two archived stacks as-is — rejected because the v2 view-model rewrite would land twice in two languages and drift, and neither stack covers the product seat-filler. |
| **IV — Playtest-driven** | This change *is* the Principle IV reinstatement mandate: seeded thousands-of-games runs, the persona × Brewer × deck-archetype matrix, diffable reports, and degenerate-strategy detection, required before boom2 ships. |

## Risks / Trade-offs

- **[Bot competence vs balance validity]** Naive heuristics in a deep game produce
  meaningless balance stats (a bot casting Redirect randomly says nothing about
  Redirect). → Per-archetype scripted heuristics tuned per spell/Brewer hook; matrix
  includes a `random` baseline so "indistinguishable from random" is itself a
  detectable red flag; the `Brain` seam admits a search brain if heuristics prove too
  weak. Treat "bots can't express the strategy" as a blocking finding before human
  playtests.
- **[Decision-frame schedule coupling]** This client is blocked until frames exist in
  the protocol. → Frames are additive; spec them here, land them with combat-core's
  protocol tasks; client core + bot brain develop against an in-process server stub
  meanwhile.
- **[Agent cost blowup]** Thousands-of-games batch with an agent brain = real money. →
  Batch defaults to bot brains; hard per-process spend cap and per-game budget; agent
  in harness mode requires an explicit flag.
- **[Server in-process seam]** Headless room boot with frame channels requires server
  cooperation and could drift from the real WS path. → The seam carries encoded frames
  through the same codec; CI runs the same scenario over both transports (v1's
  validation pattern).
- **[LLM latency on 15s sub-waves]** Budgets of ~10s may still be tight for big
  decision frames. → Fallback guarantees liveness; prompt keeps the frame compact;
  model choice is a setting (fast model for waves, bigger for draft).
- **[One client, two masters]** Harness needs (determinism, throughput) and product
  needs (liveness, personality) can pull the core apart. → The core stays a thin
  decision loop; mode-specific behavior lives in the two hosts (runner vs filler), and
  D5's policy layer is the only sanctioned coupling point.

## Migration Plan

New component; no data migration. Lands incrementally behind nothing — the crate is
inert until invoked. Rollback = remove the workspace member. The only shared-surface
change is the additive decision-frame protocol messages (versioned with the v2 protocol
that `boom2-combat-core` already breaks).

## Open Questions

- Exact decision-frame schema for targeted spells (flat enumerated actions vs
  action-template + target list) — resolve in the `boom-decision-frame` spec with
  combat-core's protocol review.
- Spell-target legality nuances (can Redirect target a folded player? Hex the caster?)
  — owned by combat-core rules; frames just carry the answer.
- Product summoning UX (lobby "add AI seat", autofill on queue timeout) and whether AI
  seats are flagged to players — follow-up change with the lobby/identity work.
- Persona set for v2 (v1's gambler/turtle/bandwagoner/trickster may not map onto
  Brewer identities) — needs design alongside `boom2-brewers` flavor.
- Whether the agent transcript compacts per round (token growth over a 15–20 min game)
  — measure first.
