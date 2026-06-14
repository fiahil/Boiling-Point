# Architecture

How Boiling Point is built — the shape of the system, the infrastructure design, the
technology choices behind it, and the observability contract. Everything here serves
one principle: **the server is the only source of truth** (constitution §I).

| # | Page | What it covers |
|---|---|---|
| 01 | [Overview](01_overview.md) | Component map, the `protocol` waist, server internals, the connection & game lifecycle, the client phase state machine, and why replays are cheap (seeded determinism). Start here. |
| 02 | [Server infrastructure design](02_server-infrastructure.md) | The infrastructure *as built*: topology, room lifecycle, concurrency, reconnection, persistence, observability, anti-cheat, and the scaling path. |
| 03 | [Tech-stack exploration](03_tech-stack-exploration.md) | Why Rust / Axum / Tokio / Postgres / MessagePack on the server, and the client decision (PixiJS, with the rejected/deferred alternatives). |
| 04 | [Span-schema contract](04_span-schema-contract.md) | The OpenTelemetry span tree and attribute schema — one instrumentation surface feeding metrics, trace export, and the admin read model. |
| 05 | [Identity, rating & skill-based matchmaking](05_identity-and-rating.md) | The v2 identity stack (`boom2-identity`): persistent accounts (device-bound + OAuth), the Weng-Lin FFA rating, the pluggable skill match policy, and the Principle-IV rating-simulation tuning record. |

The authoritative, executable requirements for these subsystems live in the resolved
capability specs under [`openspec/specs/`](../../openspec/specs/); the pages here are the
human-facing rationale.
