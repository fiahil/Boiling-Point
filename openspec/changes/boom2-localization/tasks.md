## 1. Locale-table system

- [ ] 1.1 Define the shared locale-file format (flat JSON, stable keys, `{placeholder}` interpolation) and its single canonical location.
- [ ] 1.2 Implement locale loading/lookup in `web-client/` and `tui-client/` from the shared source.
- [ ] 1.3 Implement locale-as-client-preference: default from browser/system, in-client switcher, local persistence, no account.

## 2. The one protocol touch

- [ ] 2.1 Change `Error` to carry `ErrorCode` + structured params as canonical; demote the English `message` to a debug-only fallback (`server/src/session.rs`).
- [ ] 2.2 Regenerate client wire types; render errors from the code (+ params) via the active locale.

## 3. Content & translations

- [ ] 3.1 Author `en.json` (source) covering every protocol enum (colors, spells, Brewers, buckets, modifiers, error codes) + client UI keys.
- [ ] 3.2 Translate FR / ES / DE / IT (ES kept es-ES/es-419-neutral) and the Latin flavor locale; translate flavor names, keep stable English identifiers in code/telemetry.

## 4. CI gates (fold into boom2-delivery's layers)

- [ ] 4.1 Add the key-coverage check: every protocol enum variant keyed in every shipped locale, else fail.
- [ ] 4.2 Add a pseudo-locale (expanded, accented) build for layout stress.
- [ ] 4.3 Run the Playwright visual suite per shipped locale.

## 5. Process

- [ ] 5.1 Make per-language readability part of v2-core content review (the §B.1 "one sentence, instantly readable" bar holds in every shipped language).
