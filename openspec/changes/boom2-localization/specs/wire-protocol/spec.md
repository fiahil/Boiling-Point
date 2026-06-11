## ADDED Requirements

### Requirement: Error Payloads Carry A Code And Params, Not Prose

An `Error` message SHALL carry a stable **`ErrorCode`** plus any **structured params** as the canonical, localizable contract; the client renders the localized string from the code. Any English `message` string SHALL be a **debug-only fallback**, never the localized surface shown to players.

#### Scenario: Client localizes from the error code

- **WHEN** the server sends an `Error` with a code and params
- **THEN** the client renders the message from its active locale using that code (+ params), not from any English string in the payload

#### Scenario: English message is debug-only

- **WHEN** an `Error` includes an English `message`
- **THEN** it is treated as a developer/debug fallback and is not the string presented to a player in a shipped locale
