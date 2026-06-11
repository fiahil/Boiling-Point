# Design — retire-v1-harnesses

## Context

The cargo workspace is `protocol`, `server`, `tui-client`, `bot-harness`, plus the
standalone Node package `agent-harness/`. Nothing live depends on the three retiring
components except `scripts/playtest.sh` (which orchestrates all of them) and the
`make playtest` target. `tui-client` dev-depends on `server`; `bot-harness` depends on
`server` + `protocol`; the dependency arrows all point *at* the survivors, so the
moves cannot break `server` or `protocol`. The web client (`adopt-pixi-client`) is
specified but not yet scaffolded — this change decides where it lands (`clients/web/`).

## Goals / Non-Goals

**Goals:**
- Retire the three v1 components + `playtest.sh` to `archive/`, revivable.
- Make the tree legible by role: `server/` + `protocol/` (authoritative core),
  `clients/` (renderers), `archive/` (retired).
- Amend the constitution so governance matches reality (v2.0.0).
- Leave `openspec/specs/` describing only the live system.

**Non-Goals:**
- Deleting anything (retire = archive).
- Any server, protocol, game-logic, balance, or wire-format change.
- Implementing `clients/web/` (that is `adopt-pixi-client`'s scope).
- Reviving or porting the harnesses to boom2 (a future change, gated by §IV).

## Decisions

### D1: Top-level `archive/` with a README

`archive/{tui-client,bot-harness,agent-harness,playtest.sh}` plus `archive/README.md`
stating what each entry was, which change introduced it, which capabilities left with
it, and the revival recipe. *Alternative rejected:* history-only deletion (R1).

### D2: `exclude = ["archive"]` in the root workspace

Members shrink to `["protocol", "server"]`; the exclude keeps cargo from ever
treating archived crates as workspace packages. Archived crates are intentionally not
buildable in place (`../protocol` path deps); revival = move back + re-add (R2).

### D3: Constitution v2.0.0

Principle II keeps its name, opening sentence, agent-writability bullet, and the
"agent testability is a first-class criterion" bullet. The three-layer mandate
becomes two live layers — server tests (unit + transport/integration over the real
wire protocol) and Playwright visual tests (landing with `clients/web`) — plus an
explicit statement that the v1 harnesses are archived and revivable. Principle IV
re-sources "data-informed" (telemetry, balance dashboard, structured playtests,
revived-harness statistics) and converts the standing-harness mandate into
reinstate-before-large-balance-reworks-ship. MAJOR per the constitution's own
versioning rule (R4).

### D4: `clients/` group; the web client lands at `clients/web/`

The user picked the `clients/` layout: server-side crates stay at root (no workspace
churn), client renderers group under `clients/`. All `web-client/` references in
docs and in `adopt-pixi-client` are re-pathed to `clients/web/`.

### D5: Docs strategy — live docs updated, history annotated, never rewritten

Live guidance (README, getting-started, overview, roadmap, design system, protocol
README) is rewritten to the new reality. Dated artifacts (reviews, archived changes,
tech-stack exploration narrative, boom2 decision logs) keep their text; at most they
gain a one-line retirement pointer or move to `docs/99_archive/` (R5).

## Constitution Check

- **§I Server-Authoritative** — ✅ unaffected; no game-state, validation, or wire
  change. The server loses no authority by losing dead clients.
- **§II Agent-Driven Development** — ⚠️ this change **amends §II itself** (the
  BREAKING governance item, R4). What survives intact: all source stays
  agent-writable, and agent testability remains a first-class client-selection
  criterion. The violation of the *old* §II (dropping two mandated layers) is
  documented here and legalized by the v2.0.0 amendment in the same change.
- **§III Start Simple, Scale Later** — ✅ archiving three unmaintained components
  *is* the simpler option. *Rejected alternative:* keep building, linting, and
  type-syncing three dead clients on every protocol/engine change — complexity with
  no consumer. The seam for "the thing we might need tomorrow" is the archive itself.
- **§IV Playtest-Driven Balance** — ⚠️ amended in the same bump: balance stays
  data-informed (telemetry, dashboard, structured playtests), and at-scale automated
  playtesting MUST be reinstated — by reviving `archive/bot-harness/` — before large
  balance reworks (boom2) ship. The capability is deferred, not abandoned.

## Risks / Trade-offs

- **boom2 balance work loses its standing harness** → amended §IV hard-gates shipping
  on reviving it; `archive/README.md` documents the recipe; the harness is preserved
  verbatim.
- **Stale references rot across 30+ docs/changes** → a grep gate in tasks (only
  `archive/`, `openspec/changes/archive/`, `docs/99_archive/`, and dated narrative
  may still mention the old paths).
- **In-flight worktree (`pr-14`, server replays) predates the moves** → unrelated
  files; it rebases over pure renames with no content conflicts expected.
- **`scripts/` directory disappears from git** (playtest.sh was its only file) →
  accepted; docs stop pointing at it; it returns when a new script lands.

## Migration Plan

Order: OpenSpec artifacts → constitution → moves + build plumbing → live docs →
active-change reconciliation → validate → archive the change. Rollback is a plain
`git revert` — the moves are pure renames and no data or schema changes.

## Open Questions

None — R1–R6 resolved; layout and process decided with the user.
