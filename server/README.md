# server

The **authoritative** Boiling Point server. It owns the full domain model (including
all secrets) and the game loop; every client is an untrusted renderer that it
validates. Axum + Tokio, MessagePack over WebSocket. This is the single source of
truth (constitution Principle I).

## Layout

| Area | Modules | Role |
|---|---|---|
| Transport | `transport.rs` | WebSocket upgrade, entry handshake, per-connection in/out tasks. |
| Lobby | `lobby/{room,registry,codes,matchmaking,session}.rs` | One Tokio task per room (no locks; `mpsc` commands), the code→room registry, the table-filling match queue, and session/identity. |
| Game driver | `session.rs` | Runs a game: deal → rounds → waves → depile → score → `GameOver`. |
| Engine | `game/{runner,round,resolve,pot,deck,scoring,modifiers,deathmatch,state,card}.rs` | The pure, authoritative rules. Holds every secret (boiling point, hands, deck). |
| Content | `content/*`, `config.rs`, `content.toml` | Cards/effects/modifiers config, validated at startup. |
| Observability | `observability.rs`, `observability/{lifecycle,span_schema}.rs` | JSON logs, OTEL span bridge, Prometheus metrics, in-process span feed for admin. |
| Admin | `admin/{api,auth,projection}.rs` | Operator-only read/control API on an **isolated** port — never reachable from a player connection. |
| Persistence | `persistence.rs`, `migrations/` | Post-game Postgres writes (match results + a timeless replay in one transaction), wired on the live path. **Optional** — without `DATABASE_URL` the server runs fully in memory and persistence is a clean no-op. |

## Run

```sh
cargo run -p boiling-point-server -- --help     # all options
cargo run -p boiling-point-server                # defaults below
```

| Flag | Default | Purpose |
|---|---|---|
| `--ws-addr` | `0.0.0.0:8080` | player WebSocket (`/ws`) |
| `--metrics-addr` | `0.0.0.0:9090` | Prometheus exporter |
| `--admin-addr` | `0.0.0.0:8081` | operator-only admin API |
| `--config <PATH>` | embedded `content.toml` | content/balance config |
| `--log-level <LEVEL>` | `RUST_LOG` or `info` | JSON-log verbosity |
| `--conn-timeout-secs` | `90` | player connection idle/grace timeout |

Admin tokens come from the environment (`BP_ADMIN_TOKEN` / `BP_ADMIN_OBSERVER_TOKEN`);
without them the admin API rejects every request.

## Test

```sh
cargo test -p boiling-point-server              # unit + integration (boots in-process)
cargo test -p boiling-point-server --lib        # unit tests only
```

`tests/admin_e2e.rs` exercises the admin API end-to-end; `tests/lifecycle_log_level.rs`
guards that the admin span feed stays unsampled regardless of `RUST_LOG`.

See [`docs/03_architecture/01_overview.md`](../docs/03_architecture/01_overview.md) and
[`docs/04_reviews/02_server-review.md`](../docs/04_reviews/02_server-review.md).
