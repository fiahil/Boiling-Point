//! Post-game persistence (PostgreSQL via sqlx).
//!
//! The server writes exactly once, at `GameOver`: the anonymous player records
//! and one consolidated `game_replays` row — queryable metadata + denormalised
//! `stats_*` summary columns + the MessagePack replay payload — in a single
//! transaction. No game state is written mid-game (a crash loses only the
//! in-progress game). Runtime queries are used (no compile-time DB needed).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use boiling_point_protocol::vocab::Color;

/// One player's finished-game line, stored in the `scores` JSONB column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerOutcome {
    /// The player's session-issued UUID.
    pub player_id: Uuid,
    /// The player's display name.
    pub display_name: String,
    /// The colour (seat) the player held.
    pub color: Color,
    /// Final cumulative score (may be negative).
    pub final_score: i32,
    /// 1-based finishing position (1 = winner; ties may share).
    pub finish_position: i16,
    /// Total cards the player played across the game.
    pub cards_played: i16,
}

/// Denormalised summary stats for a finished game — the queryable `stats_*`
/// columns, so leaderboards/analytics never need to decode the payload.
#[derive(Debug, Clone, PartialEq)]
pub struct GameStats {
    /// Number of rounds played.
    pub round_count: i16,
    /// Number of seated players.
    pub player_count: i16,
    /// How many rounds exploded.
    pub explosions: i16,
    /// Total cards played across all players and rounds.
    pub cards_played: i32,
    /// Highest final score at the table.
    pub high_score: i32,
    /// Lowest final score at the table.
    pub low_score: i32,
    /// Whether the lead was tied at game end (i.e. a deathmatch tiebreak ran).
    pub deathmatch: bool,
}

/// A game's replay payload + provenance, built by [`crate::replay::encode_replay`].
/// The `payload` is the raw MessagePack body (stored in the `BYTEA` column); the
/// surrounding fields carry its format/engine versions, the content-config
/// identity, and an integrity hash over the bytes.
#[derive(Debug, Clone)]
pub struct StoredReplay {
    /// The game this replay belongs to.
    pub game_id: Uuid,
    /// Raw MessagePack replay body (the `payload` column value).
    pub payload: Vec<u8>,
    /// Replay payload format version.
    pub format_version: i16,
    /// Engine version the payload was recorded under.
    pub engine_version: i16,
    /// Content-config identity the game ran against.
    pub config_fingerprint: String,
    /// Integrity hash over the payload bytes (hex SHA-256).
    pub integrity_hash: String,
}

/// A completed game ready to persist as one `game_replays` row.
#[derive(Debug, Clone)]
pub struct CompletedGame {
    /// The game's UUID (primary key).
    pub game_id: Uuid,
    /// When the game started.
    pub started_at: DateTime<Utc>,
    /// When the game ended.
    pub ended_at: DateTime<Utc>,
    /// The seated roster, in seating order.
    pub player_ids: Vec<Uuid>,
    /// The winner(s): an array for ties, `None` when no winner was declared.
    pub winner_ids: Option<Vec<Uuid>>,
    /// Per-player breakdown (→ `scores` JSONB), in seating order.
    pub players: Vec<PlayerOutcome>,
    /// Denormalised summary stats.
    pub stats: GameStats,
    /// The replay payload + provenance.
    pub replay: StoredReplay,
}

/// Connect to PostgreSQL and return a pool.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

/// Advisory-lock key serialising concurrent migrators (an arbitrary constant,
/// "BOIL" in ASCII). Held only for the migration transaction.
const MIGRATION_LOCK_KEY: i64 = 0x424f_494c;

/// Apply the schema (idempotent). Embedded so no external migration tool is
/// needed; the migration uses `CREATE TABLE IF NOT EXISTS`, so re-running is a
/// no-op. Applied on every boot when a database is configured.
///
/// Wrapped in one transaction guarded by a transaction-scoped advisory lock:
/// `CREATE TABLE IF NOT EXISTS` is not safe under concurrency on its own, so two
/// instances booting (or parallel tests) against the same database serialise
/// here rather than racing on `pg_catalog`.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *tx)
        .await?;
    for migration in [include_str!("../migrations/0001_init.sql")] {
        sqlx::raw_sql(migration).execute(&mut *tx).await?;
    }
    tx.commit().await
}

/// Persist a completed game — the anonymous player records and the consolidated
/// `game_replays` row — in a single transaction. This is the only write the
/// server performs, at `GameOver`.
///
/// `db.write` span (span_schema::span::DB_WRITE): game.id and db.rows are public.
#[tracing::instrument(
    name = "db.write",
    skip_all,
    fields(game.id = %game.game_id, db.rows = tracing::field::Empty)
)]
pub async fn persist_game(pool: &PgPool, game: &CompletedGame) -> Result<(), sqlx::Error> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    // Anonymous player records (created on first sight of a UUID).
    for p in &game.players {
        sqlx::query(
            "INSERT INTO players (id, display_name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
        )
        .bind(p.player_id)
        .bind(&p.display_name)
        .execute(&mut *tx)
        .await?;
    }

    // The one consolidated game row: metadata + stats + replay payload.
    sqlx::query(
        "INSERT INTO game_replays (\
            game_id, started_at, ended_at, player_ids, winner_ids, scores, \
            stats_round_count, stats_player_count, stats_explosions, stats_cards_played, \
            stats_high_score, stats_low_score, stats_deathmatch, \
            payload, format_version, engine_version, config_fingerprint, integrity_hash\
         ) VALUES \
         ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
    )
    .bind(game.game_id)
    .bind(game.started_at)
    .bind(game.ended_at)
    .bind(game.player_ids.as_slice())
    .bind(game.winner_ids.as_deref())
    .bind(Json(&game.players))
    .bind(game.stats.round_count)
    .bind(game.stats.player_count)
    .bind(game.stats.explosions)
    .bind(game.stats.cards_played)
    .bind(game.stats.high_score)
    .bind(game.stats.low_score)
    .bind(game.stats.deathmatch)
    .bind(&game.replay.payload)
    .bind(game.replay.format_version)
    .bind(game.replay.engine_version)
    .bind(&game.replay.config_fingerprint)
    .bind(&game.replay.integrity_hash)
    .execute(&mut *tx)
    .await?;

    // Public row count for the db.write span: players (anon) + the game row.
    let rows = game.players.len() + 1;
    tracing::Span::current().record("db.rows", rows as u64);
    tx.commit().await
}

/// Fetch a game's stored replay by id, if one was persisted. The caller verifies
/// its integrity and reconstructs via [`crate::replay`].
pub async fn fetch_replay(
    pool: &PgPool,
    game_id: Uuid,
) -> Result<Option<StoredReplay>, sqlx::Error> {
    let row: Option<(Vec<u8>, i16, i16, String, String)> = sqlx::query_as(
        "SELECT payload, format_version, engine_version, config_fingerprint, integrity_hash \
         FROM game_replays WHERE game_id = $1",
    )
    .bind(game_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(payload, format_version, engine_version, config_fingerprint, integrity_hash)| {
            StoredReplay {
                game_id,
                payload,
                format_version,
                engine_version,
                config_fingerprint,
                integrity_hash,
            }
        },
    ))
}

/// Fetch `(player_id, final_score, finish_position)` for a game, ordered by
/// position — read from the `scores` JSONB of the consolidated game row.
pub async fn fetch_player_results(
    pool: &PgPool,
    game_id: Uuid,
) -> Result<Vec<(Uuid, i32, i16)>, sqlx::Error> {
    let row: Option<(Json<Vec<PlayerOutcome>>,)> =
        sqlx::query_as("SELECT scores FROM game_replays WHERE game_id = $1")
            .bind(game_id)
            .fetch_optional(pool)
            .await?;
    let mut out: Vec<(Uuid, i32, i16)> = row
        .map(|(Json(players),)| {
            players
                .into_iter()
                .map(|p| (p.player_id, p.final_score, p.finish_position))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|r| r.2);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::encode_replay;

    /// Connection string for the local dev database; override via `DATABASE_URL`.
    fn database_url() -> String {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@localhost:5432/boiling_point".to_string())
    }

    /// A throwaway replay payload (empty action/input logs) for persistence tests
    /// that only exercise the metadata/stats columns.
    fn dummy_replay(game_id: Uuid) -> StoredReplay {
        let cfg = crate::config::ContentConfig::from_toml(include_str!("../content.toml")).unwrap();
        encode_replay(game_id, 0, &cfg, std::iter::empty(), &[], &[]).expect("encode")
    }

    /// Round-trips a completed game through PostgreSQL. Ignored by default
    /// (needs a live DB); run with `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a local PostgreSQL (DATABASE_URL)"]
    async fn persist_and_fetch_game() {
        let pool = connect(&database_url()).await.expect("connect");
        run_migrations(&pool).await.expect("migrate");

        let game_id = Uuid::new_v4();
        let players: Vec<PlayerOutcome> = (0..4)
            .map(|i| PlayerOutcome {
                player_id: Uuid::new_v4(),
                display_name: format!("player{i}"),
                color: Color::PLAYER_COLORS[i as usize],
                final_score: 10 - i,
                finish_position: (i + 1) as i16,
                cards_played: 7,
            })
            .collect();
        let game = CompletedGame {
            game_id,
            started_at: Utc::now(),
            ended_at: Utc::now(),
            player_ids: players.iter().map(|p| p.player_id).collect(),
            winner_ids: Some(vec![players[0].player_id]),
            players: players.clone(),
            stats: GameStats {
                round_count: 5,
                player_count: 4,
                explosions: 2,
                cards_played: 28,
                high_score: 10,
                low_score: 7,
                deathmatch: false,
            },
            replay: dummy_replay(game_id),
        };

        persist_game(&pool, &game).await.expect("persist");

        let fetched = fetch_player_results(&pool, game_id).await.expect("fetch");
        assert_eq!(fetched.len(), 4);
        assert_eq!(fetched[0].0, players[0].player_id); // position 1 = winner
        assert_eq!(fetched[0].1, 10);
        assert_eq!(fetched[0].2, 1);
    }
}
