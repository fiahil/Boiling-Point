## Why

v1 ships **English only**, but the architecture is already language-neutral: the wire protocol carries **enums and IDs, never display strings** (Principle I), so localization is purely a **client-side** concern ([05_roadmap.md — Localization](../../../docs/05_roadmap.md)). v2 makes that pay off — and the v2-core content multiplies the surface (12 Brewers, 15 spells, 20 Apothecary buckets, each a name + one-line rule), so translation becomes part of content design, not a post-hoc pass.

## What Changes

- **Launch languages: EFIGS** — English (source) · French · Spanish · German · Italian — **plus Latin as a flavor locale** 🏺. All Latin-script, one plural model, one rendering path.
- **Client-side string tables, keyed by stable IDs.** One locale file per language (`en.json`, `fr.json`, …) mapping protocol enums (colors, spells, Brewers, buckets, modifiers, error codes) plus client UI keys → display strings. Plain agent-writable JSON, flat keys with `{placeholder}` interpolation.
- **One canonical source.** Locale files live in one shared place consumed by the web client (`clients/web/`), with a **CI check that every protocol enum variant has a key in every locale** — a new spell without its translations fails the build.
- **Server errors become codes, not prose.** `Error` carries an `ErrorCode` (+ structured params); clients render the localized string. The hardcoded English `message` is demoted to a **debug fallback**. This is the only protocol touch the feature needs.
- **Locale is a client preference** — defaulted from browser/system language, switchable in-client, persisted locally; **no account required**.
- **Flavor names are translated, not transliterated** (Nightshade → Belladone / Belladona); stable English identifiers stay in code, telemetry, and any revived harness.
- **Testing:** a **pseudo-locale** (expanded, accented) for layout stress; the Playwright visual suite runs per shipped locale (FR/ES/IT run ~15–30% longer, German compounds stretch words).

## Capabilities

### New Capabilities

- `localization` — the client-side ID-keyed locale-table system: the EFIGS + Latin set, the shared canonical source, the CI key-coverage gate, locale-as-client-preference, translated flavor names, and the pseudo-locale/visual-test story.

### Modified Capabilities

- `wire-protocol` — `Error` payloads gain a stable code + structured params as the localizable contract; the English `message` is demoted to a debug-only fallback (added as a new requirement to the protocol spec).

## Impact

- **Protocol:** the only touch — `Error` semantics (code + params canonical; English message debug-only). All other messages already carry enums/IDs.
- **Clients:** the web client (`clients/web/`) consumes the shared locale tables and renders from codes; an in-client locale switcher.
- **CI:** a key-coverage check (every protocol enum variant keyed in every locale) + per-locale Playwright visual runs + the pseudo-locale stress run (these fold into `boom2-delivery`'s CI layers).
- **Content/design tie-in:** the v2-core names + one-line rule texts must hold the §B.1 "instantly readable" bar **in every shipped language**, reviewed at design time.
