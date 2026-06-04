-- Boiling Point — timeless replays.
-- One replay payload per completed game, stored in a single column. The payload
-- is a base64-encoded MessagePack body (seed + per-wave action log) that the
-- pinned engine re-runs to reconstruct the public event stream; the surrounding
-- columns carry the format/engine versions, the content-config identity, and an
-- integrity hash over the encoded bytes. Kept 1:1 with `games` (FK + PK on
-- game_id) so the `games` row stays lean and analytics never load the payload.

CREATE TABLE IF NOT EXISTS game_replays (
    game_id            UUID PRIMARY KEY REFERENCES games(id),
    payload            TEXT NOT NULL,
    format_version     SMALLINT NOT NULL,
    engine_version     SMALLINT NOT NULL,
    config_fingerprint TEXT NOT NULL,
    integrity_hash     TEXT NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
