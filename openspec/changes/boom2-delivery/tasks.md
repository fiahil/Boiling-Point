## 1. Fuller tests in CI (the gate)

- [ ] 1.1 Add transport/integration tests (boot an in-process server) to the CI workflow.
- [ ] 1.2 When the bot harness is revived for boom2 balance work (`archive/bot-harness/`,
  required by §IV before boom2 balance ships), add its seeded deterministic balance
  runs to CI.
- [ ] 1.3 Add the `clients/web/` build + Playwright visual tests (activates with `adopt-pixi-client`).
- [ ] 1.4 Make the full gate a required check on `main` before any deploy.

## 2. Deployment target & architecture

- [ ] 2.1 Choose the managed container host + managed Postgres; document the decision.
- [ ] 2.2 Containerize the single-binary monolith (runtime-injected config/secrets).
- [ ] 2.3 Set up TLS/WebSocket ingress and DB backups.
- [ ] 2.4 Stand up a staging environment mirroring production.

## 3. Continuous deployment pipeline

- [ ] 3.1 On green `main`: build + publish the server container and the `clients/web/` bundle.
- [ ] 3.2 Run database migrations as part of promotion.
- [ ] 3.3 Promote staging→prod; gate the whole pipeline behind the CI test gate.
- [ ] 3.4 Land the seeded benchmark regression runs in the pipeline.

## 4. Landing page (parallel)

- [ ] 4.1 Build the static landing page (what the game is, screenshots/trailer, play CTA).
- [ ] 4.2 Wire the "play now" CTA into the web client's create/join-room flow.
- [ ] 4.3 Deploy it as static content in front of `clients/web/`.
