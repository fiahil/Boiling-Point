# Boiling Point — Agent Harness Review

A review of the Claude-as-player harness (`agent-harness/`) — the Layer-2 testing
harness from the constitution's §II — against the [constitution](../../CLAUDE.md) and
the [game design](../game-design.md). It is a **Node/TypeScript** package (not a cargo
crate) so it can use the Claude Agent SDK and bill against a Claude subscription.

Reviewed 2026-06-02 against `main`. `npm run typecheck` is clean (strict TS) and the
24 pure-logic unit tests pass; four `--brain fallback` bots auto-match and play to
`GameOver`, and the Claude brain drives real decisions via in-process MCP tools.

**Overall:** a thoughtfully layered harness with two standout properties — a
**secret-by-construction view model with a runtime leak assertion on the live path**,
and **difficulty implemented as the granted capability set**. The main risks are
operational, not structural: a hand-mirrored protocol that can drift from the Rust
crate, and per-decision latency that exceeds the sub-wave timer (handled by design via
fallback, but worth understanding).

---

## 1. Architecture

One agent seat, orchestrated across three layers (`src/cli.ts` → `src/runner.ts`):

- **Net** (`net/connection.ts`, `view-model.ts`, `secret-boundary.ts`,
  `wave-lifecycle.ts`): a `ws` client that performs the entry handshake
  (`CreateRoom`/`JoinRoom`/`EnqueueMatch` + `protocol_version`), decodes MessagePack,
  folds messages into a player-visible view model, and routes wave open/resolve events.
- **Agent** (`agent/session.ts`, `tools.ts`, `difficulty.ts`, `context.ts`,
  `prompt.ts`, `actions.ts`): one **persistent** Agent SDK `query()` per game (warm
  context across waves), driving moves through an in-process MCP tool server.
- **Timeliness & personas** (`timeliness/fallback.ts`, `personas/*`): a safe-leaning
  heuristic for when the model misses a deadline, plus optional persona biases, preset
  emotes, and epsilon blunder injection.

The wave cycle implements design D4 — *deliberate at the previous wave's resolve,
commit at wave-open, fall back on overrun* — so the table never stalls on model
latency.

## 2. Secret boundary — strong (and enforced at runtime)

The view model has **no field for opponents' hands or the draw deck**; the only secret
is the boiling point, which enters solely through `discloseBoilingPoint(vm, value,
source)` with `source ∈ {peek, explosion}` (`net/secret-boundary.ts`). After **every**
inbound message the runner calls `assertNoSecretLeak(vm)` (`runner.ts`), which throws
if a boiling point is present without a legitimate source or with a forged one:

```ts
if (hasValue !== hasSource) throw new Error("secret-boundary violation: …");
if (hasSource && !VALID_SOURCES.has(boilingPointSource)) throw new Error("… illegal source …");
```

This is the bot-harness's "secret-by-construction" discipline, but with an added
**runtime tripwire on the production path** — a stronger posture than the server's own
(unused) routing rail (cf. server-review F3). `test/secret-boundary.test.ts` and
`test/context-gating.test.ts` verify both the gate and that an agent without the
`reveal_history` capability never receives past card identities in its turn context.

## 3. Difficulty as capability — strong

Difficulty **is** the granted tool set (`agent/difficulty.ts`): `easy` gets the action
tools only; `hard` adds `reveal_history` (card-counting within the current shuffle
epoch). Withholding the capability removes it at two levels — the SDK won't expose the
tool, and `context.ts` excludes reveal history / pot identities from the turn context
(it even keeps an explicit forbidden-key list). Model selection follows
(`easy`→Haiku, `hard`→Opus). Persona (playstyle) and difficulty (capability) are
cleanly orthogonal axes.

## 4. Auth safety — good

`src/auth.ts` neutralizes `ANTHROPIC_API_KEY`/`ANTHROPIC_AUTH_TOKEN` by default so play
bills the **subscription** (Claude Code login / `CLAUDE_CODE_OAUTH_TOKEN`), not
pay-as-you-go API credits; `BP_ALLOW_API_KEY=1` is the explicit opt-in, and the choice
is logged to stderr. Called only for the `claude` brain; `fallback` needs no auth.
This prevents a stray exported key from silently draining credits — a real, easily-made
mistake handled well.

## 5. Findings

### AH1 — Hand-mirrored protocol can drift from the Rust crate *(maintainability, medium)*

`src/protocol/messages.ts` is a **hand-written mirror** of the Rust `protocol/` crate;
`scripts/gen-protocol-types.ts` is a `ts-rs` generation **seam** that isn't active yet
(the Rust crate doesn't derive `ts-rs`). Until generation is wired, every protocol
change must be mirrored by hand or the harness silently desyncs (decode failures or,
worse, subtle field drift). The MessagePack-only wire and the UUID-bytes→hex
canonicalization in `codec.ts` are correct but add surface that a regenerator would
keep honest. **Recommend:** add `#[ts(export)]` to the Rust crate and make
`npm run gen:protocol` authoritative; treat `messages.ts` as generated.

### AH2 — Decision latency exceeds the sub-wave timer; correctness leans on fallback *(operational, medium)*

Documented in the README: per-decision latency is ~15s (model/rate-limit bound), while
sub-waves are 10s. The design absorbs this — deliberate early, commit at open, fall
back on overrun — so the table flows, but it means a `hard` Claude agent frequently
plays the **heuristic** move, not a reasoned one, on fast waves. For meaningful
hard-AI playtests, relax the wave timers (content config) or accept that fallback
dominates. `--brain fallback` is the right choice for UI/flow testing. Not a bug, but
set expectations.

### AH3 — Effect coverage gaps mirror protocol gaps *(coverage, low/medium)*

The harness exposes commit/pass/lock-in/emote and `reveal_history`, but effect-target
actions (e.g. Recall's target, any future `PickTarget`) aren't representable — matching
the wire gap noted in the TUI review (T4). As those effects gain wire support, the
tool surface and `actions.ts` legality checks need to grow with them.

### AH4 — No end-to-end / integration test in CI *(coverage, low)*

The 24 tests are pure-logic (view model, secret boundary, gating, difficulty, fallback,
blunder, emotes, wave lifecycle) — excellent for the units, but the Agent SDK
integration and connection/version-mismatch recovery are verified only manually
(`scripts/probe-agent.ts`, live runs). A scripted `--brain fallback` four-bot game in
CI (no Claude auth needed) would guard the net/runner path cheaply.

### AH5 — Minor fragility *(low)*

- `ensureSession()` caches `systemPrompt`/`allowedTools` once per game (correct today,
  not enforced); persona/difficulty are fixed per seat.
- `personaBiasedFallback()` assumes `vm.self.hand`/`vm.wave` exist; defensive guards
  would harden it.
- The lock-in delay is a hardcoded 150 ms for rate-limit spacing — fine, but coupled to
  the server's 100 ms `RATE_LIMIT` only by convention.

## 6. Recommendations

1. **Make protocol types generated (AH1)** — the single highest-leverage durability
   fix; eliminates the hand-mirror drift class entirely.
2. **Add a CI smoke game (AH4)** — four `--brain fallback` bots to `GameOver`, no auth,
   guarding the live net/runner path.
3. **Document the latency/fallback trade-off prominently (AH2)** — already in the
   README; reinforce in playtest guidance so "hard" results aren't over-read.
4. Grow the tool/action surface in lockstep with wire support for targeted effects (AH3).

## 7. Constitution compliance

| Principle | Verdict | Notes |
|---|---|---|
| **I. Server-Authoritative** | Strong | Receives only player-permitted data; runtime secret-leak assertion on the live path; no field can hold an undisclosed secret. |
| **II. Agent-Driven** | Strong | This *is* the Layer-2 harness; structured JSON/MCP over WebSocket, no vision needed; difficulty/persona make it a tunable opponent. |
| **III. Start Simple** | Good | One persistent session per game; heuristic fallback; hand-mirror protocol (simple now, generate later — AH1). |
| **IV. Playtest-Driven** | Strong | Purpose-built to let one person playtest with tunable opponents; epsilon/persona/difficulty are explicit knobs. |
