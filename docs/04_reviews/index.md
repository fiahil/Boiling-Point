# Reviews

Code-health and launch-readiness assessments. Each review evaluates a component against
the [constitution](../../CLAUDE.md) and the [game design](../02_game-design.md), records
findings, and tracks their resolution. Most recently refreshed **2026-06-05**.

| # | Page | What it covers |
|---|---|---|
| 01 | [Release-readiness review](01_release-readiness-review.md) | The cross-cutting view: what stands between today's codebase and a public v1 launch (launch gates, should-fixes, recommended sequence). **Start here** — it synthesizes the component reviews. |
| 02 | [Server review](02_server-review.md) | The Rust server (`server/`) and the `protocol/` crate — architecture walkthrough, concurrency model, subsystems, and findings F1–F5. |

The TUI-client and agent-harness reviews retired with their components
(`retire-v1-harnesses`, 2026-06-11) — they live on in
[99_archive/](../99_archive/index.md), the components in
[`archive/`](../../archive/README.md).
