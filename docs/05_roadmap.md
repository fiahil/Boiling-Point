# Boiling Point — V2 / Post-Launch Roadmap

Features intentionally **out of v1 scope**, parked for a post-launch (v2) pass.
This is a product/platform roadmap — distinct from the *design* deferrals in
[02_game-design.md §18](02_game-design.md) (round objectives, spectator/replays,
modifier expansions), and distinct from the architecture notes in
[`03_architecture/`](03_architecture/).

Per the [constitution](../CLAUDE.md) Principle III (*Start Simple, Scale Later*),
v1 ships the simplest viable thing; these are the "scale later" items with the
seams designed in now.

---

## Identity, Rating & Skill-Based Matchmaking

The v1 stance (see [02_game-design.md §14](02_game-design.md)): **matchmaking yes**
(simple table-filling queue, FIFO/next-open-table, **not** skill-based), running
on **anonymous session tokens** with **no persistent accounts and no rating.**

Moved to v2:

| Feature | Why it's v2 | Dependency / note |
|---|---|---|
| **Persistent player accounts** | v1 uses anonymous per-session tokens; cross-game identity isn't needed to fill tables. | Either lightweight device-bound anonymous accounts or OAuth (Google/Discord). The server doc's "OAuth later" lives here. **This is the unlock** — rating and skill-based matchmaking both depend on it. |
| **Player rating** | No persistent identity in v1 → nothing to attach a rating to. | Free-for-all results need a **multiplayer rating model — TrueSkill / Weng-Lin**, *not* 2-player Elo — which is why the v1 schema carries no rating column. |
| **Skill-based matchmaking** | Requires ratings to match by skill. | v1's table-filling queue is the seam; v2 swaps the matching policy without changing the queue's shape. |

**Ordering:** persistent accounts first (the unlock), then rating, then
skill-based matchmaking on top of rating.

---

## Deployment, Delivery & CI/CD

Per Principle III (*Start Simple, Scale Later*), v1 is locally runnable behind a
CI **gate** — `.github/workflows/ci.yml` runs `fmt`, `clippy` (warnings-as-errors)
and the **unit** tests on every push to `main` and every PR — but nothing is
hosted, there's no deploy step, and there's no public web presence. v2 turns the
project into a continuously deployed, publicly reachable service.

| Feature | Why it's v2 | Dependency / note |
|---|---|---|
| **Landing page** | v1 distributes via invite links to people who already know the game; no acquisition surface is needed to validate the core loop. | Static marketing page (what the game is, screenshots/trailer, a "play now" → create/join room CTA). Sits alongside or in front of the PixiJS client (`clients/web/`), which is the "play" target. |
| **Fuller tests in CI** | v1 CI gates `fmt` + `clippy` + **unit** tests only; `make test-unit` deliberately skips the transport tests that boot an in-process server. | Extend CI to the full Principle II gate (v2.0.0): the **transport/integration** tests, and the **web client** build + Playwright visual tests once the Pixi client lands. A seeded **bot-harness smoke** (completion + determinism only) rejoins the gate when the archived harness is revived (required before boom2 balance ships — §IV); balance *metrics* stay observational in `boom2-benchmarking`. |
| **Continuous deployment pipeline** | v1 has a CI gate but no deploy step — releases are manual/local. | CD layered on the existing CI: build the release server binary and the `clients/web/` bundle, sync to the box, run DB migrations, and restart on green `main`. Gated behind the fuller test suite above. |
| **Deployment architecture & target** | v1 runs as a single local binary + local Postgres; nothing is hosted. | **Decided** (change `boom2-delivery`): a bare-metal Dedibox, no containers — the monolith as a systemd service, Postgres on the same host with nightly off-site backups, and Caddy as the sole public ingress (automatic TLS, `/ws` WebSocket proxy, static `file_server` for the landing page and the web-client bundle). Staging is the developer's localhost. The single-server stance is the seam; horizontal scaling stays out of v2. |

**Benchmarks** fold into this work: the benchmarking suite (change
`boom2-benchmarking` — see [Server benchmarks](#other-post-v1-candidates) below)
rides this pipeline. Seeded criterion runs execute per merge to `main` and the
bench dashboard republishes — all *observational* (tracked over time, never
gating); balance studies stay on-demand, outside CI.

**Ordering:** fuller tests in CI first (they gate everything), then pick the
deployment target/architecture, then the CD pipeline on top, with the landing
page in parallel.

---

## Localization (i18n)

v1 ships **English only**, but the architecture is already language-neutral and v2
makes that pay off: the wire protocol carries **enums and IDs, never display
strings** (Principle I — the server sends state, not prose), so localization is
purely a **client-side concern**. The server never knows what language a player
reads; a French, a Spanish and an English player share a table with zero server
involvement, and emotes are already language-neutral by design
([02_game-design.md — Table Talk](02_game-design.md)).

**Launch languages: English, French, Spanish, German, Italian** (the classic
**EFIGS** set) — **plus Latin as a flavor locale.**

| Tier | Languages | Note |
|---|---|---|
| **v2 launch** | English (source) · French · Spanish · German · Italian | EN as the source-of-truth strings; ES kept neutral (readable for both es-ES and es-419). All Latin script, one plural model — EFIGS rides a single rendering path. |
| **Flavor locale** | Latin 🏺 | A thematic bonus for a potion-brewing game — grimoires, wards and hexes *want* to read in Latin, and half the herb names already are (*Belladonna*, *Helleborus*). Same file format, same CI gate, zero engine cost; translations reviewed for *readability and fun*, not classical purity. No support promise. |
| Next (cheap) | Portuguese (BR) | Latin script, similar plural rules — adding it is "add one file". Driven by demand, not committed. |
| Deferred | CJK (ja / ko / zh) · RTL (ar / he) | Real engine cost, not just translation: CJK needs font loading / glyph handling in the Pixi canvas, RTL needs layout mirroring. Not until a market case exists. |

**The design (Principle III — simplest viable, seams documented):**

- **Client-side string tables, keyed by stable IDs.** One locale file per language
  (`en.json`, `fr.json`, `es.json`) mapping protocol enums (colors, effects/spells,
  Brewers, buckets, modifiers, error codes) plus client UI keys → display strings.
  Plain agent-writable JSON (Principle II), flat keys with `{placeholder}`
  interpolation. *Rejected (deferred) alternative:* Fluent / ICU MessageFormat —
  full plural/gender grammar machinery isn't justified while every string is a
  short label or a one-sentence rule text; revisit if a string genuinely needs it.
- **One canonical source.** Same pattern as the wire types (TS generated from the
  Rust `protocol` crate so the client cannot drift): locale files live in one
  shared place, consumed by `clients/web/`, with a CI check that every protocol
  enum variant has a key in **every** locale — a new spell that lands without its
  translations fails the build.
- **Server errors become codes, not prose.** Today `Error` carries an `ErrorCode`
  *plus* a hardcoded English `message` (`server/src/session.rs`). v2 clients
  render the localized string from the code (+ structured params where needed);
  the English `message` is demoted to a debug fallback. This is the only protocol
  touch the whole feature needs.
- **Locale is a client preference.** Default from the browser/system language,
  in-client switcher, persisted locally — no account required (fits the
  anonymous-session ethos, [02 §14](02_game-design.md)).
- **Flavor names are translated, not transliterated.** Brewers, buckets and spells
  read in the player's language (Nightshade → Belladone / Belladona); the stable
  English identifiers remain in code, telemetry, and any revived harness.

**Testing (Principle II):** French, Spanish and Italian run ~15–30% longer than
English, and German compounds can stretch a single *word* past a button's width —
layouts must tolerate both. Add a **pseudo-locale** (expanded, accented strings)
for layout stress, and run the Playwright visual suite per shipped locale.

**The v2-core tie-in:** the core rework
([06_boom2/02_toward-a-v2-core.md](06_boom2/02_toward-a-v2-core.md)) multiplies the translation
surface — 12 Brewers, 15 spells, and 20 Apothecary buckets, each with a name and a
one-line rule text. The §B.1 bar (*one sentence, instantly readable*) must hold in
**every shipped language**, so translation is part of content design review, not a
post-hoc pass.

---

## Other Post-V1 Candidates

These also sit beyond v1. (Some overlap with the design-side deferrals in
02_game-design.md §18 — cross-referenced, not duplicated.)

- **Round objectives** — per-round scoring tweaks to nudge specific game-theory
  dilemmas. The modifier stack covers round-to-round variety in v1; revisit if
  rounds feel samey. *(Design deferral — see 02_game-design.md §18.)*
- **Spectator mode & replays** — needs an append-only event log; v1 persists
  post-game only. *(Design deferral — see 02_game-design.md §18.)*
- **Cauldron-modifier expansions** — more modifiers beyond the launch 6, once the
  stacking system is validated. *(Design deferral — see 02_game-design.md §18.)*
- **OAuth / cross-device identity** — folds into the persistent-accounts work
  above.
- **Player profiles** — per-player career stats, history, and identity surfaced
  on top of persisted match results. Depends on **persistent accounts** (above):
  the v1 *persistence-and-replays* work stores match results and replays but
  attaches **no** profile or cross-game identity. Moved here out of the v1
  persistence scope.
- **Server benchmarks** — now scoped in change **`boom2-benchmarking`** as one
  instrument of the **benchmarking suite** (the other is the on-demand **balance
  study**, where the revived bot harness fits; both read from one self-contained
  HTML dashboard, all observational — benchmarks measure, tests gate):
  - **Engine micro-benchmarks** (`criterion`): hot paths in the round engine —
    deck realization, wave resolution, explosion resolution/depile, modifier
    stacking — run per merge to `main`, read as *trends* (observed rerun noise
    is 6–12%, so single-run deltas are noise).
  - **WebSocket load harness**: many concurrent rooms driven over the real wire
    (the AI client's WebSocket transport — `clients/ai`), measuring tick latency,
    broadcast fan-out cost, and memory per room, against target throughput/latency
    budgets. **Deferred** out of `boom2-benchmarking`; the suite's report and
    dashboard conventions are its seam.

---

## What v1 Deliberately Keeps Simple (the seams)

So the v2 work is a swap, not a rewrite:

- **Auth:** anonymous session token now → persistent account later (additive).
- **Matchmaking:** table-filling queue now → skill-based policy later (same queue).
- **Persistence:** post-game results now → append-only event log later (enables
  replays/spectating).
- **Delivery:** CI gate now (`fmt`/`clippy`/unit) → fuller test layers + CD later
  (additive; the Makefile targets are the seam — the archived harnesses are
  revivable for the balance layers).
- **Text:** hardcoded English strings now → ID-keyed locale tables later (the
  wire is already language-neutral — the protocol enums *are* the seam).
- **Hosting:** single local binary + local Postgres now → managed container +
  managed Postgres later (same monolith, one container).
