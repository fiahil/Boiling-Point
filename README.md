# Boiling Point

A 4-player free-for-all card game with an **authoritative Rust server**: the server
owns all game state and secrets, and every client is an untrusted renderer. Players
secretly toss ingredient cards into a shared, unstable cauldron — push the brew past
its hidden boiling point and *everyone* eats the loss; stop in time and the dominant
color scoops the pot.

📖 **New here? Start with [docs/01_getting-started.md](docs/01_getting-started.md).**
The full documentation hub is [docs/](docs/); the canonical rules are in
[docs/02_game-design.md](docs/02_game-design.md).

## Workspace

```
protocol/       wire messages + MessagePack/JSON codec (no game logic, no secrets)
server/         authoritative game engine, content/config, game loop, admin + metrics
clients/
└── web/        graphical client — TypeScript + PixiJS (lands with adopt-pixi-client)
archive/        retired v1 components — tui-client, bot-harness, agent-harness,
                playtest.sh (revivable, not deleted; see archive/README.md)
docs/           architecture, game design, roadmap, and code reviews
openspec/       change proposals (changes/), resolved specs (specs/), archive
```

See [docs/03_architecture/01_overview.md](docs/03_architecture/01_overview.md) for how
the pieces fit together.

## Development

```sh
make check    # fmt + clippy (-D warnings) + tests — the CI gate
make run      # boot the server (loads & validates the default content config)
make test     # cargo test --workspace
```

The server binary has `--help` (clap). Balance/content lives in
[`server/content.toml`](server/content.toml) and is validated at startup; an
inconsistent config fails the boot, not a game.

## How changes are made

Work is proposed and tracked with a file-based [OpenSpec](openspec/) workflow:
proposals in [`openspec/changes/`](openspec/changes/), the current resolved capability
specs in [`openspec/specs/`](openspec/specs/), and shipped work in
[`openspec/changes/archive/`](openspec/changes/archive/). The project
[constitution](CLAUDE.md) governs all of it.
