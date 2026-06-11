## Context

The wire is already language-neutral (enums/IDs, never prose — Principle I), so localization is a client concern with one small protocol touch ([05_roadmap.md — Localization](../../../docs/05_roadmap.md)). This change ships EFIGS + a Latin flavor locale, a shared locale-table system for both clients, and a CI coverage gate, timed to the v2-core content surface.

## Goals / Non-Goals

**Goals**
- Client-side, ID-keyed locale tables; EFIGS + Latin; one canonical source for both clients.
- CI key-coverage (every enum keyed in every shipped locale); locale as a no-account client preference.
- Translated flavor names with stable English identifiers; pseudo-locale + per-locale visual tests.
- The one protocol touch: `Error` = code + params, English demoted to debug fallback.

**Non-Goals**
- Fluent / ICU MessageFormat (full plural/gender grammar) — not justified while strings are short labels / one-sentence rules; revisit if a string needs it.
- CJK / RTL — real engine cost (font/glyph loading, layout mirroring); deferred until a market case exists.
- Portuguese (BR) — cheap to add later ("add one file"), demand-driven, not committed here.

## Decisions

### D1: Flat JSON string tables, not ICU

Every string is a short label or a one-sentence rule, so flat keyed JSON with `{placeholder}` interpolation suffices and stays agent-writable (Principle II). *Alternative deferred:* Fluent/ICU — revisit only if a string genuinely needs plural/gender machinery.

### D2: One canonical source, generated-types discipline

Locale files live in one shared place consumed by both clients, mirroring the wire-typegen "single source so clients can't drift" pattern. A CI check enforces full enum coverage per locale. *Alternative rejected:* per-client string tables — drift and double-maintenance.

### D3: Errors are codes, not prose — the only protocol touch

`Error` carries a code + structured params (canonical/localizable); the English `message` becomes a debug fallback. Everything else already uses enums/IDs. *Alternative rejected:* server-side localization — violates §I (the server would need to know each player's language) and bloats the wire.

### D4: EFIGS rides one path; Latin is a free flavor win

All six launch locales are Latin-script with one plural model, so they share a single rendering path at near-zero engine cost; Latin is a thematic bonus for a potion game, reviewed for fun not purity. CJK/RTL are deferred because they carry *engine* cost, not just translation.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | The server keeps sending enums/IDs and never display prose; language is resolved entirely client-side. The lone protocol change (Error code+params) *reinforces* §I by removing prose from the wire. |
| **II — Agent-driven** | Locale files are plain JSON (agent-writable); the CI coverage gate and pseudo-locale/visual suite are automated, source-defined checks. |
| **III — Start simple** | Flat JSON over ICU; EFIGS + Latin on **one Latin-script rendering path**; CJK/RTL deferred for real engine cost. **Rejected simpler alternative:** stay English-only — but the language-neutral wire is exactly the seam v2 is meant to cash in, and the v2-core content is the natural translation moment. |
| **IV — Playtest-driven** | Not a balance feature, but readability is validated: the §B.1 "one sentence, instantly readable" bar must hold per language (content-review gate), and the pseudo-locale/per-locale visual runs catch layout breakage. |

## Risks / Migration

- **Coupling to v2-core content:** the 12 Brewers / 15 spells / 20 buckets must be translated as they're designed (§B.1 bar per language), not after — this is a process risk more than a code one.
- **Length variance & German compounds** can break layouts; the pseudo-locale + per-locale Playwright runs are the guard (these land in `boom2-delivery`'s CI layers).
- **Latin scope creep:** explicitly readability/fun only, no support promise, to keep it a free win.
