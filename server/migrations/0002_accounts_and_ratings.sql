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

-- Privacy-first: no email, no real name. The only identity carried is the
-- per-kind opaque credential (device token / OAuth subject / passkey credential)
-- plus an auto-assigned, renameable-once pseudonym.
CREATE TABLE IF NOT EXISTS accounts (
    id              UUID PRIMARY KEY,
    -- The durable player id this account binds (the cross-game identity).
    player_id       UUID        NOT NULL,
    -- 'device' (device-bound anonymous), 'passkey', or 'oauth'.
    kind            TEXT        NOT NULL,
    -- The auto-assigned, unique, themed pseudonym (also the passkey sign-in handle).
    display_name    TEXT        NOT NULL,
    -- Remaining display-name changes (1 fresh, 0 once spent).
    renames_remaining SMALLINT  NOT NULL DEFAULT 1,
    -- The durable device token, for device-bound accounts (the client's secret).
    device_token    TEXT        UNIQUE,
    -- The OAuth provider label ('google'|'apple'|'microsoft'|'discord'), for OAuth.
    oauth_provider  TEXT,
    -- The provider's stable subject id, for OAuth accounts (no name, no email).
    oauth_subject   TEXT,
    -- The stored WebAuthn credential, for passkey accounts.
    passkey_credential TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Last successful sign-in (touched on every resume / OAuth / passkey login).
    last_login_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One account per player, one per (provider, subject) identity, and unique names.
CREATE UNIQUE INDEX IF NOT EXISTS accounts_player_idx ON accounts (player_id);
CREATE UNIQUE INDEX IF NOT EXISTS accounts_name_idx ON accounts (display_name);
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
