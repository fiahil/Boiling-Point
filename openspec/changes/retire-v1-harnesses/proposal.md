# Retire the v1 harnesses and rework the tree

## Why

v1 has shipped and the forward path is the boom2 rework rendered by the PixiJS web
client (`adopt-pixi-client`). The v1 TUI reference client, bot harness, and agent
harness are no longer maintained against that path, yet as live workspace members they
make every protocol/engine change pay a three-client tax. Retire them to `archive/`
(revivable, not deleted) and rework the tree so assets are identifiable by role
(server vs clients vs retired).

## What Changes

- Move `tui-client/`, `bot-harness/`, `agent-harness/`, and `scripts/playtest.sh` into
  a new top-level `archive/` (pure git renames — full history preserved, nothing
  deleted; revival recipe in `archive/README.md`).
- Shrink the cargo workspace to `members = ["protocol", "server"]` and add
  `exclude = ["archive"]`; `Cargo.lock` prunes the TUI/harness dependency trees.
- Introduce the `clients/` group: the PixiJS client of `adopt-pixi-client` lands at
  `clients/web/` instead of a root-level `web-client/`.
- Drop the `make playtest` target (it orchestrated server + TUI + agent harness).
- **BREAKING (governance):** constitution amendment v1.1.0 → **v2.0.0** (MAJOR).
  Principle II's three mandatory testing layers are redefined — server tests (now) and
  Playwright visual tests (when `clients/web` lands); the retired harnesses become
  revivable rather than standing. Principle IV's standing bot-harness mandate becomes
  reinstate-on-revival, required before large balance reworks (boom2) ship.
- **Remove the 10 spec capabilities wholly owned by the retired components** (REMOVED
  deltas in this change's `specs/`).
- Reconcile active changes that reference the retired components or the old client
  path: `adopt-pixi-client`, `boom2-delivery`, `boom2-apothecary`, `boom2-brewers`,
  `boom2-combat-core`, `boom2-compounding`, `boom2-localization`.
- Move the two retired-component review docs (`docs/04_reviews/03_tui-client-review.md`,
  `docs/04_reviews/04_agent-harness-review.md`) to `docs/99_archive/`; update live docs
  (README, getting-started, architecture overview, roadmap, design system, indexes).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None — no surviving spec's requirements change. (The unmerged delta specs inside
active changes — `boom2-delivery`'s `ci-test-layers` and `boom2-localization`'s
`localization` — are edited in place in those changes, not via deltas here.)

### Removed Capabilities

- `bot-harness` — the Layer-1 headless balance client is retired to
  `archive/bot-harness/`.
- `balance-analysis` — the seeded batch runner / statistics / smell detection lived in
  the bot harness; at-scale runs return when the harness is revived (constitution §IV).
- `agent-player` — the Layer-2 Claude-as-player client is retired to
  `archive/agent-harness/`.
- `agent-personas` — persona playstyles/emotes/blunders were agent-harness behavior.
- `tui-client-shell` — the terminal reference client is retired to
  `archive/tui-client/`.
- `tui-codex` — TUI-only codex screen.
- `tui-debug-and-test` — TUI debug overlay, replay, and snapshot tests.
- `tui-lobby` — TUI lobby screens.
- `tui-reveal-and-score` — TUI reveal/boom/scoring screens.
- `tui-round-play` — TUI round-play screens.

## Impact

- **Build:** root `Cargo.toml` (members + exclude), `Cargo.lock` prune, `Makefile`
  (drop `playtest`), `.gitignore` (archive-relative node_modules guard). CI needs no
  edits — its `make fmt/lint/test-unit` targets are `--workspace`-scoped and
  auto-adjust; rust-cache keys off `Cargo.lock`.
- **Governance:** `CLAUDE.md` → v2.0.0 (Principles II and IV redefined, structure
  block reworked, amendment logged).
- **Specs:** 32 → 22 capabilities in `openspec/specs/` at archive-time sync.
- **Docs:** README, `docs/index.md`, getting-started, architecture overview &
  tech-stack tree, roadmap, design system, review indexes, `protocol/README.md`.
- **Active changes:** path and harness-claim reconciliation (see What Changes).
- **No** server, protocol, game-logic, balance, or wire-format change.
