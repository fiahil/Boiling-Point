## Open Questions

- R1: What language and Claude integration should the harness use?
- R2: How is Claude billed, and how is auth made reliable?
- R3: How is difficulty made tunable — and genuinely gateable?
- R4: How does the agent meet the wave timers without a server "harness mode"?
- R5: How do personas behave and express themselves, given emotes-only comms?
- R6: How does a TypeScript harness speak the Rust wire protocol without drift?

## R1: Language and Claude integration

**Decision:** A **TypeScript/Node package** that drives Claude through the **Claude Agent SDK** (`@anthropic-ai/claude-agent-sdk`), exposing tools as **in-process MCP tools** (`tool()` + Zod + `createSdkMcpServer`). The package lives **outside** the cargo workspace.

**Rationale:** The Agent SDK ships only for TypeScript and Python, and it is the thing that unlocks the billing path in R2. In-process MCP tools run in the *same* Node process as the WebSocket connection, so a tool handler reads the live, accumulated game state directly — exactly what the difficulty dial (R3) needs. TS over Python is chosen because protocol types can be generated from the Rust `protocol/` crate via `ts-rs` (R6), and because the agent loop is the Agent SDK's job, not ours, so the SDK's polish matters more than language ecosystem fit.

**Alternatives Considered:**
- **Rust in the workspace** — shares `protocol/` types with zero drift and a compile-enforced secret boundary (as `bot-balance-harness` D2 enjoys), and would let the Claude tools share an `analysis` core with future sharp bots. Rejected: there is **no official Anthropic Rust SDK** (community crates only), and the Rust path bills pay-as-you-go API credits, not the subscription. The type-sharing win does not outweigh billing for a tool run constantly during solo dev.
- **Python + Agent SDK** — same billing win. Rejected for the weaker codegen story (`ts-rs` gives clean TS protocol types straight from `protocol/`).

**Key Details:** out-of-workspace Node package at `agent-harness/` (keeps the established directory name); the protocol-drift seam is accepted and guarded by the version handshake (R6).

## R2: Billing and auth

**Decision:** Authenticate via **subscription OAuth** — `claude setup-token` → `CLAUDE_CODE_OAUTH_TOKEN` — so moves bill against the Pro/Max plan, not pay-as-you-go API credits. The harness MUST explicitly neutralize any `ANTHROPIC_API_KEY` in the environment and log which auth path is active.

**Rationale:** This is a single-developer playtesting tool run many times; subscription billing is the right fit, and the **Agent SDK monthly credit (from 2026-06-15)** is *explicitly* sized for "individual experimentation and automation" and stops Agent-SDK usage from counting against interactive Claude Code limits.

**Alternatives Considered:** API-key (Console) billing — predictable for sustained/team load but pay-as-you-go; rejected for a personal tool.

**Key Details:** the credential resolution order puts `ANTHROPIC_API_KEY` *ahead* of the OAuth token, so a stray exported key silently bills API credits while appearing to use the subscription — the harness must unset/override it and surface the active path at startup. Subscription OAuth is licensed for **individual use**; three concurrent agent sessions for personal playtesting is fine, a hosted/multi-user deployment is not (Non-Goal).

## R3: Tunable, gateable difficulty

**Decision:** **A difficulty preset IS an `allowedTools` subset.** All tools are registered on the in-process MCP server; the preset controls which Claude may actually call. Crucially, **information we want to be able to revoke lives behind a tool, never in the per-turn context.** The per-turn message to Claude carries only thin public state (its own hand, public wave state, scores, the threshold range); card identities from the depile are reachable **only** through a capability tool, so an agent without that tool never receives them — even within a long-lived session. v0 presets: **Easy** (action tools only) and **Hard** (actions + reveal-history).

**Rationale:** This makes difficulty a config array rather than a different prompt or model, and makes "no card counting" *real* rather than aspirational. The depile is public in-game, so withholding the history tool models a player who "didn't watch the reveal" — a believable casual opponent. Card-counting honors `server-release-1`'s reshuffle rule: counting resets when the draw deck reshuffles from discard, so the history tool MUST scope to the current shuffle epoch.

**Alternatives Considered:**
- **Different prompt/model per difficulty** — fuzzier; competence leaks. Kept as a *complement* (Easy may also use a smaller model), not the mechanism.
- **Fresh session per decision** to guarantee gating — clean, but pays session-startup latency every move. Rejected for v0: a long-lived session plus the thin-per-turn rule already gates correctly, because a never-registered tool's output never enters context.

**Key Details:** v0 tool set = `commit_card`, `pass`, `lock_in`, `pick_target` (for effects like Recall), `send_emote`, and the single capability tool `reveal_history`. The full capability ladder (`remaining_deck`, `explosion_risk`, `model_opponent`, `simulate`) is **deferred** to a follow-up once the loop is proven.

## R4: Meeting the wave timers without a server harness mode

**Decision:** No server "harness mode." The agent **starts deliberating for wave N+1 the moment wave N resolves** (in simultaneous waves, all information needed is final at resolution — nothing new appears when the next timer merely opens), holds the chosen action, and **locks it in at wave-open**. If no decision is ready near the deadline, the harness commits a **fast local fallback** (a cheap heuristic or pass). The server remains the sole authority on wave close.

**Rationale:** The simultaneous-wave structure hands us the head start for free, so the nominal 10s/30s timers become deadlines for a computation that began at the previous resolution. Locking in early (per `round-engine` / `terminal-client` R7, commits are changeable until close and unanimous lock-in closes a wave early) keeps the table flowing instead of waiting on a slow agent.

**Alternatives Considered:**
- **Server harness mode** (disable timers for bots) — rejected by the developer; keeps the server honest to the real game.
- **Always-long timers** — changes the game's feel; instead use the existing room-config knob to *optionally* relax timing only for the hardest presets.

**Key Details:** Easy is fast by construction (no `simulate`-class tools); only high presets risk overrunning, and they have the longest head start. The fallback heuristic is a tiny local function (no LLM call), the TS analogue of a baseline bot strategy.

## R5: Persona behavior and expression

**Decision:** A persona is an **optional archetype** layered on a difficulty preset (the two are independent axes). It biases playstyle — Gambler → high-volatility commits, Turtle → cautious / pass-leaning, Bandwagoner → the leading color, Trickster → decoy color early / own color late — and it **expresses through the preset-emote palette only** (`table-talk` has no free text). A dormant **epsilon blunder-injection** knob can override a chosen action with a random legal one at a set probability, giving a reliable difficulty lever independent of model competence.

**Rationale:** The developer wants to playtest the *political* layer solo; named archetypes that bias play and emote in character make the table feel alive and let the archetypes from the design docs (Aggressor / Vulture / Misdirection / Saboteur) be instantiated and observed. Persona-via-prompt reliably shapes **style** but not **blunder rate**, so the epsilon knob exists as the dependable skill-down lever if a "casual" Claude still plays too well — which is itself the thing the developer is curious to observe.

**Alternatives Considered:**
- **Free-text trash talk** — does not exist; `table-talk` is preset emotes only. Personas map archetype → emote choices instead.
- **Skill purely from a weaker model** — coarse and not expressive; persona + optional epsilon is finer-grained.

**Key Details:** the concrete archetype→emote mapping depends on the finalized emote palette in `server-release-1`'s content config; epsilon defaults to 0 (off) in v0.

## R6: Speaking the Rust wire protocol from TypeScript

**Decision:** Connect as an ordinary WebSocket client and build the player-visible view model **solely from received `ServerMessage`s** (mirroring `bot-balance-harness` D2's secret-by-construction intent). Generate the TS message types from the Rust `protocol/` crate via **`ts-rs`** as a build step. Use **MessagePack in JS** (`@msgpack/msgpack`) on the wire by default; treat the server's JSON-fallback mode as a debugging nicety, not a dependency.

**Rationale:** `ts-rs` keeps the TS DTOs in lockstep with `protocol/`, and the `JoinRoom`/`protocol_version` handshake catches any residual drift at connect time. Claude never sees raw frames anyway — it sees the curated thin-per-turn context (R3) — so the wire format is purely an internal concern of the net layer, and adopting MessagePack avoids creating a cross-change blocker for a JSON-selection mechanism the server spec does not yet define.

**Alternatives Considered:**
- **Hand-written TS types** — drift-prone; rejected in favor of `ts-rs` codegen.
- **Require JSON-fallback selection on the handshake** — would make this change depend on a new `server-release-1` ask; deferred. If debugging pain justifies it later, it is a small additive server ask, not a blocker.

**Key Details:** unlike the Rust bot-harness, the TS secret boundary is **not compile-enforced**; it is upheld by (a) the server never sending secrets (Constitution I) and (b) a runtime assertion that no secret field is ever populated on the view model. Card-history tooling scopes to the current shuffle epoch (R3).

## Summary

- **R1** TypeScript/Node package driving Claude via the Agent SDK with in-process MCP tools; outside the cargo workspace.
- **R2** Subscription OAuth (`CLAUDE_CODE_OAUTH_TOKEN`); neutralize `ANTHROPIC_API_KEY`; the 2026-06-15 Agent SDK credit fits individual playtesting.
- **R3** Difficulty = `allowedTools` subset; revocable info lives behind tools; thin per-turn context; long-lived session; v0 = Easy / Hard with one capability tool; counting honors reshuffle epochs.
- **R4** Deliberate at the previous wave's resolution, lock in early, fast local fallback; no server harness mode; optional relaxed room timing for hard presets only.
- **R5** Persona = optional archetype biasing playstyle + preset-emote expression; dormant epsilon blunder knob as the reliable skill-down lever.
- **R6** `ts-rs`-generated protocol types; view model from received messages only; MessagePack in JS by default, JSON fallback optional; runtime secret-boundary assertion.
