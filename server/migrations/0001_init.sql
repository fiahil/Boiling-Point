-- Boiling Point — post-game persistence (single-table game records).
--
-- Two tables only:
--   players      — anonymous identity (UUID) → display name, written on first sight.
--   game_replays — one row per finished game: queryable metadata + denormalised
--                  `stats_*` summary columns + a single MessagePack replay payload.
--
-- The payload (raw MessagePack, `BYTEA`) re-runs the pinned engine to reconstruct
-- the full public event stream (seed + per-wave action log) and additionally
-- carries the timestamped raw-input log (every commit/pass/lock-in/emote a player
-- sent, with ms-since-game-start). The surrounding columns are everything you can
-- query without decoding the payload: who played, who won, and the summary stats.

CREATE TABLE IF NOT EXISTS players (
    id           UUID PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS game_replays (
    game_id             UUID PRIMARY KEY,
    started_at          TIMESTAMPTZ NOT NULL,
    ended_at            TIMESTAMPTZ NOT NULL,
    -- Seated roster, in seating order.
    player_ids          UUID[]      NOT NULL,
    -- The winner(s): an array for ties, NULL when no winner was declared
    -- (e.g. an aborted/errored game).
    winner_ids          UUID[],
    -- Per-player breakdown: [{player_id, display_name, color, final_score,
    -- finish_position, cards_played}], in seating order.
    scores              JSONB       NOT NULL,
    -- Denormalised summary stats (queryable without decoding the payload).
    stats_round_count   SMALLINT    NOT NULL,
    stats_player_count  SMALLINT    NOT NULL,
    stats_explosions    SMALLINT    NOT NULL,
    stats_cards_played  INT         NOT NULL,
    stats_high_score    INT         NOT NULL,
    stats_low_score     INT         NOT NULL,
    stats_deathmatch    BOOLEAN     NOT NULL,
    -- Replay payload (raw MessagePack) + provenance / integrity.
    payload             BYTEA       NOT NULL,
    format_version      SMALLINT    NOT NULL,
    engine_version      SMALLINT    NOT NULL,
    config_fingerprint  TEXT        NOT NULL,
    integrity_hash      TEXT        NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
