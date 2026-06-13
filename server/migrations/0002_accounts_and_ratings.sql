-- Boiling Point — persistent accounts + FFA rating (change `boom2-identity`).
--
-- Additive over the v1 schema (0001): the anonymous `players` table is
-- unchanged and anonymous play still needs no row here. An account merely
-- *binds* an existing player id so the identity persists across sessions and
-- devices; rating attaches to the account, never to an anonymous session.
--
-- No foreign key to `players(id)`: a player row is written on first *finished
-- game* (0001's "first sight"), but an account can be created before a player
-- has finished any game, so the link is kept loose (the player id is still the
-- durable identity, just not yet a `players` row).

CREATE TABLE IF NOT EXISTS accounts (
    id              UUID PRIMARY KEY,
    -- The durable player id this account binds (the cross-game identity).
    player_id       UUID        NOT NULL,
    -- 'device' (device-bound anonymous) or 'oauth'.
    kind            TEXT        NOT NULL,
    -- The durable device token, for device-bound accounts (the client's secret).
    device_token    TEXT        UNIQUE,
    -- The OAuth provider label ('google' | 'discord'), for OAuth accounts.
    oauth_provider  TEXT,
    -- The provider's stable subject id, for OAuth accounts.
    oauth_subject   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One account per player, and one account per (provider, subject) identity.
CREATE UNIQUE INDEX IF NOT EXISTS accounts_player_idx ON accounts (player_id);
CREATE UNIQUE INDEX IF NOT EXISTS accounts_oauth_idx
    ON accounts (oauth_provider, oauth_subject)
    WHERE oauth_provider IS NOT NULL;

-- Per-account FFA rating (Weng-Lin): the skill estimate (mu/sigma) and the
-- rated-game count. Anonymous participants never get a row.
CREATE TABLE IF NOT EXISTS account_ratings (
    account_id      UUID PRIMARY KEY REFERENCES accounts (id),
    mu              DOUBLE PRECISION NOT NULL,
    sigma           DOUBLE PRECISION NOT NULL,
    games_played    INTEGER          NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ      NOT NULL DEFAULT now()
);
