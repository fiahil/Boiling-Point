## Context

v1 ships a CI gate (`fmt`/`clippy`/unit) and a locally runnable monolith, with delivery deliberately deferred ([05_roadmap.md — Deployment](../../../docs/05_roadmap.md)). This change makes the project a continuously deployed, publicly reachable service, reusing the existing Makefile targets as the seam (the archived harnesses are revivable for the balance layers — `retire-v1-harnesses`).

## Goals / Non-Goals

**Goals**
- Extend CI to the full Principle-II gate, v2.0.0 (the gate for everything).
- Choose and stand up a hosting target (managed container + managed Postgres, staging→prod).
- A CD pipeline that builds/publishes/migrates/promotes on green `main`.
- A static landing page with a play CTA.

**Non-Goals**
- Horizontal scaling (single-server only; the seam is documented).
- Any game-loop/protocol/balance change.
- The `web-client` itself (that is `adopt-pixi-client`); this consumes its bundle and Playwright suite.

## Decisions

### D1: Tests gate delivery

The fuller CI layers land **first** and gate the CD pipeline — nothing deploys on a red gate. The revived bot harness contributes a seeded/deterministic **smoke** to the gate (completion + reproducibility only); its balance metrics stay observational in `boom2-benchmarking`. *Alternative rejected:* deploy first, harden tests later — risks shipping balance/transport regressions to a live service.

### D2: Managed container + managed Postgres, one container

The Principle-III single-binary monolith maps cleanly to one container; managed Postgres avoids running a database. Staging mirrors prod; promotion flows staging→prod. *Alternative rejected:* bespoke VM/orchestrator setup — more ops than a single monolith warrants pre-scale.

### D3: Single-server only

Horizontal scaling stays out of v2; the single-server stance is the explicit seam. *Alternative rejected:* build clustering now — premature before load justifies it.

### D4: Static, independent landing page

A static page deployable in front of `clients/web/`, cacheable, with no game logic. *Alternative rejected:* server-rendered marketing — couples acquisition to the game server.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | Delivery is infrastructure only; it changes no game logic and moves nothing authoritative client-side. The landing page carries no game logic. |
| **II — Agent-driven** | CI runs the v2.0.0 testing layers (transport/integration, web visual; revived-harness balance runs when boom2 revives `archive/bot-harness/`); pipelines are source-defined (YAML), agent-writable. |
| **III — Start simple** | A single managed container + managed Postgres, **single-server only** (scaling deferred), CD layered on the existing CI gate rather than a new system. **Rejected simpler alternative:** stay local-only — but v2's purpose is a reachable, continuously delivered service. |
| **IV — Playtest-driven** | The pipeline runs the `boom2-benchmarking` suite (per-merge criterion + dashboard; on-demand balance studies stay outside CI) so balance and performance are tracked release-over-release — observationally, never as a gate. The revived harness's gate contribution is the determinism smoke only. |

## Risks / Migration

- **Depends on `adopt-pixi-client`** for the `clients/web/` bundle + Playwright layer; CI's web-visual layer activates once that lands.
- **Secrets/DB backups** are new operational surface; get them right before public traffic.
- **Cost/latency of the hosting target** is a real decision; the single-container monolith keeps it minimal.
