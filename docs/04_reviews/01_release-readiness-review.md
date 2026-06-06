# Boiling Point — Release-Readiness Review

A cross-cutting assessment of what stands between today's codebase and a **public v1
launch**, synthesizing the component reviews ([server](02_server-review.md),
[tui-client](03_tui-client-review.md), [agent-harness](04_agent-harness-review.md)) against
the [constitution](../../CLAUDE.md), [game design](../02_game-design.md), and
[roadmap](../05_roadmap.md).

Assessed 2026-06-02 against `main`; **refreshed 2026-06-05** after `review-remediation`,
`group-model`, `group-fill-and-standings`, `converge-game-loops`, and
`persistence-and-replays` landed. **Verdict: production-grade server, still no
shippable player client.** The server-side launch debt the first pass flagged is
largely cleared — persistence + timeless replays are wired (G1), invalid in-wave
actions error correctly (G4/F1), the two game loops are converged onto one tested core
(S2/F2), the secret rail is self-enforcing with an end-to-end scan (S1/F3), and the
async-path `unwrap()`s are hardened (S3/F5). The remaining launch gates are a
**player-facing client** (the PixiJS web client is proposed in `adopt-pixi-client` but
unbuilt) and a **deployment/ops story**; the Recall-target protocol gap and CI coverage
are the main should-fixes left.

---

## What "release" means here

The repo contains the server, a **terminal** client (a dev/agent renderer), and two
test harnesses. The tech-stack doc names a **web client (PixiJS)** as the player-facing
client — **that client is not in this repo**. So "ship v1" implies either (a) building
the web client, or (b) deliberately scoping the first release to the terminal client.
This is the single biggest unstated decision; everything below assumes the server must
be production-grade regardless.

## Launch-gating (must resolve before a public v1)

| # | Gate | Status / where it's tracked |
|---|---|---|
| **G1** | ~~No persisted results or replays.~~ | ✅ **Resolved** by `persistence-and-replays`: results + timeless replays persist in one post-game write; `DATABASE_URL` wires the pool; no-DB degrades cleanly. |
| **G2** | **No shippable player client in-repo.** The terminal client is for dev/agents; the web client is unbuilt. The constitution now adopts **PixiJS (web + mobile via Capacitor)** and the `adopt-pixi-client` change scopes it — but it is not yet built. | `adopt-pixi-client` change (in progress) + build work. |
| **G3** | **No deployment story.** No Dockerfile/compose, no TLS (the server speaks plain `ws://` — production needs `wss://` via a reverse proxy/terminator), no health/readiness endpoint, no graceful shutdown. (`DATABASE_URL` wiring now exists — arrived with G1.) | new ops work (suggest a `deployment` change). |
| **G4** | ~~§I: invalid in-wave actions are silently dropped~~ (server **F1**). | ✅ **Resolved** by `review-remediation`: in-wave invalid actions reply `NotYourCard`/`LockedOut`/`InvalidEmote`/`WrongPhase` with no state change. |

## Should-fix (robustness / correctness before scale)

| # | Item | Status / source |
|---|---|---|
| **S1** | ~~Secret boundary isn't self-enforcing; no end-to-end no-leak scan.~~ | ✅ **Resolved** (server **F3** → `review-remediation`): every send routes through `Outbound`; an e2e test scans a full game's frames for leaks. |
| **S2** | ~~The shipping async loop re-implements orchestration the tested engine has.~~ | ✅ **Resolved** (server **F2** → `converge-game-loops`): `run_game` drives the tested `Game` core; a sync==async parity test pins their scores. |
| **S3** | ~~Production `unwrap()`s on the async path.~~ | ✅ **Resolved** (server **F5** → `review-remediation`): hardened to `expect`/`entry`. |
| **S4** | **Recall has no wire target** (and no `PickTarget`): a designed effect is not fully playable end-to-end; the agent harness can't represent targeted effects either. (`converge-game-loops` D3 surfaces an auto-chosen recall to its owner, but not target *selection*.) | tui **T4**, agent **AH3** → protocol work (still open). |
| **S5** | Admin API is safe-closed (rejects all requests when no tokens are set) but then unusable; provision `BP_ADMIN_TOKEN`/`BP_ADMIN_OBSERVER_TOKEN` and firewall the admin port in prod. | server (admin) — ops. |
| **S6** | CI runs fmt + clippy + **server-free** unit tests (`make test-unit` skips the in-process-server transport tests). The booting integration tests and a `--brain fallback` agent smoke game don't run in CI. | `.github/workflows/ci.yml`; agent **AH4**. |

## Acceptable for v1 / deliberately deferred

- **No benchmarks or load testing** → capacity-per-box is unknown. Fine for a soft
  launch; **measure before scaling**. Tracked as a v2 item ([roadmap](../05_roadmap.md)).
- **OTLP trace backend deferred** (no endpoint configured) — JSON logs + Prometheus
  metrics are sufficient for v1; the span pipeline is in place when a backend is added.
- **Anonymous sessions, no accounts/rating** — by design for v1 (Principle III); the
  seams for accounts/profiles/rating are documented in the roadmap.
- **room→group rename + persistent groups + standings + matchmaking fill** — **landed**
  (`group-model`, `group-fill-and-standings`); groups now persist across games, members
  vs. matchmaking guests are distinguished, and a live in-memory standings tally is kept.
- **Single-game lifecycle** in the client — **resolved** by `group-model`'s play-again:
  the table stays together across games (tui **T5**).

## Readiness matrix

| Dimension | Status | Note |
|---|---|---|
| Game correctness | ✅ Strong | Deep engine unit tests; deterministic; 300-game stress; one orchestration core (F2) with a sync==async parity test. |
| Server-authoritative / secrets | ✅ Strong, enforced | Self-enforcing `Outbound` rail + e2e leak scan (S1/F3); invalid in-wave actions error (G4/F1). |
| Player client (shippable) | ❌ Missing | Web client (PixiJS) adopted in the constitution and scoped by `adopt-pixi-client`, but unbuilt; terminal client is dev/agent (G2). |
| Persistence / replays | ✅ Wired | G1 resolved — results + timeless replays in one post-game write; clean no-op without a DB. |
| Deployment / ops | ❌ Absent | No container, TLS, health, graceful shutdown (G3). |
| Observability | ✅ Good | Logs + metrics + admin span projection; OTLP backend deferred. |
| Config / secrets | 🟡 Partial | Content config validated at boot; `DATABASE_URL` wires persistence; admin tokens via env (S5). |
| CI | 🟡 Good, partial | fmt + clippy + unit tests; integration/agent smoke not in CI (S6). |
| Reconnection | ✅ Implemented | grace timeout; auto-pass; scoped snapshot on rejoin. |
| Balance validation | ✅ Tooling ready | `bot-harness` runs thousands of games; numbers still **[needs playtesting]**. |

## Recommended sequence to v1

1. ~~Land `persistence-and-replays` (G1) and `review-remediation` (G4, S1–S3)~~ —
   **done**, plus `converge-game-loops` (S2/F2) and `group-model` + `group-fill-and-standings`.
2. **Build the player client** (G2): the constitution adopts PixiJS (web + mobile via
   Capacitor); execute `adopt-pixi-client`. This is now the single biggest launch gate.
3. Add the **deployment** layer (G3): TLS/reverse-proxy, health endpoint, graceful
   shutdown, container, migrations-on-boot (the DB pool/migrations already run at boot),
   admin-token provisioning (S5).
4. Close the **Recall/target protocol gap** (S4) so all designed effects are fully
   playable end to end.
5. Strengthen **CI** (S6): run the booting integration tests + a fallback agent smoke
   game.
6. Run a **balance pass** with the bot harness, then human playtests, before scaling.
7. Treat **benchmarks/load** (v2) as the pre-scale gate, not pre-launch.

None of these are deep redesigns — the architecture already has the seams, and the
server-side launch debt is now largely cleared. v1 is now mostly a **client-build and
operations** push.
