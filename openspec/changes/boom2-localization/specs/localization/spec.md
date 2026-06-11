## ADDED Requirements

### Requirement: Client-Side ID-Keyed Locale Tables

Display strings SHALL live in **client-side locale files**, one per language, mapping **stable IDs** (protocol enums — colors, spells, Brewers, buckets, modifiers, error codes — plus client UI keys) to display strings. Files SHALL be plain agent-writable JSON with flat keys and `{placeholder}` interpolation. The server SHALL NOT send display prose.

#### Scenario: A spell name renders from its ID

- **WHEN** the client displays a spell the server identified by its stable enum
- **THEN** it looks the enum up in the active locale file and renders that language's string

#### Scenario: Players in different languages share a table

- **WHEN** a French, a Spanish, and an English player are in the same game
- **THEN** each client renders its own locale from the same enum/ID state, with no server involvement in language

### Requirement: Launch Locale Set — EFIGS Plus Latin

The launch set SHALL be **English (source), French, Spanish, German, Italian**, plus **Latin** as a flavor locale. All are Latin-script and ride a single rendering/plural path. Latin carries no support promise and is reviewed for readability/fun, not classical purity.

#### Scenario: Each launch locale resolves

- **WHEN** a player selects any of EN/FR/ES/DE/IT or Latin
- **THEN** the client renders fully from that locale file with no missing-key fallbacks for shipped content

### Requirement: One Canonical Source For Both Clients, Enforced In CI

Locale files SHALL live in **one shared place** consumed by both `web-client/` and `tui-client/`. CI SHALL fail if **any** protocol enum variant lacks a key in **any** shipped locale.

#### Scenario: A new spell without translations fails CI

- **WHEN** a new spell enum lands without keys in every shipped locale
- **THEN** the CI key-coverage check fails the build

### Requirement: Locale Is A Client Preference, No Account Required

The active locale SHALL default from the browser/system language, be switchable in-client, and persist locally — requiring **no account**.

#### Scenario: Locale persists locally without an account

- **WHEN** an anonymous player switches locale and returns later on the same client
- **THEN** their chosen locale is restored without any account

### Requirement: Flavor Names Are Translated, Not Transliterated

Brewer, bucket, and spell names SHALL be **translated** into each language while the **stable English identifiers** remain in code, telemetry, and the harnesses.

#### Scenario: A flavor name localizes while its identifier is stable

- **WHEN** the Nightshade bucket is shown in French
- **THEN** the player sees "Belladone" while code/telemetry still reference the stable `Nightshade` identifier

### Requirement: Layout Tolerates Localized Lengths

The UI SHALL tolerate localized length variance (FR/ES/IT ~15–30% longer than EN; German compounds can exceed a single button's width). A **pseudo-locale** (expanded, accented) SHALL exist for layout stress, and the visual test suite SHALL run per shipped locale.

#### Scenario: Pseudo-locale surfaces overflow

- **WHEN** the visual suite runs under the expanded pseudo-locale
- **THEN** layout overflow/clipping is caught before it ships
