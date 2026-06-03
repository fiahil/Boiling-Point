# Boiling Point — Release-Readiness Review

A cross-cutting assessment of what stands between today's codebase and a **public v1
launch**, synthesizing the component reviews ([server](server-review.md),
[tui-client](tui-client-review.md), [agent-harness](agent-harness-review.md)) against
the [constitution](../../CLAUDE.md), [game design](../game-design.md), and
[roadmap](../roadmap.md).

Assessed 2026-06-02 against `main`. **Verdict: feature-complete game core, not yet a
shippable product.** The engine is correct, server-authoritative, and well-tested; the
gaps are integration/operational — no shippable player client in this repo, persistence
not wired, no deployment story, and a handful of constitution/robustness items tracked
in the `review-remediation` and `persistence-and-replays` changes.

---

## What "release" means here

The repo contains the server, a **terminal** client (a dev/agent renderer), and two
test harnesses. The tech-stack doc names a **web client (PixiJS)** as the player-facing
client — **that client is not in this repo**. So "ship v1" implies either (a) building
the web client, or (b) deliberately scoping the first release to the terminal client.
This is the single biggest unstated decision; everything below assumes the server must
be production-grade regardless.

## Launch-gating (must resolve before a public v1)

| # | Gate | Where it's tracked |
|---|---|---|
| **G1** | **No persisted results or replays.** Completed games vanish (server **F4**); no leaderboards, match history, or replays. | `persistence-and-replays` change (supersedes F4). |
| **G2** | **No shippable player client in-repo.** The terminal client is for dev/agents; the web client is unbuilt. Decide scope (web client vs. terminal-only launch). | product decision (out of OpenSpec scope). |
| **G3** | **No deployment story.** No Dockerfile/compose, no TLS (the server speaks plain `ws://` — production needs `wss://` via a reverse proxy/terminator), no health/readiness endpoint, no graceful shutdown, no `DATABASE_URL` wiring (arrives with G1). | new ops work (suggest a `deployment` change). |
| **G4** | **§I compliance: invalid in-wave actions are silently dropped** (server **F1**) — decide error-reply vs. documented anti-leak silence, and implement. | `review-remediation` change. |

## Should-fix (robustness / correctness before scale)

| # | Item | Source |
|---|---|---|
| **S1** | Secret boundary isn't self-enforcing; no end-to-end no-leak scan over a full game's broadcast stream. | server **F3** → `review-remediation`. |
| **S2** | The **shipping** async game loop re-implements orchestration the tested sync engine has, and is only coarsely tested (drift risk). | server **F2** → `review-remediation`. |
| **S3** | Production `unwrap()`s on the async path; harden to `expect`/`entry`. | server **F5** → `review-remediation`. |
| **S4** | **Recall has no wire target** (and no `PickTarget`): a designed effect is non-functional end-to-end; the agent harness can't represent targeted effects either. | tui **T4**, agent **AH3** → protocol work. |
| **S5** | Admin API is safe-closed (rejects all requests when no tokens are set) but then unusable; provision `BP_ADMIN_TOKEN`/`BP_ADMIN_OBSERVER_TOKEN` and firewall the admin port (8081) in prod. | server (admin) — ops. |
| **S6** | CI runs fmt + clippy + **server-free** unit tests (`make test-unit` skips the in-process-server transport tests). The booting integration tests and a `--brain fallback` agent smoke game don't run in CI. | `.github/workflows/ci.yml`; agent **AH4**. |

## Acceptable for v1 / deliberately deferred

- **No benchmarks or load testing** → capacity-per-box is unknown. Fine for a soft
  launch; **measure before scaling**. Tracked as a v2 item ([roadmap](../roadmap.md)).
- **OTLP trace backend deferred** (no endpoint configured) — JSON logs + Prometheus
  metrics are sufficient for v1; the span pipeline is in place when a backend is added.
- **Anonymous sessions, no accounts/rating** — by design for v1 (Principle III); the
  seams for accounts/profiles/rating are documented in the roadmap.
- **room→group rename + persistent groups** — polish/feature, proposal-only
  (`group-model`); not a launch blocker.
- **Single-game lifecycle** in the client — acceptable for v1; revisited by `group-model`.

## Readiness matrix

| Dimension | Status | Note |
|---|---|---|
| Game correctness | ✅ Strong | Deep engine unit tests; deterministic; 300-game stress. |
| Server-authoritative / secrets | 🟡 Strong, unenforced | Discipline correct; add self-enforcement + e2e scan (S1) and fix F1 (G4). |
| Player client (shippable) | ❌ Missing | Web client unbuilt; terminal client is dev/agent (G2). |
| Persistence / replays | ❌ Not wired | G1 — `persistence-and-replays`. |
| Deployment / ops | ❌ Absent | No container, TLS, health, graceful shutdown (G3). |
| Observability | ✅ Good | Logs + metrics + admin span feed; OTLP backend deferred. |
| Config / secrets | 🟡 Partial | Content config validated at boot; admin tokens via env (S5); no DB URL yet. |
| CI | 🟡 Good, partial | fmt + clippy + unit tests; integration/agent smoke not in CI (S6). |
| Reconnection | ✅ Implemented | 60s grace; auto-pass; scoped snapshot on rejoin. |
| Balance validation | ✅ Tooling ready | `bot-harness` runs thousands of games; numbers still **[needs playtesting]**. |

## Recommended sequence to v1

1. **Decide G2** (web client vs. terminal-only) — gates the rest of the scope.
2. Land **`persistence-and-replays`** (G1) and **`review-remediation`** (G4, S1–S3).
3. Add the **deployment** layer (G3): TLS/reverse-proxy, health endpoint, graceful
   shutdown, container, migrations-on-boot, admin-token provisioning (S5).
4. Close the **Recall/target protocol gap** (S4) so all designed effects are playable.
5. Strengthen **CI** (S6): run the booting integration tests + a fallback agent smoke
   game.
6. Run a **balance pass** with the bot harness, then human playtests, before scaling.
7. Treat **benchmarks/load** (v2) as the pre-scale gate, not pre-launch.

None of these are deep redesigns — the architecture already has the seams. v1 is an
integration-and-operations push, not a rewrite.
