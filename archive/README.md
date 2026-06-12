# Archive — retired v1 components

Everything here is **retired, not deleted**: kept on disk (full git history via
renames) so it stays discoverable and revivable. Nothing in this directory is built,
linted, or tested — the root cargo workspace excludes it. Retired by change
`retire-v1-harnesses` (see `openspec/changes/archive/2026-06-11-retire-v1-harnesses/`),
which also amended the constitution to v2.0.0.

**Revived and re-retired:** `bot-harness/` moved back to the repo root with
change `boom2-combat-core` (2026-06-11) to deliver the first boom2 balance
derivation against the v4 protocol, then returned here (2026-06-12) by change
`boom2-ai-client`, whose `clients/ai` harness mode (`balance_tester`) fully
supersedes it — same statistics and smells plus decision-frame-driven seats,
codec-exercising byte transports, transport parity, and the matrix sample
spec. It remains revivable per the recipe below, but `clients/ai` is the
standing §IV instrument.

## Inventory

| Entry | What it was | Introduced by | Capabilities removed with it |
|---|---|---|---|
| `bot-harness/` | Layer-1 balance harness — headless protocol bots, seeded batch runner, balance reports (v4-protocol rewrite) | `2026-06-02-bot-playtest-harness`; rewritten by `boom2-combat-core` | superseded by `boom-balance-harness` (`clients/ai`) |
| `tui-client/` | v1 reference client — Rust + ratatui, the original agent-test target | `2026-06-02-terminal-client` (+ `2026-06-05-tui-readability-pass`) | `tui-client-shell`, `tui-lobby`, `tui-round-play`, `tui-reveal-and-score`, `tui-codex`, `tui-debug-and-test` |
| `agent-harness/` | Layer-2 Claude-as-player harness — Node/TypeScript over WebSocket + MCP tools | `2026-06-02-agent-player-harness` | `agent-player`, `agent-personas` |
| `playtest.sh` | One-command solo playtest: server + agent opponents + the TUI | (script, no change) | — |

The removed capability specs survive verbatim as REMOVED deltas in the archived
`retire-v1-harnesses` change and in full in each component's original archived change.

## Revival recipe

The archived crates are intentionally **not buildable in place** — their
`path = "../protocol"` dependencies and `*.workspace = true` keys assume they sit at
the repo root.

1. Move the directory back to the repo root (`git mv archive/<name> <name>`).
2. Rust crates: re-add the crate to `members` in the root `Cargo.toml`, then
   `cargo check --workspace`. (`tui-client` also dev-depends on `server`.)
3. `agent-harness`: `npm install` (Node ≥ 22; it runs TypeScript directly via
   `node --experimental-strip-types`).
4. Restore the component's capability specs into `openspec/specs/` from the
   `retire-v1-harnesses` archived change's delta files (or re-spec against the
   current protocol — after boom2 the v1 specs are likely stale).
5. Amend the constitution (CLAUDE.md) — Principle II/IV mention the archived status.

`playtest.sh` hard-codes root-relative paths (`target/debug/boiling-point-tui`,
`agent-harness/src/cli.ts`, a root `scripts/` location), so it only works after a
full revival of the TUI **and** the agent harness.

## Why they were retired (2026-06-11)

v1 shipped; the boom2 rework + PixiJS web client (`adopt-pixi-client`) are the
forward path. Keeping three unmaintained protocol consumers green made every
protocol/engine change pay a three-client tax. Constitution v2.0.0 (§IV)
required reviving the bot harness for at-scale balance validation before large
balance reworks (boom2) ship — done with `boom2-combat-core`, then handed off
to the AI client's harness mode (`boom2-ai-client`, constitution v2.2.1); the
interim harness returned here once superseded (see above).
