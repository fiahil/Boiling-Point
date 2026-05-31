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
- [ ] 6.2 Run three agents against a room and join as the fourth seat to confirm independent processes and the full loop

## 7. Validation

- [ ] 7.1 Test: an agent plays a complete game (vs. other agents) from join through `GameOver` issuing only valid actions
- [x] 7.2 Test: an Easy agent cannot call any card-history tool; a Hard agent can, and gets only current-epoch reveals
- [x] 7.3 Test: across a session, an Easy agent's context never contains past card identities (gating holds)
- [ ] 7.4 Test: deliberation overrunning the wave deadline triggers the local fallback and never stalls the wave
- [x] 7.5 Test: the secret-boundary assertion never trips during a full game; persona emotes are palette-only and state-neutral
- [ ] 7.6 Manual: a developer plays a live game with three agents (mixed difficulty/persona) and confirms it feels like a real table

## Status notes (not tasks)

**Implemented and unit-verified now** (24 passing tests, run with
`node --experimental-strip-types --test test/*.test.ts`, no deps or server needed):
the view model (2.2), secret-boundary assertion (2.3, and the unit portion of 7.5), wave
lifecycle (2.4), thin-context gating (3.4, 7.3), difficulty→`allowedTools` + epoch-scoped
counting (3.6, 7.2), fallback heuristic (4.2), personas + palette-only emotes (5.1, 5.2,
emote portion of 7.5), and blunder injection (5.3).

**Implemented, verification deferred until `server-release-1` is committed** (per the apply
instruction — server is in parallel development): the WS connection + handshake (2.1), auth
(3.1), the MCP tool server + SDK session (3.2, 3.3, 3.5), the runner's live deliberation/
fallback timing (4.1, 4.3), and the CLI (6.1). These need `npm install` + a running server.

**Blocked on the live server / deps** (left unchecked): the full-game and multi-process
runs and the manual playtest — 6.2, 7.1, 7.4 (timer-trigger integration), 7.5 (the
"during a full game" portion), 7.6.

**Provisional surface:** `src/protocol/messages.ts` is hand-authored from the documented
catalog and MUST be regenerated from the Rust `protocol/` crate via `npm run gen:protocol`
(1.2's seam) once that crate lands.
