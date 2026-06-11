# agent-harness — Boiling Point Layer-2 (Claude-as-player)

A tunable, persona-driven Claude opponent that fills seats so Boiling Point can be
playtested by one person. It connects over the **same public WebSocket protocol a real
client uses** (`protocol/`) and receives only player-permitted information. See the
shipped proposal/research/specs/design in
`openspec/changes/archive/2026-06-02-agent-player-harness/`, the review in
[`docs/04_reviews/04_agent-harness-review.md`](../docs/04_reviews/04_agent-harness-review.md),
and the project [docs hub](../docs/).

This is a **TypeScript/Node package** (not a cargo crate) so it can use the **Claude Agent
SDK** and bill against a Claude subscription — see design D1/D2.

## Status

Integrated against the committed server and verified end-to-end:

- `npm test` — 24 pure-logic unit tests (view model, secret boundary, gating, difficulty,
  personas, blunder, fallback). `npm run typecheck` is clean against Agent SDK 0.2.141.
- Four `--brain fallback` bots auto-match and play a complete game to `GameOver`.
- The Claude brain authenticates via your Claude subscription (Claude Code login) and drives
  real decisions through in-process MCP tools.

The Claude brain uses a **persistent Agent SDK session** (one warm subprocess per game,
context preserved across waves). Per-decision latency is ~15s and is **model/rate-limit
bound** (not subprocess startup), so on the 10s sub-waves the agent falls back to the local
heuristic — deliberating at the previous wave's resolution plus the fallback keep the table
flowing (it never stalls). Use `--brain fallback` for instant, zero-cost seats, or relax the
room timers for hard presets. `scripts/probe-agent.ts` measures two decisions on one warm
session.

## Running

The server must be running (`cargo run -p boiling-point-server`, listens on
`ws://127.0.0.1:8080/ws`).

```bash
npm install

# One bot opens a room and prints the invite code; share it with the others / your client:
node --experimental-strip-types src/cli.ts --create --persona gambler

# Join a known room:
node --experimental-strip-types src/cli.ts --room BREW-7K3F --difficulty hard --persona turtle

# Zero-cost heuristic seat-fillers via auto-match (no LLM, no auth needed):
node --experimental-strip-types src/cli.ts --enqueue --brain fallback
```

Flags: `--room <code>` | `--create` | `--enqueue` (entry mode); `--brain claude|fallback`;
`--difficulty easy|hard`; `--persona gambler|turtle|bandwagoner|trickster` (optional — omit
to play straight); `--epsilon <0..1>` (blunder injection, default 0); `--url`, `--name`.

Auth uses your Claude subscription via the Claude Code CLI login (or `CLAUDE_CODE_OAUTH_TOKEN`);
any `ANTHROPIC_API_KEY` is neutralized so play does not silently bill API credits (set
`BP_ALLOW_API_KEY=1` to opt in). Set `BP_DEBUG=1` to log each decision.

To verify quickly: `node --experimental-strip-types scripts/probe-agent.ts` runs one isolated
Haiku decision (no server needed).

## Protocol surface

`src/protocol/messages.ts` is an exact hand-mirror of the Rust `protocol/` crate. The wire is
**MessagePack only** (the server ignores text frames); `uuid` `PlayerId`s arrive as bytes and
are canonicalized to hex strings by the codec. Regenerating the types via a feature-gated
`ts-rs` derive (`npm run gen:protocol`) remains the proper long-term step once the crate
derives `ts-rs`.

## Layout

```
src/
  protocol/   messages.ts (hand-mirror), codec.ts, index.ts
  net/        connection.ts, view-model.ts, secret-boundary.ts, wave-lifecycle.ts
  agent/      tools.ts, context.ts, difficulty.ts, session.ts, prompt.ts, tool-names.ts, actions.ts
  timeliness/ fallback.ts
  personas/   archetypes.ts, emotes.ts, blunder.ts
  auth.ts, runner.ts, cli.ts
scripts/      gen-protocol-types.ts (ts-rs seam), probe-agent.ts (one-shot SDK probe)
```
