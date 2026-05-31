# agent-harness — Boiling Point Layer-2 (Claude-as-player)

A tunable, persona-driven Claude opponent that fills seats so Boiling Point can be
playtested by one person. It connects over the **same public WebSocket protocol a
real client uses** and receives only player-permitted information. See
`openspec/changes/agent-player-harness/` for the proposal, research, specs, and design.

This is a **TypeScript/Node package** (not a cargo crate) so it can use the
**Claude Agent SDK** and bill against a Claude subscription — see design D1/D2.

## Status: v0, server-dependent parts untested

`server-release-1` (which provides the Rust `protocol/` crate) is being built in
parallel and is not yet committed. Therefore:

- **The protocol surface in `src/protocol/messages.ts` is PROVISIONAL** — hand-authored
  from the documented message catalog (`knowledge/brainstorming/server-architecture.md`
  §4, `server-release-1` `wire-protocol`/`round-engine` specs, and the v2 game design).
  It MUST be regenerated from the Rust `protocol/` crate via `ts-rs` once that crate
  lands (see `scripts/gen-protocol-types.ts` for the seam). Variant names use the
  `#[serde(tag = "type")]` discriminant the server emits.
- **Everything that talks to a live server or the Agent SDK runtime is untested.**
  Run those only after the server is committed (per the apply instruction).

## What runs today, without a server or `npm install`

The pure-logic modules import only protocol **types**, so Node's type-stripping runs
their unit tests with no dependencies installed:

```
node --experimental-strip-types --test test/*.test.ts
```

These cover the view model, the secret-boundary assertion, difficulty→`allowedTools`
mapping, the thin-context gating rule, blunder injection, the fallback heuristic, and
persona emote selection.

## Running an agent (after the server is up and deps are installed)

```
npm install
CLAUDE_CODE_OAUTH_TOKEN=… npx bp-bot --room BREW-7K3F --difficulty hard --persona gambler
```

`--difficulty` ∈ {easy, hard}; `--persona` ∈ {gambler, turtle, bandwagoner, trickster}
(optional; omit to play straight); `--epsilon <0..1>` enables blunder injection (default 0).
Auth uses subscription OAuth; any `ANTHROPIC_API_KEY` in the environment is neutralized
so play does not silently bill API credits (design D2).

## Layout

```
src/
  protocol/   messages.ts (PROVISIONAL), codec.ts, index.ts
  net/        connection.ts, view-model.ts, secret-boundary.ts, wave-lifecycle.ts
  agent/      tools.ts, context.ts, difficulty.ts, session.ts, tool-names.ts
  timeliness/ fallback.ts
  personas/   archetypes.ts, emotes.ts, blunder.ts
  auth.ts, runner.ts, cli.ts
```
