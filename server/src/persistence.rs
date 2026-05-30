//! Post-game persistence (PostgreSQL via sqlx).
//!
//! The server writes exactly once, at `GameOver`: the game, its anonymous
//! players, their per-player results, and optional per-round analytics — in one
//! transaction. No game state is written mid-game (a crash loses only the
//! in-progress game). Runtime queries are used (no compile-time DB needed).

use chrono::{DateTime, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// The final result of one player in a completed game.
#[derive(Debug, Clone)]
pub struct PlayerResult {
    /// The player's session-issued UUID.
    pub player_id: Uuid,
    /// The player's display name (for the anonymous player record).
    pub display_name: String,
    /// Final cumulative score (may be negative).
    pub final_score: i32,
    /// 1-based finishing position (1 = winner; ties may share).
    pub finish_position: i16,
    /// Total cards the player played across the game.
    pub cards_played_total: i16,
}

/// Optional per-round analytics row.
#[derive(Debug, Clone)]
pub struct RoundResult {
    /// 1-based round number.
    pub round_number: i16,
    /// The round's (post-modifier) boiling point.
    pub threshold: i16,
    /// Whether the round exploded.
    pub exploded: bool,
    /// Final cauldron volatility.
    pub volatility_total: i16,
    /// Cards played in the round.
    pub cards_played: i16,
}

/// A completed game ready to persist.
#[derive(Debug, Clone)]
pub struct GameResult {
    /// The game's UUID.
    pub game_id: Uuid,
    /// When the game started.
    pub started_at: DateTime<Utc>,
    /// When the game ended.
    pub ended_at: DateTime<Utc>,
    /// Number of rounds played.
    pub round_count: i16,
    /// Per-player results.
    pub players: Vec<PlayerResult>,
    /// Optional per-round analytics.
    pub rounds: Vec<RoundResult>,
}

/// Connect to PostgreSQL and return a pool.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

/// Apply the schema (idempotent). Embedded so no external migration tool is needed.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    let schema = include_str!("../migrations/0001_init.sql");
    sqlx::raw_sql(schema).execute(pool).await?;
    Ok(())
}

/// Persist a completed game and all its results in a single transaction. This is
/// the only write the server performs, at `GameOver`.
pub async fn persist_game(pool: &PgPool, result: &GameResult) -> Result<(), sqlx::Error> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    // Anonymous player records (created on first sight of a UUID).
    for p in &result.players {
        sqlx::query(
            "INSERT INTO players (id, display_name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
        )
        .bind(p.player_id)
        .bind(&p.display_name)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO games (id, started_at, ended_at, round_count) VALUES ($1, $2, $3, $4)",
    )
    .bind(result.game_id)
    .bind(result.started_at)
    .bind(result.ended_at)
    .bind(result.round_count)
    .execute(&mut *tx)
    .await?;

    for p in &result.players {
        sqlx::query(
            "INSERT INTO game_players \
             (game_id, player_id, final_score, finish_position, cards_played_total) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(result.game_id)
        .bind(p.player_id)
        .bind(p.final_score)
        .bind(p.finish_position)
        .bind(p.cards_played_total)
        .execute(&mut *tx)
        .await?;
    }

    for r in &result.rounds {
        sqlx::query(
            "INSERT INTO game_rounds \
             (game_id, round_number, threshold, exploded, volatility_total, cards_played) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(result.game_id)
        .bind(r.round_number)
        .bind(r.threshold)
        .bind(r.exploded)
        .bind(r.volatility_total)
        .bind(r.cards_played)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await
}

/// Fetch (player_id, final_score, finish_position) for a game, ordered by
/// position — for retrieving a persisted result.
pub async fn fetch_player_results(
    pool: &PgPool,
    game_id: Uuid,
) -> Result<Vec<(Uuid, i32, i16)>, sqlx::Error> {
    let rows: Vec<(Uuid, i32, i16)> = sqlx::query_as(
        "SELECT player_id, final_score, finish_position \
         FROM game_players WHERE game_id = $1 ORDER BY finish_position",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Connection string for the local dev database; override via `DATABASE_URL`.
    fn database_url() -> String {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@localhost:5432/boiling_point".to_string())
    }

    /// Round-trips a completed game through PostgreSQL. Ignored by default
    /// (needs a live DB); run with `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a local PostgreSQL (DATABASE_URL)"]
    async fn persist_and_fetch_game() {
        let pool = connect(&database_url()).await.expect("connect");
        run_migrations(&pool).await.expect("migrate");

        let game_id = Uuid::new_v4();
        let players: Vec<PlayerResult> = (0..4)
            .map(|i| PlayerResult {
                player_id: Uuid::new_v4(),
                display_name: format!("player{i}"),
                final_score: 10 - i,
                finish_position: (i + 1) as i16,
                cards_played_total: 7,
            })
            .collect();
        let result = GameResult {
            game_id,
            started_at: Utc::now(),
            ended_at: Utc::now(),
            round_count: 5,
            players: players.clone(),
            rounds: vec![RoundResult {
                round_number: 1,
                threshold: 11,
                exploded: false,
                volatility_total: 9,
                cards_played: 6,
            }],
        };

        persist_game(&pool, &result).await.expect("persist");

        let fetched = fetch_player_results(&pool, game_id).await.expect("fetch");
        assert_eq!(fetched.len(), 4);
        assert_eq!(fetched[0].0, players[0].player_id); // position 1 = winner
        assert_eq!(fetched[0].1, 10);
        assert_eq!(fetched[0].2, 1);
    }
}
