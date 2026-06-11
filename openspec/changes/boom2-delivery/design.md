## Context

v1 ships a CI gate (`fmt`/`clippy`/unit) and a locally runnable monolith, with delivery deliberately deferred ([05_roadmap.md — Deployment](../../../docs/05_roadmap.md)). This change makes the project a continuously deployed, publicly reachable service, reusing the existing Makefile targets as the seam (the archived harnesses are revivable for the balance layers — `retire-v1-harnesses`).

## Goals / Non-Goals

**Goals**
- Extend CI to the full Principle-II gate, v2.0.0 (the gate for everything).
- Stand up the hosting target (bare-metal Dedibox: systemd monolith, same-box Postgres, Caddy ingress).
- A CD pipeline that builds/publishes/migrates/promotes on green `main`.
- A static landing page with a play CTA.

**Non-Goals**
- Horizontal scaling (single-server only; the seam is documented).
- Any game-loop/protocol/balance change.
- The `web-client` itself (that is `adopt-pixi-client`); this consumes its bundle and Playwright suite.

## Decisions

### D1: Tests gate delivery

The fuller CI layers land **first** and gate the CD pipeline — nothing deploys on a red gate. The revived bot harness contributes a seeded/deterministic **smoke** to the gate (completion + reproducibility only); its balance metrics stay observational in `boom2-benchmarking`. *Alternative rejected:* deploy first, harden tests later — risks shipping balance/transport regressions to a live service.

### D2: Bare-metal Dedibox, no containers

The service runs on a single bare-metal dedicated server (Scaleway Dedibox): the single-binary monolith as a systemd service, PostgreSQL on the same host, secrets injected at runtime via a root-only environment file. Nightly `pg_dump` backups ship **off-site** to object storage — same-disk backups don't survive the box's primary failure mode. *Alternative rejected:* managed container host + managed Postgres — adds a platform layer and per-service cost without removing the ops work that remains ours either way (migrations, secrets, backup verification), while the owned box gives fixed cost and full control of the stack, including TLS.

### D3: Single-server only

Horizontal scaling stays out of v2; the single-server stance is the explicit seam. *Alternative rejected:* build clustering now — premature before load justifies it.

### D4: Static, independent landing page

A static page deployable in front of `clients/web/`, cacheable, with no game logic. *Alternative rejected:* server-rendered marketing — couples acquisition to the game server.

### D5: Caddy is the sole public ingress and serves all statics

Caddy terminates TLS (automatic ACME) and is the only publicly exposed process: it reverse-proxies `/ws` to the game server (which binds localhost only) and serves all static content via `file_server` — the landing page at `/` and the `clients/web/` bundle under `/play`. The Rust server stays a pure `/ws` origin with no static serving. The admin API (`:8081`) and Grafana stay localhost-bound, reached over an SSH tunnel/VPN, never routed through public Caddy. *Alternatives rejected:* nginx — manual ACME plus the WebSocket `Upgrade`/`Connection` header dance for the same result; serving statics from the Rust binary (`tower-http`) — couples the landing page's availability to the game-server process, against D4; split static hosting/CDN — a second provider and deploy target the single box makes unnecessary.

### D6: Staging is the developer's localhost

There is no hosted staging environment: pre-deploy verification is the CI gate plus the full stack (server, Postgres, client bundle) running on the developer's machine, and green `main` deploys straight to production. *Alternative rejected:* a hosted staging instance (second box, or a second instance on the same box) — more infra than a single-server project warrants; the CI gate is the real promotion barrier.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | Delivery is infrastructure only; it changes no game logic and moves nothing authoritative client-side. The landing page carries no game logic. |
| **II — Agent-driven** | CI runs the v2.0.0 testing layers (transport/integration, web visual; revived-harness balance runs when boom2 revives `archive/bot-harness/`); pipelines are source-defined (YAML), agent-writable. |
| **III — Start simple** | One bare-metal box running the monolith under systemd, Postgres alongside, Caddy as the sole ingress — no containers, no orchestration, **single-server only** (scaling deferred), CD layered on the existing CI gate rather than a new system. **Rejected simpler alternative:** stay local-only — but v2's purpose is a reachable, continuously delivered service. |
| **IV — Playtest-driven** | The pipeline runs the `boom2-benchmarking` suite (per-merge criterion + dashboard; on-demand balance studies stay outside CI) so balance and performance are tracked release-over-release — observationally, never as a gate. The revived harness's gate contribution is the determinism smoke only. |

## Risks / Migration

- **Depends on `adopt-pixi-client`** for the `clients/web/` bundle + Playwright layer; CI's web-visual layer activates once that lands.
- **Single box, single disk** — the Dedibox is the only host; the off-site backups (D2) are the recovery path, and a restore must be exercised, not assumed.
- **Deploys restart the process** — a `systemctl restart` drops live WebSocket connections; without a graceful drain, every merge to `main` can interrupt in-flight matches (task 3.4).
