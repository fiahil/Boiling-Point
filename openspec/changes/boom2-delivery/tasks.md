## 1. Fuller tests in CI (the gate)

- [ ] 1.1 Add transport/integration tests (boot an in-process server) to the CI workflow.
- [ ] 1.2 When the bot harness is revived for boom2 balance work (`archive/bot-harness/`,
  required by §IV before boom2 balance ships), add its seeded deterministic smoke run
  to CI (completion + determinism only — balance metrics stay observational in
  `boom2-benchmarking`).
- [ ] 1.3 Add the `clients/web/` build + Playwright visual tests (activates with `adopt-pixi-client`).
- [ ] 1.4 Make the full gate a required check on `main` before any deploy.

## 2. Deployment target & architecture

- [ ] 2.1 Provision the Dedibox: hardened SSH, firewall open on 80/443 + SSH only, Caddy and
  PostgreSQL installed.
- [ ] 2.2 Run the monolith as a systemd service, localhost-bound (`:8080` game, `:8081` admin),
  secrets via a root-only environment file.
- [ ] 2.3 Write the Caddyfile: automatic TLS, `/ws` reverse-proxy to `localhost:8080`,
  `file_server` for the landing page (`/`) and the `clients/web/` bundle (`/play`).
- [ ] 2.4 Nightly `pg_dump` shipped off-site to object storage; exercise a restore against a
  scratch database.
- [ ] 2.5 Keep admin (`:8081`) + Grafana localhost-only; document SSH-tunnel access in
  `ops/README.md`.

## 3. Continuous deployment pipeline

- [ ] 3.1 On green `main`: build the release server binary and the `clients/web/` bundle in CI.
- [ ] 3.2 Deploy step: sync binary + bundle to the box, run DB migrations, restart the systemd
  service; gate the whole pipeline behind the CI test gate (staging is localhost — green
  `main` goes straight to prod).
- [ ] 3.3 Give the server a graceful shutdown/drain on restart — or explicitly document the
  accepted in-flight-game interruption — so routine deploys aren't silent match-killers.
- [ ] 3.4 Run the `boom2-benchmarking` per-merge jobs in the pipeline (criterion bench +
  history append + dashboard republish) — observational, outside the gate.

## 4. Landing page (parallel)

- [ ] 4.1 Build the static landing page (what the game is, screenshots/trailer, play CTA).
- [ ] 4.2 Wire the "play now" CTA into the web client's create/join-room flow.
- [ ] 4.3 Deploy it as static content in front of `clients/web/`.
