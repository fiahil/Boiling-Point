# Reviews

Code-health and launch-readiness assessments. Each review evaluates a component against
the [constitution](../../CLAUDE.md) and the [game design](../02_game-design.md), records
findings, and tracks their resolution. Most recently refreshed **2026-06-05**.

| # | Page | What it covers |
|---|---|---|
| 01 | [Release-readiness review](01_release-readiness-review.md) | The cross-cutting view: what stands between today's codebase and a public v1 launch (launch gates, should-fixes, recommended sequence). **Start here** — it synthesizes the component reviews below. |
| 02 | [Server review](02_server-review.md) | The Rust server (`server/`) and the `protocol/` crate — architecture walkthrough, concurrency model, subsystems, and findings F1–F5. |
| 03 | [Terminal client review](03_tui-client-review.md) | The `tui-client/` ratatui renderer — the reference untrusted client and Layer-3 test target. |
| 04 | [Agent-harness review](04_agent-harness-review.md) | The `agent-harness/` Claude-as-player harness (Layer-2 testing) — secret-boundary analysis and findings. |
