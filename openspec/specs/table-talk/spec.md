# table-talk Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Preset Emote Palette

The server SHALL offer a fixed, curated palette of preset emotes as the only in-game communication channel — there is no free-text chat and no quick-phrases. The palette is defined in config and validated at startup.

#### Scenario: A palette emote is accepted

- **WHEN** a player sends an emote whose id is in the configured palette
- **THEN** the server accepts it for broadcast

#### Scenario: Unknown id or free text is rejected

- **WHEN** a player sends an emote id not in the palette, or any free-text payload
- **THEN** the server replies with an `Error` to that sender and broadcasts nothing

### Requirement: Emotes Are Broadcast and Non-Binding

A valid emote SHALL be broadcast to the whole room, attributed to its sender, in any phase, and MUST NOT change any game state — it carries no mechanical weight (the lie is the feature).

#### Scenario: Emote reaches the table without affecting state

- **WHEN** a player sends a valid emote during an open wave
- **THEN** every player in the room receives the emote attributed to the sender
- **AND** the cauldron, hands, scores, modifiers, and wave timer are all unchanged

### Requirement: Emote Rate Limiting

Emotes SHALL be subject to the same per-connection rate limit as other actions (one per 100 ms), with excess silently dropped, so the channel cannot be used to spam.

#### Scenario: Emote spam is throttled

- **WHEN** a player sends emotes faster than the rate limit
- **THEN** the server broadcasts at most one per window and silently drops the rest

