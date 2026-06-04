# Research — persistence-and-replays

## Open Questions

- **R1** — What replay fidelity/encoding gives "timeless" playback in one DB column?
- **R2** — Where does the replay payload live in the schema?
- **R3** — Are player profiles part of this work?

## R1 — Replay fidelity & encoding

**Decision:** A **hybrid** payload — the deterministic **input log** (root seed +
content-config identity + ordered per-wave actions) plus a `format_version`,
`engine_version`, and an integrity hash — encoded compactly (MessagePack → base64) into
a single column. Playback reconstructs by re-running the **pinned** engine version; when
the engine changes incompatibly, the payload is migrated/re-rendered (optionally caching
a resolved public-event snapshot) rather than lost.

**Rationale:** The engine is already deterministic from one `u64` seed over a pinned
content config (server-review §Determinism), so the input log is tiny and fully
reconstructs a game. The version tag + integrity hash buy "timeless": old replays
select a compatible reconstruction path, and we can detect corruption/drift. This is the
maintainer's chosen option.

**Alternatives considered:**
- *Deterministic input log only (smallest).* A stored replay breaks the moment engine
  logic changes — not timeless. Rejected as the sole strategy.
- *Resolved event log only (self-contained).* Stores every public outcome so it renders
  forever without the engine — but larger, and it duplicates logic the engine already
  has. Kept as the **migration target** for incompatible engine changes, not the primary
  store.

**Key details:** Requires an engine-side per-wave action recorder (today's `WaveChoice`
is transient). `engine_version` should bump on any change to engine resolution; the
content-config identity is the existing config fingerprint (cf. `bot-harness` report
`fingerprint`).

## R2 — Schema placement

**Decision:** Store the payload in a **single column**, either on the `games` row or a
1:1 `game_replays(game_id, payload, format_version, engine_version, integrity_hash)`
table, added via a new `0002_*` migration. A 1:1 table keeps the hot `games` row lean
and lets large payloads be fetched only when needed.

**Rationale:** Meets "fits nicely into a database column"; keeps result analytics
(scores, rounds) queryable without loading replay blobs.

**Alternatives considered:** an append-only event-sourcing table per action — powerful
(spectating, partial replays) but explicit v2 scope (roadmap) and far heavier than v1
needs.

## R3 — Player profiles

**Decision:** **Out of scope.** This change persists *match results* and *replays* only.
The existing anonymous per-game player record (UUID + display name) stays as the FK for
results. Player **profiles** (career stats, identity, cross-game history) move to the
[roadmap](../../../docs/roadmap.md) and depend on persistent accounts.

**Rationale:** Per the maintainer; keeps the change focused and avoids coupling to the
unbuilt accounts/identity work.

## Summary

- R1 → **hybrid** replay payload: input log + version tag + integrity hash; re-run the
  pinned engine to render; migrate/re-render on engine change.
- R2 → one column (prefer a 1:1 `game_replays` table); new `0002_*` migration.
- R3 → player profiles are **not** in this change (→ roadmap).
