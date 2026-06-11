## Why

v1 is locally runnable behind a CI **gate** (`fmt` + `clippy` + unit tests) but nothing is hosted, there is no deploy step, and there is no public presence ([05_roadmap.md — Deployment](../../../docs/05_roadmap.md)). v2 turns the project into a continuously deployed, publicly reachable service. This change lands the **delivery stack** — fuller CI test layers, a deployment target/architecture, a continuous-deployment pipeline, and a landing page — per the roadmap ordering.

## What Changes

- **Fuller tests in CI** — extend the gate beyond `fmt`/`clippy`/unit to the full Principle-II gate (constitution v2.0.0): **transport/integration** tests (boot an in-process server), and the **web client** (`clients/web/`) build + Playwright visual tests (once the Pixi client lands). A seeded deterministic **bot-harness smoke** joins the gate when the archived harness is revived (required before boom2 balance ships, §IV) — asserting the runs complete and reproduce, never asserting balance-metric bands (those are observational, change `boom2-benchmarking`). This gates everything below.
- **Deployment architecture & target** — a **bare-metal Dedibox**, no containers: the single-binary monolith as a systemd service, PostgreSQL on the same host with nightly **off-site** backups, and **Caddy** as the sole public ingress — automatic TLS, `/ws` WebSocket proxying to the localhost-bound server, and static `file_server` for the landing page and the `clients/web/` bundle. Staging is the developer's localhost. Single-server stance is the seam; horizontal scaling stays out.
- **Continuous deployment pipeline** — on green `main`: build the release server binary and the `clients/web/` bundle, sync them to the box, run DB migrations, and restart the service. Gated behind the fuller test suite.
- **Landing page** — a static marketing page (what the game is, screenshots/trailer, a "play now" → create/join CTA) in front of the PixiJS client (`clients/web/`).
- **Benchmarks fold in** — the benchmarking suite (change `boom2-benchmarking`) rides this pipeline: the per-merge criterion job and the bench-dashboard republish run on green `main`. Benchmarks are observational and never gate; the only harness item in the **gate** is the deterministic smoke above.
- **Ordering (phased in tasks):** fuller CI tests → deployment target → CD pipeline → landing page (in parallel).

## Capabilities

### New Capabilities

- `ci-test-layers` — CI extended to transport/integration and web-client visual layers (plus the revived-harness deterministic smoke when boom2 balance work revives `archive/bot-harness/`); the gate everything else depends on.
- `deployment-target` — the chosen hosting architecture: bare-metal Dedibox, systemd monolith, same-box Postgres with off-site backups, Caddy as TLS/WebSocket/static ingress, localhost-as-staging.
- `continuous-deployment` — the CD pipeline that builds, publishes, migrates, and promotes on green `main`, gated by `ci-test-layers`.
- `landing-page` — a public static marketing/acquisition page with a play CTA into the web client.

### Modified Capabilities

<!-- Net-new ops/delivery capabilities; no existing spec's behavior changes. The
     game loop, protocol, and balance are untouched by delivery. -->

## Impact

- **CI/CD:** new pipeline stages and a deploy step on top of the existing `.github/workflows/ci.yml` gate; the Makefile targets are the seam (the archived harnesses are revivable for the balance layers).
- **Infra:** a bare-metal Dedibox (systemd service, Caddy ingress, same-box Postgres, off-site backups) — the first time anything is hosted.
- **Web:** the landing page sits alongside/in front of `clients/web/` (depends on the Pixi client from `adopt-pixi-client`; Playwright layer arrives with it).
- **No** server game-logic, protocol, or balance change. Single-server only; horizontal scaling stays out of v2.
