-- Boiling Point — initial schema (post-game persistence only).
-- Anonymous players (UUID identity), one row per completed game, per-player
-- results, and optional per-round analytics.

CREATE TABLE IF NOT EXISTS players (
    id           UUID PRIMARY KEY,
    display_name TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS games (
    id          UUID PRIMARY KEY,
    started_at  TIMESTAMPTZ NOT NULL,
    ended_at    TIMESTAMPTZ NOT NULL,
    round_count SMALLINT NOT NULL
);

CREATE TABLE IF NOT EXISTS game_players (
    game_id            UUID NOT NULL REFERENCES games(id),
    player_id          UUID NOT NULL REFERENCES players(id),
    final_score        INT NOT NULL,
    finish_position    SMALLINT NOT NULL,
    cards_played_total SMALLINT NOT NULL DEFAULT 0,
    PRIMARY KEY (game_id, player_id)
);

CREATE TABLE IF NOT EXISTS game_rounds (
    game_id          UUID NOT NULL REFERENCES games(id),
    round_number     SMALLINT NOT NULL,
    threshold        SMALLINT NOT NULL,
    exploded         BOOLEAN NOT NULL,
    volatility_total SMALLINT NOT NULL,
    cards_played     SMALLINT NOT NULL,
    PRIMARY KEY (game_id, round_number)
);
