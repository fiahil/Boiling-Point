# Tasks: boom2-observability

> Sequencing: groups 1–4 land with/immediately behind `boom2-combat-core` (same deploy window — D4).
> Groups 5–7 are gated on their content changes landing (D5); group 8 closes out docs and validation.
>
> **Status (2026-06-11):** the core phases (groups 1–4 and 8) are implemented.
> The unchecked tasks are all gated on changes that have not landed yet — 4.3 on
> `boom2-benchmarking` publishing the bench dashboard, 5.x/6.x/7.x on
> `boom2-brewers` / `boom2-apothecary` / `boom2-compounding`. Per D5's escape
> hatch, if this change archives first, the gated phases move to a small
> follow-up change.

## 1. Span schema v2 (blocks on combat-core's engine events existing)

- [x] 1.1 Define the v2 tree in `server/src/observability/span_schema.rs`: `commit` (Vote color), `spell.cast`, `resolve` (P, fatal-wave sort, detonator split), `depile` (boiling-point reveal), `score`; document `brewer.pick`/`draft` as planned; bump `SPAN_SCHEMA_VERSION` to 2.
- [x] 1.2 Re-point the engine span emitters at combat-core's event seams (wave loop, resolution, depile); remove v1-only emissions (`round.exploded`, `dominant_color`, reshuffle).
- [x] 1.3 Audit sensitive attributes for v2 (boiling point, pantry/spell hands, uncommitted plays, pot volatility, deck seeds) and mark them in the contract for the reveal.
- [x] 1.4 Unit tests: tree nesting per the contract, schema version stamped on `game`, sensitive-attribute markers present, no v1 span names emitted.

## 2. Balance metric definitions (`boom-balance-metrics`)

- [x] 2.1 Create `server/src/observability/balance_metrics.rs`: definition struct (id, formula, unit, optional `[needs playtesting]` target) + the core v2 set (boom rate, detonator distribution, fold/freeze rate, wave depth/duration, round/game duration, timeout rate, reconnection rate, fleet gauges).
- [x] 2.2 Seed targets from `docs/06_boom2/02_toward-a-v2-core.md` starting numbers; leave targets unset where the log has none.
- [x] 2.3 Re-point the Prometheus emitters at the definitions; delete v1 formula code; keep fleet metric identities unchanged.
- [x] 2.4 Unit tests: each definition evaluated over synthetic v2 event streams; fleet metrics unchanged across the cutover; no v1 metric ids emitted.
- [x] 2.5 Coordinate `boom2-benchmarking`: balance-study runner imports the module (one-line dependency note in its design; no structural change).

## 3. Projection re-key

- [x] 3.1 Re-key the open-span registry on the v2 tree (waves with commit/spell-cast leaves); verify ignore-unknown tolerance against planned-but-unimplemented spans.
- [x] 3.2 Swap the rolling aggregates to evaluate `boom-balance-metrics` definitions on span close.
- [x] 3.3 Update the replay buffer's preserved tree (commits/spell-casts, depile) for wave-by-wave replay.
- [x] 3.4 Update the room inspector reveal to the v2 hidden state (pantry/spell hands, pot volatility, active spell effects) and the no-open-round guard.
- [x] 3.5 Unit tests: registry keys, aggregate parity with a direct definition evaluation, replay ordering, reveal contents.

## 4. Command center + dashboard v2

- [x] 4.1 Anchor the admin UI as the command center: navigation hosting dashboard, inspector, replays alongside control actions, all behind admin auth (reads via projection, control via command API — unchanged channels).
- [x] 4.2 Rebuild the Grafana dashboard panels on the v2 metric ids with `[needs playtesting]` target bands; delete v1 panels (historical series left in storage).
- [ ] 4.3 Link (not host) the offline bench dashboard from the command center once `boom2-benchmarking` publishes it.
- [x] 4.4 Update the e2e "triggering a boom" scenario (server-side, headless) to assert the boom span and boom-rate metric appear for a deterministic seeded game.

## 5. Brewer panel (gated on `boom2-brewers`)

- [ ] 5.1 Add `brewer.pick` span emission per the planned contract entry (no version bump) and per-Brewer pick/win-rate definitions.
- [ ] 5.2 Add the per-Brewer dashboard panel inside the command center; unit-test the definitions over synthetic Brewer games.

## 6. Apothecary panel (gated on `boom2-apothecary`)

- [ ] 6.1 Add `draft` span emission (buckets taken are public; realized decks stay sensitive-marked) and bucket pick-rate / deck-archetype definitions.
- [ ] 6.2 Add the draft dashboard panel inside the command center; unit-test the definitions.

## 7. Compounding panel (gated on `boom2-compounding`)

- [ ] 7.1 Add compounding trigger attributes to `resolve`/`depile` spans and trigger-rate definitions.
- [ ] 7.2 Add the compounding panel inside the command center; unit-test the definitions.

## 8. Docs and validation (same change — constitution v2.1.1 docs-currency)

- [x] 8.1 Rewrite `docs/03_architecture/04_span-schema-contract.md` as the v2 contract (tree, attributes, sensitive markers, planned spans, versioning rules).
- [x] 8.2 Update `docs/03_architecture/02_server-infrastructure.md` and `docs/06_boom2/index.md` to reference the v2 observability surface and this change.
- [x] 8.3 Replay vocabulary: confirm the per-wave action log records the v2 action set (Vote color, pass/fold, spell casts with targets) and the engine-pinning path for v1 payloads is untouched.
- [x] 8.4 Full pass: `cargo fmt` + `clippy` + unit suite green; one seeded game inspected end-to-end through registry → aggregates → dashboard.

## 9. Popularity panel (command center; historical reads from persistence — added 2026-06-12)

- [x] 9.1 Add `fetch_popularity` to `server/src/persistence.rs` (games/players/new-players per UTC day over a clamped window, zero-filled series, window + lifetime totals; read-only, never decodes payloads) with a pure zero-fill helper unit-tested without a DB and a DB-gated `#[ignore]` round-trip test.
- [x] 9.2 Serve `GET /admin/stats/popularity?days=N` behind operator auth (`AdminState` gains the optional pool); no database ⇒ `available: false` instead of an error.
- [x] 9.3 Add the Popularity tab to the command center: games-per-day bar chart, unique-players-per-day chart with first-ever players highlighted, window/lifetime total cards, window selector — dependency-free CSS bars, no chart library.
- [x] 9.4 Spec: amend `admin-command-center`'s read-channels requirement (live reads = projection; historical reads = read-only persistence) and add the Popularity Stats requirement with the no-DB degradation scenario.
- [x] 9.5 Add the games-by-hour-of-day (UTC) histogram and the returning-player share (window players on 2+ distinct days) to the stats, the panel, and the spec; zero-fill helper unit-tested, DB-gated assertions extended.
