## ADDED Requirements

### Requirement: TypeScript Wire Types Generated From The Rust Protocol

The web client's protocol types SHALL be **generated** from the canonical Rust `protocol`
crate, which remains the single source of truth for the wire contract. Hand-written
TypeScript duplicates of protocol messages, enums, or IDs are prohibited.

#### Scenario: Generated types mirror the Rust source

- **WHEN** the typegen step runs against the `protocol` crate
- **THEN** it emits TypeScript types covering every client-facing message, enum, and ID,
  and the web client imports those generated types rather than hand-written copies

#### Scenario: A protocol change forces a client update

- **WHEN** a message in the Rust `protocol` crate changes shape and types are regenerated
- **THEN** the web client's TypeScript build fails wherever it no longer matches, until the
  client is updated to the new contract

### Requirement: Generation Is Reproducible And Enforced In CI

Regenerating the types SHALL be deterministic, and CI SHALL fail if the checked-in
generated types are stale relative to the `protocol` crate.

#### Scenario: Stale generated types fail CI

- **WHEN** the `protocol` crate has changed but the committed generated TypeScript was not
  regenerated
- **THEN** the CI typegen check detects the drift and fails the build

### Requirement: Transport Format Is Unchanged

Typegen SHALL describe the **same** MessagePack-over-WebSocket messages the server already
speaks; it MUST NOT introduce a new wire format or a parallel protocol.

#### Scenario: Same bytes on the wire

- **WHEN** the generated client encodes a message and the server decodes it (and vice
  versa)
- **THEN** both use the existing MessagePack encoding of the `protocol` crate's types with
  no format change
