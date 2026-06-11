# Tasks — retire-v1-harnesses

## 1. Constitution amendment (CLAUDE.md → v2.0.0)

- [x] 1.1 Bump the header to v2.0.0 and redefine Principle II (two live testing
      layers: server tests + Playwright visual tests; harnesses archived & revivable;
      agent-writability and agent-testability bullets kept)
- [x] 1.2 Redefine Principle IV (data-informed sources; at-scale playtesting
      reinstated via the revived bot harness before large balance reworks ship)
- [x] 1.3 Update the Technology Stack TUI sentence (past tense, archive pointer) and
      the project-structure block (server, protocol, clients/web, archive)
- [x] 1.4 Prepend the v2.0.0 amendment-log entry

## 2. Moves & build plumbing

- [x] 2.1 Remove untracked `agent-harness/node_modules` and `.playtest/`
- [x] 2.2 `git mv` `tui-client`, `bot-harness`, `agent-harness` → `archive/`, and
      `scripts/playtest.sh` → `archive/playtest.sh`
- [x] 2.3 Root `Cargo.toml`: `members = ["protocol", "server"]`,
      `exclude = ["archive"]`, rewrite the header comment; `cargo check --workspace`
      to prune `Cargo.lock`
- [x] 2.4 `Makefile`: drop the `playtest` target (+ comment + `.PHONY` entry)
- [x] 2.5 `.gitignore`: re-path the node_modules guard to
      `/archive/agent-harness/node_modules/`; drop the `/.playtest/` line
- [x] 2.6 Write `archive/README.md` (inventory + revival recipe)

## 3. Live docs

- [x] 3.1 `README.md`: new workspace map (clients/web future, archive/); drop
      playtest + scripts rows
- [x] 3.2 `docs/index.md`: re-path review rows to 99_archive; per-crate READMEs →
      protocol + server; drop playtest mention
- [x] 3.3 `docs/01_getting-started.md`: drop Node/TUI prerequisites, TUI/bot-harness
      run commands, the one-command-playtest section; re-source balance tuning (§IV)
- [x] 3.4 `docs/03_architecture/01_overview.md`: rework components diagram
      (clients/web + server over protocol); retitle the TUI phase-state-machine
      section as archived reference
- [x] 3.5 `docs/03_architecture/03_tech-stack-exploration.md`: update the structure
      tree + one-line retirement note
- [x] 3.6 `docs/05_roadmap.md`: CI layers, localization consumer, harness mentions →
      "if revived" / `archive/bot-harness/`
- [x] 3.7 `docs/07_design-system.md`: `web-client/` → `clients/web/`; palette pointer
      → `archive/tui-client/src/palette.rs`
- [x] 3.8 Move the TUI + agent-harness reviews to `docs/99_archive/`; update
      `docs/04_reviews/index.md`, `01_release-readiness-review.md` links, and
      `docs/99_archive/index.md`
- [x] 3.9 `docs/06_boom2/index.md`: one-line note that harness-based validation now
      implies reviving `archive/bot-harness/`
- [x] 3.10 `protocol/README.md`: replace the "keep agent-harness types in sync"
      instruction with the typegen-for-`clients/web/` story

## 4. Active-change reconciliation

- [x] 4.1 `adopt-pixi-client`: `web-client/` → `clients/web/` everywhere; reword the
      "TUI remains the reference client" / "harnesses unaffected" claims as
      superseded; drop TUI-parity from task 9.3; align design D5 + Constitution Check
      with v2.0.0
- [x] 4.2 `boom2-delivery`: rewrite the `ci-test-layers` delta spec, proposal CI
      bullet, tasks 1.2/1.3, and design Constitution Check rows for v2.0.0
- [x] 4.3 `boom2-apothecary` 5.1, `boom2-brewers` 4.1 + proposal, `boom2-combat-core`
      7.1 + proposal/design, `boom2-compounding` 6.1: retarget client work "TUI:" →
      "Web client (`clients/web/`):"
- [x] 4.4 `boom2-localization`: proposal "both clients" bullets, task 1.2, and the
      `localization` delta-spec requirement → single client `clients/web/`

## 5. Validate & archive

- [x] 5.1 `cargo check --workspace` && `make fmt lint test-unit` (CI parity)
- [x] 5.2 `openspec validate retire-v1-harnesses --strict` and
      `openspec validate --all --strict`
- [x] 5.3 Stale-reference grep — hits only under `archive/`,
      `openspec/changes/archive/`, `docs/99_archive/`, and dated narrative
- [x] 5.4 Logical commits; archive this change via the archive workflow (syncs the
      REMOVED deltas: 32 → 22 specs)
