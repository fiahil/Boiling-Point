## Context

v1 ships a CI gate (`fmt`/`clippy`/unit) and a locally runnable monolith, with delivery deliberately deferred ([05_roadmap.md — Deployment](../../../docs/05_roadmap.md)). This change makes the project a continuously deployed, publicly reachable service, reusing the existing Makefile targets and bot/agent harnesses as the seams.

## Goals / Non-Goals

**Goals**
- Extend CI to the three Principle-II testing layers (the gate for everything).
- Choose and stand up a hosting target (managed container + managed Postgres, staging→prod).
- A CD pipeline that builds/publishes/migrates/promotes on green `main`.
- A static landing page with a play CTA.

**Non-Goals**
- Horizontal scaling (single-server only; the seam is documented).
- Any game-loop/protocol/balance change.
- The `web-client` itself (that is `adopt-pixi-client`); this consumes its bundle and Playwright suite.

## Decisions

### D1: Tests gate delivery

The fuller CI layers land **first** and gate the CD pipeline — nothing deploys on a red gate. Balance runs are seeded/deterministic so a regression is attributable. *Alternative rejected:* deploy first, harden tests later — risks shipping balance/transport regressions to a live service.

### D2: Managed container + managed Postgres, one container

The Principle-III single-binary monolith maps cleanly to one container; managed Postgres avoids running a database. Staging mirrors prod; promotion flows staging→prod. *Alternative rejected:* bespoke VM/orchestrator setup — more ops than a single monolith warrants pre-scale.

### D3: Single-server only

Horizontal scaling stays out of v2; the single-server stance is the explicit seam. *Alternative rejected:* build clustering now — premature before load justifies it.

### D4: Static, independent landing page

A static page deployable in front of `web-client/`, cacheable, with no game logic. *Alternative rejected:* server-rendered marketing — couples acquisition to the game server.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | Delivery is infrastructure only; it changes no game logic and moves nothing authoritative client-side. The landing page carries no game logic. |
| **II — Agent-driven** | CI runs all three agent-relevant testing layers (transport, bot-harness, agent-harness, web visual); pipelines are source-defined (YAML), agent-writable. |
| **III — Start simple** | A single managed container + managed Postgres, **single-server only** (scaling deferred), CD layered on the existing CI gate rather than a new system. **Rejected simpler alternative:** stay local-only — but v2's purpose is a reachable, continuously delivered service. |
| **IV — Playtest-driven** | The seeded bot-harness balance runs and benchmark regressions live in the pipeline, so balance and performance are tracked release-over-release. |

## Risks / Migration

- **Depends on `adopt-pixi-client`** for the web-client bundle + Playwright layer; CI's web-visual layer activates once that lands.
- **Secrets/DB backups** are new operational surface; get them right before public traffic.
- **Cost/latency of the hosting target** is a real decision; the single-container monolith keeps it minimal.
