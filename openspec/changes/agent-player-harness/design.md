## Context

The constitution names three testing layers (Principle II). `bot-balance-harness` is Layer 1 — Rust bots playing thousands of games for statistical balance. `terminal-client` is Layer 3 — the rendered client (and its R2 explicitly anticipates *this* harness as "Layer 2"). The gap this change fills is **qualitative**: a single developer cannot feel the hidden-information tension or the political/table-talk layer of Boiling Point alone. This harness supplies intelligent, tunable opponents that fill seats so the game can be playtested by one person, built on `server-release-1`'s public protocol.

## Goals / Non-Goals

**Goals:**
- Let one developer play a complete game against intelligent opponents and feel the blind-volatility and political layers.
- Make difficulty a first-class, legible dial — and make a "casual" opponent's limits *real*, not just prompted.
- Give opponents personality so the emote/table-talk layer can be observed solo.
- Stay a pure protocol client: never compute outcomes, never hold secrets.

**Non-Goals:**
- Statistical balance over thousands of games — that is Layer 1 (`bot-balance-harness`).
- The full analytical capability ladder (`remaining_deck`, `explosion_risk`, `model_opponent`, `simulate`) — deferred past v0.
- Any change to the server's rules or wire protocol.
- A hosted or multi-user deployment — subscription OAuth is licensed for individual use.
- Automatic balance tuning — the harness is for play, not reporting.

## Decisions

### D1. TypeScript/Node package, Claude Agent SDK, outside the cargo workspace

The harness is a Node package at `agent-harness/`, driving Claude through the Agent SDK with **in-process MCP tools**. *Why:* the Agent SDK ships only for TS/Python and unlocks the billing path (D2); in-process tools run in the same process as the socket, so a tool handler reads live game state with no IPC. This deviates from the `CLAUDE.md` workspace sketch (which lists `agent-harness/` as a cargo crate); the deviation is justified in the Constitution Check. *Alternative:* Rust in the workspace (zero type drift, compile-enforced secret boundary, shared `analysis` core with future bots) — rejected because there is no official Anthropic Rust SDK and that path bills API credits, not the subscription.

### D2. Subscription OAuth billing, with the API-key footgun neutralized

Auth uses `CLAUDE_CODE_OAUTH_TOKEN` (via `claude setup-token`) so play bills against the Pro/Max plan; the 2026-06-15 Agent SDK monthly credit covers individual playtesting and removes it from interactive limits. *Risk:* `ANTHROPIC_API_KEY` sits ahead of the OAuth token in the resolution order and silently bills API credits — the harness unsets/overrides it and logs the active auth path at startup.

### D3. Difficulty = `allowedTools`, with revocable info behind tools

A preset is an `allowedTools` subset over a single registered tool set. The per-turn context is **thin** (own hand, public wave state, scores, threshold range); anything we may want to revoke — chiefly depile card identities for counting — is reachable **only** via a capability tool. A long-lived session per game is therefore compatible with gating: a never-registered tool's output never enters context. Card-history scopes to the current shuffle epoch, honoring `server-release-1`'s reshuffle-resets-counting rule. v0 = Easy (actions only) / Hard (actions + `reveal_history`). *Alternative:* fresh session per move for airtight gating — deferred; unnecessary given the thin-per-turn rule, and it would pay startup latency every move.

### D4. Timeliness by deliberating in the inter-wave gap; no server harness mode

In simultaneous waves, everything needed for wave N+1 is final at wave N's resolution, so the agent starts thinking then, holds its action, and locks in at wave-open. A fast local fallback (heuristic or pass, no LLM call) covers overruns. The server stays the sole authority on close; the harness never enforces a local clock. The only optional accommodation is the existing room-config timing knob, used to relax timers for the hardest presets — not a bespoke harness mode.

### D5. Persona as an independent axis; expression via preset emotes

Personas bias playstyle (Gambler/Turtle/Bandwagoner/Trickster) and select from the `table-talk` preset-emote palette (the only comms channel — no free text). Difficulty and persona are orthogonal. A dormant epsilon blunder-injection knob is the *reliable* skill-down lever, since prompted "casual" play shapes style but not blunder rate — and observing whether Claude-as-casual still plays too well is itself a goal.

### D6. Protocol types via `ts-rs`; MessagePack in JS; runtime secret assertion

TS message types are generated from the Rust `protocol/` crate via `ts-rs`, and the `JoinRoom`/`protocol_version` handshake catches residual drift at connect. The wire uses MessagePack in JS by default (JSON fallback is a debugging nicety, not a dependency), since Claude sees only the curated thin-per-turn context, never raw frames. The view model is built solely from received messages; because TS cannot compile-enforce the secret boundary the way the Rust bot-harness does, a runtime assertion guards that no secret field is ever populated outside a legitimate disclosure.

### D7. One process per seat, driven by a CLI

Each agent is a separate process: `bp-bot --room <code> --difficulty <easy|hard> [--persona <archetype>] [--epsilon <p>]`. Independent processes cannot share state, so the agents are honest separate players. A convenience launcher that spawns a preset mix is a later nicety, not v0.

## Constitution Check

- **I — Server-Authoritative:** the harness is a pure client. It computes no scores or outcomes, holds no secrets, and treats the server as sole authority on wave close (D4) and on validity (it forwards intents; the server validates). ✔
- **II — Agent-Driven Development:** this *is* Layer 2. Everything Claude perceives and does is structured tool I/O over the WebSocket protocol — satisfying the principle's "structured JSON over WebSocket." **Documented deviation:** `agent-harness/` is a Node package, not a cargo crate as the `CLAUDE.md` structure sketch shows. Justified by D1 (the Agent SDK is TS/Python-only) and D2 (billing); the principle's normative requirement is the interface, which is met. ✔ (with noted deviation)
- **III — Start Simple, Scale Later:** v0 ships two presets and one capability tool, deferring the full ladder, the fresh-session gating, and the launcher. The simpler-on-one-axis alternative (Rust in-workspace, zero type drift) is explicitly rejected with reasoning (D1) — the added billing/SDK path is justified, not incidental. ✔
- **IV — Playtest-Driven Balance:** the harness is itself a playtesting instrument; difficulty and persona axes let the developer probe game *feel* qualitatively, complementing Layer 1's numbers rather than duplicating them. ✔

## Risks / Trade-offs

- **Protocol drift between TS and Rust** → `ts-rs` codegen in the build; the version handshake fails fast at connect.
- **"Casual" Claude still plays too well** → the dormant epsilon blunder knob (D5) is the reliable skill-down lever; Easy may also use a smaller model.
- **Latency overruns the wave timer** → deliberate at the prior resolution, lock in early, fast local fallback (D4); relax timing via room config only for the hardest presets.
- **Weaker secret boundary than the Rust bot-harness** → mitigated by Constitution I (server never sends secrets) plus a runtime assertion (D6); the boundary is a property of the messages received, not of TS types.
- **Silent API-key billing** → unset/override `ANTHROPIC_API_KEY` and log the active auth path (D2).
- **Subscription rate limits with concurrent sessions** → three agents at human pace is within individual-use limits; hosting is a Non-Goal.

## Migration Plan

Additive. A new Node package outside the cargo workspace, consuming `server-release-1`'s protocol and `table-talk` palette. No schema or protocol changes. Ships after `server-release-1` is runnable enough to host a game and `protocol/` exports `ts-rs` types. Rollback is deletion of the package; nothing else depends on it.

## Post-integration findings (after merging the committed `server-release-1`)

Implementation against the real `protocol/` crate and a running server surfaced these:

- **D9. Uuid is bytes on the wire.** The Rust `uuid` crate serializes as a STRING in JSON but
  as 16 RAW BYTES in MessagePack, so `PlayerId` arrives as a `Uint8Array`. The codec now
  canonicalizes every decoded byte array to a stable hex string, making `PlayerId` usable as
  a Map key / Set member / `includes` value. (No client message carries a `PlayerId`, so no
  encode-side handling is needed.)
- **D10. No `PickTarget`; numeric emotes.** The real protocol has no target-pick message
  (targeted effects resolve server-side), so that tool was dropped. Emotes are numeric
  `EmoteId`s against the configured palette (1 truce … 6 youre_done), not strings.
- **D11. Persistent session + the real latency picture.** The session layer now keeps ONE
  `query()` alive for the whole game in streaming-input mode (`AgentSession`): each wave pushes
  the thin context as a user turn and the agent answers via a move tool, reusing one warm
  subprocess and preserving conversation context (verified: multi-turn on a single session,
  in-runner decisions with zero errors). **However**, a two-turn probe showed per-decision
  latency is **model/rate-limit bound (~14–17s, with an observed `rate_limit_event`), not
  subprocess cold-start** — the warm turn was not faster than the cold one. So the persistent
  session is the correct architecture (no per-wave spawns, shared context) but does NOT by
  itself make a decision fit a 10s sub-wave. Timeliness therefore still rests on
  **deliberate-at-resolve (the head start) + the fast local fallback**, with the relaxed
  room-timer config available for hard presets. Further latency wins (e.g. the SDK's
  pre-warmed `startup()` handle, smaller/no system reasoning, or batching) are open follow-ups.
- **D12. `--brain fallback`.** A zero-cost heuristic brain (no LLM) was added; it fills seats
  for client/UI testing and doubles as the protocol integration harness (four fallback bots
  played a full game to `GameOver`).

## Open Questions

- ~~JSON-fallback wire mode~~ — **resolved:** the server accepts only binary MessagePack
  frames (text is ignored), so the wire is MessagePack-only; JSON stays a debug-decode helper.
- The `ts-rs` derive on `protocol/` is still the proper source of truth for `messages.ts`
  (currently an exact hand-mirror); adding the feature-gated derive remains a follow-up.
- Whether to implement the **persistent SDK session** (D11) and the full capability-tool ladder
  as the next change once the loop and gating are proven (they are).
