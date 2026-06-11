# boom2-ai-client — Tasks

## 1. Decision frames (protocol + server, with boom2-combat-core)

- [ ] 1.1 Add decision-frame message shapes to `protocol/`: pending decision kind (Brewer pick, draft, wave commit), deadline, and the enumerated legal action set incl. spell targets — coordinated with the combat-core protocol review.
- [ ] 1.2 Server: emit a decision frame to each player owing a decision, derived from the same validation logic (exactness both ways: everything listed validates, everything valid is listed).
- [ ] 1.3 Server: invalidate stale frames on phase advance; late submissions get an error and no state change.
- [ ] 1.4 Server tests: frame exactness (property-style: submit every enumerated action / probe non-enumerated ones), no-secrets audit of frame contents, stale-frame rejection.

## 2. Server in-process seam

- [ ] 2.1 Expose a headless in-process room boot with per-seat channels carrying **encoded wire frames** through the production codec (no domain objects cross).
- [ ] 2.2 Transport-parity test: same seeded scenario over in-process and WebSocket produces identical outcomes.

## 3. Client core (`clients/ai/`)

- [ ] 3.1 Create the `clients/ai` workspace member; dependency firewall: `protocol/` only, no `server/` types (enforce with a CI/dep check).
- [ ] 3.2 Secret-free view model rebuilt from received messages (no field can hold a boiling point, opponents' cards, or unrealized own-deck).
- [ ] 3.3 Connection layer: entry-handshake-first WebSocket transport + in-process frame-channel transport, one codec path.
- [ ] 3.4 `Brain` trait (`decide(view, frame) → action`) and the decision loop that only submits frame-enumerated actions.
- [ ] 3.5 Host decision policy per decision kind: `Scripted(value) | Delegated`.
- [ ] 3.6 Latency-budget race with fallback commit (late brain answers discarded); per-game fallback-rate accounting.
- [ ] 3.7 Core unit tests: secret boundary, legal-set adherence, policy routing, budget/fallback race.

## 4. Bot brain

- [ ] 4.1 Seeded RNG tree (root → game → seat); all bot randomness draws from the seat RNG.
- [ ] 4.2 Heuristics for the full v2 surface: ingredient-or-pass, spell casting + targeting (15 spells), Active priming, Brewer pick, Apothecary draft.
- [ ] 4.3 Archetypes (cautious, aggressive, political, random baseline) with distinct spell/fold/draft postures; epsilon blunder injection.
- [ ] 4.4 Tests: determinism (same seed → same actions), archetype divergence on aggregate stats, epsilon-0 purity.

## 5. Agent brain

- [ ] 5.1 Anthropic API client in Rust; tool schema derived from the decision frame; map responses back to legal actions (malformed → fallback + log).
- [ ] 5.2 Prompt assembly from the secret-free view + running transcript; persona and difficulty framing; transcript growth measured (compaction if needed).
- [ ] 5.3 Settings block: model, persona, difficulty, latency budget, fallback policy, auth, spend caps (per process + per game; cap reached → degrade to bot brain).
- [ ] 5.4 Tests: schema-from-frame correctness, no-secrets prompt audit, cap-degradation path, budget-miss fallback (mock API).
- [ ] 5.5 Probe script/binary: one isolated decision against a fixture frame (no server needed) to measure latency per model.

## 6. Harness mode (Principle IV reinstatement)

- [ ] 6.1 Batch runner over the in-process transport: matrix sample spec (cells × games), per-seat Brewer assignment and scripted deck-archetypes, root-seed reproducibility.
- [ ] 6.2 Stats + reports (markdown + diffable JSON): explosion rate, detonator distribution, per-Brewer/per-archetype win rates, spell fire rates (Peek economy), fold-to-safety, freeze frequency, game length, fallback rates; non-reproducible marking for agent seats.
- [ ] 6.3 Degenerate-strategy detection: random-baseline cells in samples; per-cell dominance visible in reports.
- [ ] 6.4 Agent-in-batch behind an explicit flag (default all-bot, zero Claude calls).
- [ ] 6.5 Validation tests: reproducibility, seed divergence, transport parity, 1000-game unattended run.

## 7. Seat-filler mode (product)

- [ ] 7.1 CLI/process: join by invite code or enqueue; multiple concurrent seats per process, each with its own brain + settings.
- [ ] 7.2 Delegated-by-default pre-game decisions (genuine Brewer pick + draft); persona display names and table presence.
- [ ] 7.3 Reconnection per protocol contract; clean per-seat exit on permanent failure.
- [ ] 7.4 End-to-end: four filler seats (mixed brains) auto-match and play a complete v2 game to GameOver over the real wire with zero missed deadlines.

## 8. Integration, CI, docs

- [ ] 8.1 Repoint `boom2-delivery` CI tasks at this client: pinned seeded harness sample in CI + an agent-brain smoke (mock or capped).
- [ ] 8.2 Fold the per-change harness tasks (combat-core 7.2/7.3, brewers 4.2, apothecary 5.2) into runs of this client; record first balance findings in docs/06_boom2.
- [ ] 8.3 README for `clients/ai/` (modes, brains, settings, firewall rules); update CLAUDE.md project structure and the Principle IV revival note to point at `clients/ai/`.
