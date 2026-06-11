# Research — retire-v1-harnesses

## Open Questions

- R1: Archive or delete? Where does retired code live?
- R2: How should cargo treat archived crates?
- R3: What happens to the spec capabilities owned by retired components?
- R4: Constitution bump — MAJOR or MINOR?
- R5: What happens to the component review docs?
- R6: How does boom2 get balance validation without a standing harness?

## R1: Archive or delete?

**Decision:** Archive at a new top-level `archive/` (with `archive/README.md`), never
delete. Decided with the user: *retire means "archive"*.

**Rationale:** Git history alone hides retired code behind archaeology; an on-disk
archive keeps it discoverable and trivially revivable, and the top level makes the
role split (server / clients / archive) legible at a glance.

**Alternatives considered:** Deletion (history-only) — rejected, the user explicitly
wants archive semantics and §IV expects the bot harness to come back. Per-subsystem
archives (`docs/99_archive/`-style nesting) — rejected for code, the components are
peers of `server/`, not docs.

**Key details:** `archive/` holds `tui-client/`, `bot-harness/`, `agent-harness/`,
`playtest.sh`. Moves are pure `git mv` (rename detection preserves history).
`agent-harness/node_modules` is untracked and is removed, not moved.

## R2: How should cargo treat archived crates?

**Decision:** Drop `tui-client` and `bot-harness` from `members`, add
`exclude = ["archive"]` to the root `[workspace]`.

**Rationale:** Root `--workspace` builds ignore non-members anyway, but without
`exclude`, running cargo *inside* `archive/tui-client/` errors with "package is not a
member of the enclosing workspace"; `exclude` makes the intent explicit and keeps any
future member glob from swallowing the archive.

**Key details:** Archived crates are not buildable in place regardless — their
`path = "../protocol"` dependencies and `*.workspace = true` keys assume root
siblings. Revival = move the directory back to root and re-add it to `members`.
`Cargo.lock` prunes ratatui/crossterm and friends on the next `cargo check`.

## R3: What happens to the retired capabilities' specs?

**Decision:** REMOVED deltas for every requirement of the 10 wholly-owned
capabilities (`bot-harness`, `balance-analysis`, `agent-player`, `agent-personas`,
`tui-client-shell`, `tui-codex`, `tui-debug-and-test`, `tui-lobby`,
`tui-reveal-and-score`, `tui-round-play`); the capability directories leave
`openspec/specs/` at archive-time sync.

**Rationale:** `openspec/specs/` describes the current system; retired capabilities
no longer belong there. The full text survives twice over — in this change's archived
delta files and in the original archived changes (`2026-06-02-bot-balance-harness`,
`2026-06-02-agent-player-harness`, `2026-06-02-terminal-client`,
`2026-06-05-tui-readability-pass`).

**Alternatives considered:** An `openspec/specs/archive/` — rejected, the CLI would
parse it as a capability and the changes archive already is the historical record.

**Key details:** `balance-dashboard` stays (it is the server admin UI from
`admin-ui`). `wire-protocol`'s incidental "every client and bot" phrasing stays
(generically true; no delta churn).

## R4: Constitution bump — MAJOR or MINOR?

**Decision:** MAJOR → v2.0.0.

**Rationale:** CLAUDE.md's own rule: "MAJOR: Principle removal, redefinition, or
backward-incompatible governance change." Principle II's three-layer mandate and
Principle IV's standing bot-harness mandate are both redefined; previously-mandatory
artifacts cease to be mandatory.

## R5: What happens to the component review docs?

**Decision:** Move `03_tui-client-review.md` and `04_agent-harness-review.md` to
`docs/99_archive/` (un-numbered, per that directory's convention); leave
`02_server-review.md` in place.

**Rationale:** `docs/99_archive/`'s charter is "nothing here is current guidance" —
exactly what a review of a retired component is. The server review covers a live
component; its dated harness mentions are historical snapshots, not instructions.

## R6: How does boom2 get balance validation without a standing harness?

**Decision:** Reviving `archive/bot-harness/` is the documented path, and amended §IV
makes it mandatory before large balance reworks (boom2) ship. Until then, balance
stays data-informed via server telemetry, the admin balance dashboard, and structured
playtests.

**Rationale:** boom2 changes the combat core enough that the v1 strategies need
rework anyway; keeping the harness green against a moving v1 protocol is pure tax.
Archiving now and reviving against the boom2 protocol is the cheaper sequence, and
the constitution keeps the at-scale requirement so it cannot be silently skipped.

## Summary

- R1 — archive at top-level `archive/` with README; pure git renames.
- R2 — `members = ["protocol", "server"]` + `exclude = ["archive"]`.
- R3 — REMOVED deltas ×10 capabilities; 32 → 22 specs at sync; `balance-dashboard`
  and `wire-protocol` untouched.
- R4 — MAJOR bump to v2.0.0.
- R5 — review docs → `docs/99_archive/`; server review stays.
- R6 — boom2 balance = revive the harness (required by amended §IV before shipping).
