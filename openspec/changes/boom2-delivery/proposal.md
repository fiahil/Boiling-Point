## Why

v1 is locally runnable behind a CI **gate** (`fmt` + `clippy` + unit tests) but nothing is hosted, there is no deploy step, and there is no public presence ([05_roadmap.md — Deployment](../../../docs/05_roadmap.md)). v2 turns the project into a continuously deployed, publicly reachable service. This change lands the **delivery stack** — fuller CI test layers, a deployment target/architecture, a continuous-deployment pipeline, and a landing page — per the roadmap ordering.

## What Changes

- **Fuller tests in CI** — extend the gate beyond `fmt`/`clippy`/unit to the three Principle-II testing layers: transport/integration tests (boot an in-process server), seeded deterministic **bot-harness** balance runs, an **agent-harness** Claude-as-player smoke, and **web-client** build + Playwright visual tests (once the Pixi client lands). This gates everything below.
- **Deployment architecture & target** — pick the target: a **managed container host + managed Postgres** (the Principle-III single-binary monolith maps to one container). Decide TLS/WebSocket ingress, config/secrets, DB backups, and the staging→prod path. Single-server stance is the seam; horizontal scaling stays out.
- **Continuous deployment pipeline** — on green `main`: build + publish the server container and the web-client bundle, run DB migrations, and promote. Gated behind the fuller test suite.
- **Landing page** — a static marketing page (what the game is, screenshots/trailer, a "play now" → create/join CTA) in front of the PixiJS `web-client/`.
- **Benchmarks fold in** — the server-benchmark regression runs (a separate roadmap candidate) land in this pipeline once it exists.
- **Ordering (phased in tasks):** fuller CI tests → deployment target → CD pipeline → landing page (in parallel).

## Capabilities

### New Capabilities

- `ci-test-layers` — CI extended to transport/integration, bot-harness balance, agent smoke, and web-client visual layers; the gate everything else depends on.
- `deployment-target` — the chosen hosting architecture: managed container + managed Postgres, TLS/WebSocket ingress, secrets/config, DB backups, staging→prod.
- `continuous-deployment` — the CD pipeline that builds, publishes, migrates, and promotes on green `main`, gated by `ci-test-layers`.
- `landing-page` — a public static marketing/acquisition page with a play CTA into the web client.

### Modified Capabilities

<!-- Net-new ops/delivery capabilities; no existing spec's behavior changes. The
     game loop, protocol, and balance are untouched by delivery. -->

## Impact

- **CI/CD:** new pipeline stages and a deploy step on top of the existing `.github/workflows/ci.yml` gate; the Makefile targets and bot/agent harnesses are the seams.
- **Infra:** a hosting target, managed Postgres, TLS/WS ingress, secrets, and backups — the first time anything is hosted.
- **Web:** the landing page sits alongside/in front of `web-client/` (depends on the Pixi client from `adopt-pixi-client`; Playwright layer arrives with it).
- **No** server game-logic, protocol, or balance change. Single-server only; horizontal scaling stays out of v2.
