## 1. Package Setup

- [x] 1.1 Create the `agent-harness/` Node package (TypeScript, outside the cargo workspace) with `@anthropic-ai/claude-agent-sdk`, `zod`, and `ws`
- [x] 1.2 Add a `ts-rs` export step that generates TS protocol types from the Rust `protocol/` crate, and wire it into the build (D6)
- [x] 1.3 Add MessagePack-in-JS (`@msgpack/msgpack`) codec helpers matching the `protocol/` encoding

## 2. Net Layer & View Model

- [x] 2.1 Implement the WebSocket client and the `JoinRoom`/`protocol_version` handshake, reporting an incompatible-version `Error` instead of crashing through (spec: Claude-Driven Protocol Client)
- [x] 2.2 Build the player-visible view model solely from received `ServerMessage`s — no boiling point, no opponents' hands, no draw deck (spec: Player-Visible View Model)
- [x] 2.3 Add the runtime secret-boundary assertion that fails if a secret field is populated outside a legitimate disclosure (own `PeekResult`, explosion depile) (D6)
- [x] 2.4 Track wave lifecycle (resolve → next wave-open with its timer budget) to drive the deliberation trigger (D4)

## 3. Agent SDK Session & Tools

- [x] 3.1 Establish subscription OAuth auth (`CLAUDE_CODE_OAUTH_TOKEN`), neutralize any `ANTHROPIC_API_KEY`, and log the active auth path at startup (D2)
- [x] 3.2 Stand up an in-process MCP server and register the action tools: `commit_card`, `pass`, `lock_in`, `pick_target`, `send_emote` (spec: Actions and Capabilities as In-Process MCP Tools)
- [x] 3.3 Validate each tool call against the view model and forward a corresponding `ClientMessage`; refuse impossible calls locally with an error result (spec scenario: impossible tool call refused)
- [x] 3.4 Implement the thin per-turn context builder — own hand, public wave state, scores, threshold range — and nothing revocable (spec: Revocable Information Lives Behind Tools)
- [x] 3.5 Implement the `reveal_history` capability tool, scoped to the current shuffle epoch (spec: Difficulty Is the Granted Tool Set; D3)
- [x] 3.6 Define difficulty presets as `allowedTools` subsets — Easy (actions only) and Hard (actions + `reveal_history`) (spec: Difficulty Is the Granted Tool Set)

## 4. Timeliness

- [x] 4.1 Trigger deliberation for wave N+1 at wave N's resolution and lock in early when decided (spec: Timely Commitment Within the Wave; D4)
- [x] 4.2 Implement the fast local fallback action (cheap heuristic or pass, no LLM call) for deadline overruns
- [x] 4.3 Ensure the harness never treats its local clock as authoritative — only the server closes a wave

## 5. Personas

- [x] 5.1 Implement persona archetypes (Gambler, Turtle, Bandwagoner, Trickster) as playstyle-biasing prompt layers, independent of difficulty; no persona = plays straight (spec: Persona Shapes Playstyle Bias)
- [x] 5.2 Map archetypes to preset-emote selection over the `table-talk` palette; never emit free text (spec: Persona-Driven Emote Selection)
- [x] 5.3 Implement the epsilon blunder-injection knob (random legal action with probability epsilon), defaulting to off (spec: Optional Blunder Injection)

## 6. CLI & Run

- [x] 6.1 Build the `bp-bot` CLI: `--room`, `--difficulty`, `--persona`, `--epsilon`, one process per seat (spec: Per-Seat Process; D7)
- [x] 6.2 Run three agents against a room and join as the fourth seat to confirm independent processes and the full loop

## 7. Validation

- [x] 7.1 Test: an agent plays a complete game (vs. other agents) from join through `GameOver` issuing only valid actions
- [x] 7.2 Test: an Easy agent cannot call any card-history tool; a Hard agent can, and gets only current-epoch reveals
- [x] 7.3 Test: across a session, an Easy agent's context never contains past card identities (gating holds)
- [x] 7.4 Test: deliberation overrunning the wave deadline triggers the local fallback and never stalls the wave
- [x] 7.5 Test: the secret-boundary assertion never trips during a full game; persona emotes are palette-only and state-neutral
- [ ] 7.6 Manual: a developer plays a live game with three agents (mixed difficulty/persona) and confirms it feels like a real table

## Status notes (not tasks)

**Unit-verified** (24 passing tests, `npm test`): view model (2.2), secret-boundary assertion
(2.3, 7.5), wave lifecycle (2.4), thin-context gating (3.4, 7.3), difficulty→`allowedTools`
+ epoch-scoped counting (3.6, 7.2), fallback heuristic (4.2), personas + palette-only emotes
(5.1, 5.2, 7.5), blunder injection (5.3). `tsc --noEmit` is clean against the real Agent SDK
0.2.141 and the `protocol/` crate.

**Verified against the live server** (after merging `server-release-1` into this worktree):
- Four `--brain fallback` bots auto-matched and played a complete game to `GameOver`, all
  exiting cleanly — exercising the handshake, msgpack framing, the full view-model
  reconciliation, early lock-in/wave-close, and the continuous secret-boundary assertion
  (2.1, 3.3, 4.1, 4.3, 6.2, 7.1, 7.5 live).
- The Agent SDK path (3.1, 3.2, 3.5) works: a probe authenticated via the Claude Code stored
  login (subscription, no API key), ran Haiku, and called `commit_card` with a valid card.
- A live mixed game (1 Haiku claude bot + 3 fallback) progressed through multiple rounds:
  the agent decided on 30s opening waves and **fell back to the heuristic on 10s sub-waves,
  never stalling the table** (4.2/4.4 live) — confirming 7.4.

**7.6 is the developer's to do** — the harness is ready; a human plays a live table with
three agents and judges the feel. See the README for the commands.

**Known findings / divergences from the original plan:**
- The Rust `uuid` serializes as 16 raw BYTES over MessagePack (a string only in JSON), so the
  codec canonicalizes decoded byte arrays to hex `PlayerId` strings.
- There is **no `PickTarget`** in the real protocol (3.2's `pick_target` tool was dropped);
  targeted effects resolve server-side. Emotes are numeric `EmoteId`s, not strings.
- **Per-decision latency ≈ 23s** (Agent SDK spawns the Claude CLI per `query()`), so on 10s
  sub-waves the agent falls back. A **persistent SDK session** (vs. fresh `query()` per wave)
  is the key follow-up to make the LLM brain timely on fast waves.
- A `--brain fallback` mode was added (zero-cost heuristic seat-filler) — it doubles as the
  protocol integration harness.
- `messages.ts` is now an exact hand-mirror of `protocol/`; regenerating it via a feature-
  gated `ts-rs` derive (1.2's seam) remains the proper long-term step (the crate does not
  derive ts-rs yet, so `npm run gen:protocol` reports the crate's absence of bindings).
