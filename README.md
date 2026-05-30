# Boiling-Point

A 4-player free-for-all card game with an authoritative Rust server. See the
canonical [game design](knowledge/game-design.md) and the active work in
[`openspec/changes/`](openspec/changes/).

## Workspace

```
protocol/   wire messages + MessagePack/JSON codec (no game logic, no secrets)
server/     authoritative game engine, content/config, game loop
```

The full bot/balance harness lives in the separate `bot-balance-harness` change.

## Development

```sh
make check   # fmt + clippy (-D warnings) + tests — the CI gate
make run     # boot the server (loads & validates the default content config)
make test    # cargo test --workspace
```

Balance/content lives in [`server/content.toml`](server/content.toml) and is
validated at startup; an inconsistent config fails the boot, not a game.
