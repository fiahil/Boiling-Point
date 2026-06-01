## ADDED Requirements

### Requirement: Single Authoritative Secret-Attribute Set

The server SHALL define one authoritative set of **secret span attributes** —
covering at least the boiling point, committed cards, player hands, and mid-round
volatility totals — within the versioned span-schema contract. This set SHALL be
the single source of truth for what must never leave the process via telemetry.

#### Scenario: Secret set is enumerated in one place

- **WHEN** any component needs to know whether a span attribute is secret
- **THEN** it consults the span-schema contract's secret-attribute set, not a
  locally duplicated list

### Requirement: Allow-List Redaction At The Export Boundary

The OTLP span exporter SHALL apply **allow-list** redaction: only attributes whose
keys are on the contract's public allow-list are exported, and every other
attribute (including all secret attributes) SHALL be stripped before a span leaves
the process. Redaction SHALL be fail-closed — an attribute not on the allow-list is
dropped even if it is not explicitly enumerated as secret.

#### Scenario: A secret attribute never leaves the process

- **WHEN** a span carrying a secret attribute (e.g. the boiling point) is exported
- **THEN** the secret attribute is absent from the exported span data handed to the
  underlying OTLP exporter

#### Scenario: An unknown attribute is dropped, not leaked

- **WHEN** a span carries an attribute key that is neither public nor enumerated as
  secret
- **THEN** redaction drops it (fail-closed) rather than exporting it

#### Scenario: Public attributes still export

- **WHEN** a span carries public attributes (e.g. room code, round number)
- **THEN** those attributes are preserved in the exported span data

### Requirement: Redaction Is A Tested Security Control

Redaction SHALL be covered by a test that asserts no secret-attribute key reaches
the exporter output, exercising the redacting exporter directly. The test SHALL
derive the secret keys from the authoritative secret-attribute set so that adding a
new secret without allow-listing it stays redacted by construction.

#### Scenario: A redaction test guards the boundary

- **WHEN** the redaction security test runs over a span populated with every secret
  attribute in the set
- **THEN** it asserts that none of those keys appear in the exporter's output
