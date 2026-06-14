//! Post-game persistence (PostgreSQL via sqlx).
//!
//! The server writes exactly once, at `GameOver`: the anonymous player records
//! and one consolidated `game_replays` row — queryable metadata + denormalised
//! `stats_*` summary columns + the MessagePack replay payload — in a single
//! transaction. No game state is written mid-game (a crash loses only the
//! in-progress game). Runtime queries are used (no compile-time DB needed).

use std::collections::HashMap;

use chrono::{DateTime, Duration, NaiveDate, Utc};
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
    for migration in [
        include_str!("../migrations/0001_init.sql"),
        include_str!("../migrations/0002_accounts_and_ratings.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut *tx).await?;
    }
    tx.commit().await
}

/// One persisted account row (boot-time hydrate). Tuple-shaped for `query_as`.
pub type AccountRow = (
    Uuid,           // id
    Uuid,           // player_id
    String,         // kind
    String,         // display_name
    i16,            // renames_remaining
    Option<String>, // device_token
    Option<String>, // oauth_provider
    Option<String>, // oauth_subject
    Option<String>, // passkey_credential
);

/// Write-through an account on creation (idempotent on the id). Plain arguments
/// so this layer stays decoupled from the lobby's account types.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_account(
    pool: &PgPool,
    id: Uuid,
    player_id: Uuid,
    kind: &str,
    display_name: &str,
    renames_remaining: i16,
    device_token: Option<&str>,
    oauth_provider: Option<&str>,
    oauth_subject: Option<&str>,
    passkey_credential: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts \
         (id, player_id, kind, display_name, renames_remaining, device_token, \
          oauth_provider, oauth_subject, passkey_credential) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
    )
    .bind(id)
    .bind(player_id)
    .bind(kind)
    .bind(display_name)
    .bind(renames_remaining)
    .bind(device_token)
    .bind(oauth_provider)
    .bind(oauth_subject)
    .bind(passkey_credential)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Persist a display-name change (the one allowed rename).
pub async fn update_account_name(
    pool: &PgPool,
    id: Uuid,
    display_name: &str,
    renames_remaining: i16,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts SET display_name = $2, renames_remaining = $3 WHERE id = $1")
        .bind(id)
        .bind(display_name)
        .bind(renames_remaining)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Touch an account's last-login timestamp on a successful sign-in.
pub async fn touch_last_login(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts SET last_login_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Delete an account (identity-only erasure): its rating, the account row, and
/// the player record. Shared `game_replays` are immutable anonymous records and
/// are intentionally left intact. Runs in one transaction.
pub async fn delete_account(
    pool: &PgPool,
    account_id: Uuid,
    player_id: Uuid,
) -> Result<(), sqlx::Error> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    sqlx::query("DELETE FROM account_ratings WHERE account_id = $1")
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM players WHERE id = $1")
        .bind(player_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

/// Load every persisted account (boot-time hydrate).
pub async fn load_accounts(pool: &PgPool) -> Result<Vec<AccountRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, player_id, kind, display_name, renames_remaining, device_token, \
                oauth_provider, oauth_subject, passkey_credential FROM accounts",
    )
    .fetch_all(pool)
    .await
}

/// Write-through an account's rating after a finished game (upsert).
pub async fn persist_rating(
    pool: &PgPool,
    account_id: Uuid,
    mu: f64,
    sigma: f64,
    games_played: u32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO account_ratings (account_id, mu, sigma, games_played, updated_at) \
         VALUES ($1, $2, $3, $4, now()) \
         ON CONFLICT (account_id) DO UPDATE \
         SET mu = EXCLUDED.mu, sigma = EXCLUDED.sigma, \
             games_played = EXCLUDED.games_played, updated_at = now()",
    )
    .bind(account_id)
    .bind(mu)
    .bind(sigma)
    .bind(games_played as i32)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Load every persisted rating (boot-time hydrate). Returns rows as
/// `(account_id, mu, sigma, games_played)`.
pub async fn load_ratings(pool: &PgPool) -> Result<Vec<(Uuid, f64, f64, i32)>, sqlx::Error> {
    sqlx::query_as("SELECT account_id, mu, sigma, games_played FROM account_ratings")
        .fetch_all(pool)
        .await
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

/// One day of popularity figures (UTC days).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DailyPopularity {
    /// The UTC day (`YYYY-MM-DD`).
    pub day: NaiveDate,
    /// Games completed that day.
    pub games: i64,
    /// Distinct players seated in those games.
    pub players: i64,
    /// Players whose first-ever game was that day.
    pub new_players: i64,
}

/// Popularity figures read from post-game persistence: a per-day series over the
/// window (gaps filled with zero days), a games-by-hour-of-day histogram, the
/// returning-player count, and window/lifetime totals.
#[derive(Debug, Clone, Serialize)]
pub struct PopularityStats {
    /// The window size in days (the series covers the last `window_days` UTC
    /// days, oldest first, today included).
    pub window_days: i64,
    /// One entry per day in the window, oldest first, zero-filled.
    pub daily: Vec<DailyPopularity>,
    /// Games within the window by UTC hour of day: 24 entries, index = hour.
    pub by_hour: Vec<i64>,
    /// Games completed within the window.
    pub window_games: i64,
    /// Distinct players seated within the window.
    pub window_players: i64,
    /// Players whose first-ever game fell within the window.
    pub window_new_players: i64,
    /// Players who played on **more than one** distinct UTC day of the window —
    /// the returning-player (stickiness) numerator over `window_players`.
    pub window_returning_players: i64,
    /// Games ever recorded.
    pub total_games: i64,
    /// Players ever seen.
    pub total_players: i64,
}

/// Zero-fill a per-day series over the `window_days` UTC days ending at `today`
/// (inclusive), oldest first, from sparse `(day → (games, players, new))` rows.
/// Pure, so the shape is testable without a database.
fn fill_daily(
    window_days: i64,
    today: NaiveDate,
    rows: &HashMap<NaiveDate, (i64, i64, i64)>,
) -> Vec<DailyPopularity> {
    (0..window_days)
        .rev()
        .map(|back| {
            let day = today - Duration::days(back);
            let (games, players, new_players) = rows.get(&day).copied().unwrap_or((0, 0, 0));
            DailyPopularity {
                day,
                games,
                players,
                new_players,
            }
        })
        .collect()
}

/// Zero-fill the 24 hour-of-day buckets from sparse `(hour, games)` rows
/// (out-of-range hours are ignored). Pure, so the shape is testable without a
/// database.
fn fill_hours(rows: &[(i32, i64)]) -> Vec<i64> {
    let mut by_hour = vec![0i64; 24];
    for (hour, games) in rows {
        if let Some(slot) = by_hour.get_mut(*hour as usize) {
            *slot = *games;
        }
    }
    by_hour
}

/// Read the popularity figures for the last `window_days` UTC days from the
/// consolidated `game_replays` rows (and the `players` first-seen table). A
/// read-only analytics query — it never touches the replay payloads.
pub async fn fetch_popularity(
    pool: &PgPool,
    window_days: i64,
) -> Result<PopularityStats, sqlx::Error> {
    let window_days = window_days.clamp(1, 365);
    let today = Utc::now().date_naive();
    let cutoff = today - Duration::days(window_days - 1);

    // Games and distinct seated players per UTC day in the window.
    let per_day: Vec<(NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT (started_at AT TIME ZONE 'UTC')::date AS day, \
                count(*) AS games, \
                count(DISTINCT p) AS players \
         FROM game_replays, unnest(player_ids) AS p \
         WHERE (started_at AT TIME ZONE 'UTC')::date >= $1 \
         GROUP BY 1",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?;

    // New players per UTC day: players whose first-ever game fell in the window.
    let new_per_day: Vec<(NaiveDate, i64)> = sqlx::query_as(
        "SELECT first_day AS day, count(*) AS new_players FROM ( \
            SELECT p, min((started_at AT TIME ZONE 'UTC')::date) AS first_day \
            FROM game_replays, unnest(player_ids) AS p \
            GROUP BY p \
         ) firsts \
         WHERE first_day >= $1 \
         GROUP BY 1",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?;

    // When in the day people play: games in the window by UTC hour of day.
    let per_hour: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT extract(hour FROM started_at AT TIME ZONE 'UTC')::int AS hour, \
                count(*) AS games \
         FROM game_replays \
         WHERE (started_at AT TIME ZONE 'UTC')::date >= $1 \
         GROUP BY 1",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?;

    let (total_games, total_players): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM game_replays), (SELECT count(*) FROM players)",
    )
    .fetch_one(pool)
    .await?;
    let (window_games, window_players): (i64, i64) = sqlx::query_as(
        "SELECT count(DISTINCT game_id), count(DISTINCT p) \
         FROM game_replays, unnest(player_ids) AS p \
         WHERE (started_at AT TIME ZONE 'UTC')::date >= $1",
    )
    .bind(cutoff)
    .fetch_one(pool)
    .await?;
    // Stickiness: window players who came back on a second distinct day.
    let (window_returning_players,): (i64,) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE days > 1) FROM ( \
            SELECT p, count(DISTINCT (started_at AT TIME ZONE 'UTC')::date) AS days \
            FROM game_replays, unnest(player_ids) AS p \
            WHERE (started_at AT TIME ZONE 'UTC')::date >= $1 \
            GROUP BY p \
         ) per_player",
    )
    .bind(cutoff)
    .fetch_one(pool)
    .await?;

    let mut rows: HashMap<NaiveDate, (i64, i64, i64)> = per_day
        .into_iter()
        .map(|(day, games, players)| (day, (games, players, 0)))
        .collect();
    for (day, new_players) in new_per_day {
        rows.entry(day).or_insert((0, 0, 0)).2 = new_players;
    }
    let daily = fill_daily(window_days, today, &rows);
    let window_new_players = daily.iter().map(|d| d.new_players).sum();

    Ok(PopularityStats {
        window_days,
        daily,
        by_hour: fill_hours(&per_hour),
        window_games,
        window_players,
        window_new_players,
        window_returning_players,
        total_games,
        total_players,
    })
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
        encode_replay(
            game_id,
            0,
            &cfg,
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            &[],
            &[],
        )
        .expect("encode")
    }

    /// The day series is zero-filled over the whole window, oldest first, with
    /// sparse rows landing on their day.
    #[test]
    fn fill_daily_zero_fills_the_window() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        let mut rows = HashMap::new();
        rows.insert(today, (3, 9, 2));
        rows.insert(today - Duration::days(2), (1, 4, 4));
        let daily = fill_daily(4, today, &rows);

        assert_eq!(daily.len(), 4, "one entry per day in the window");
        assert_eq!(daily[0].day, today - Duration::days(3));
        assert_eq!(
            (daily[0].games, daily[0].players),
            (0, 0),
            "gap day is zero"
        );
        assert_eq!(
            (daily[1].games, daily[1].players, daily[1].new_players),
            (1, 4, 4)
        );
        assert_eq!((daily[2].games, daily[2].players), (0, 0));
        assert_eq!(daily[3].day, today, "newest day last");
        assert_eq!(
            (daily[3].games, daily[3].players, daily[3].new_players),
            (3, 9, 2)
        );
    }

    /// The hour histogram always has 24 buckets, sparse rows land on their hour,
    /// and out-of-range hours are ignored rather than panicking.
    #[test]
    fn fill_hours_zero_fills_all_buckets() {
        let by_hour = fill_hours(&[(0, 2), (13, 5), (23, 1), (24, 9), (-1, 9)]);
        assert_eq!(by_hour.len(), 24);
        assert_eq!(by_hour[0], 2);
        assert_eq!(by_hour[13], 5);
        assert_eq!(by_hour[23], 1);
        assert_eq!(by_hour.iter().sum::<i64>(), 8, "out-of-range rows ignored");
    }

    /// Popularity figures over freshly persisted games. Ignored by default
    /// (needs a live DB); run with `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a local PostgreSQL (DATABASE_URL)"]
    async fn popularity_counts_persisted_games() {
        let pool = connect(&database_url()).await.expect("connect");
        run_migrations(&pool).await.expect("migrate");

        let before = fetch_popularity(&pool, 30).await.expect("popularity");
        // Two games today with one shared roster of brand-new players.
        let players: Vec<PlayerOutcome> = (0..4)
            .map(|i| PlayerOutcome {
                player_id: Uuid::new_v4(),
                display_name: format!("pop{i}"),
                color: Color::PLAYER_COLORS[i as usize],
                final_score: i,
                finish_position: (4 - i) as i16,
                cards_played: 5,
            })
            .collect();
        for _ in 0..2 {
            let game_id = Uuid::new_v4();
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
                    cards_played: 20,
                    high_score: 3,
                    low_score: 0,
                    deathmatch: false,
                },
                replay: dummy_replay(game_id),
            };
            persist_game(&pool, &game).await.expect("persist");
        }

        let after = fetch_popularity(&pool, 30).await.expect("popularity");
        assert_eq!(after.window_days, 30);
        assert_eq!(after.daily.len(), 30, "the series covers the whole window");
        let today = after.daily.last().expect("today is the last entry");
        assert!(today.games >= 2, "today counts the persisted games");
        assert!(today.players >= 4, "today counts the seated players");
        assert!(today.new_players >= 4, "first-ever players count as new");
        assert!(after.total_games >= before.total_games + 2);
        assert!(after.total_players >= before.total_players + 4);
        assert!(after.window_games >= 2 && after.window_players >= 4);
        // Hour histogram: 24 buckets covering exactly the window's games, with
        // the just-persisted games landing on the current UTC hour.
        assert_eq!(after.by_hour.len(), 24);
        assert_eq!(after.by_hour.iter().sum::<i64>(), after.window_games);
        let this_hour = chrono::Timelike::hour(&Utc::now()) as usize;
        assert!(after.by_hour[this_hour] >= 2);
        // Returning players: a single-day roster adds none, and the count never
        // exceeds the window's distinct players.
        assert!(after.window_returning_players <= after.window_players);
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
