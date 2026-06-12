# Boiling Point — AI client (`clients/ai`)

One Rust client, two jobs (change `boom2-ai-client`):

- **Harness mode** (`balance_tester`) — the constitution §IV reinstatement: seeded
  batches of thousands of complete v2 games over an in-process server, the
  persona-matrix sample spec, and diffable balance reports.
- **Seat-filler mode** (`familiar_summoning`) — the product feature: **familiars**,
  AI seats that join real rooms over WebSocket, play with either brain, and
  never stall a wave.

Both are hosts around one core: a firewalled protocol consumer whose brains
choose among **server-enumerated legal actions** (decision frames) and race a
latency budget with an instant bot fallback.

## The firewall (read this before adding a dependency)

This crate shares exactly one crate with the server: `protocol/`. No server
domain type may appear here — the view model is rebuilt from received wire
messages and **structurally cannot hold a secret** (no boiling-point field
outside the two sanctioned disclosures, no opponents' hands, no deck
realization). Even the in-process transport carries **encoded MessagePack
frames** through the production codec; domain objects never cross.

The one sanctioned exception is the opt-in `harness` cargo feature, which links
the server **only** so the batch-runner host can boot headless games — every
seat still talks to them through byte frames. CI proves the default build is
clean: `make firewall-check` fails if `boiling-point-server` appears in the
default-features dependency graph.

## Brains

Both brains implement one `Brain` trait (`decide(view, frame) → answer`) and
are interchangeable per seat. Every answer is validated against the frame's
enumerated legal set before submission; anything illegal or late falls back.

| | Bot brain (`bot/`) | Agent brain (`agent/`) |
|---|---|---|
| Decisions | deterministic heuristics over the frame | one tool-forced Claude Messages API call per decision |
| Settings | archetype (`cautious` / `aggressive` / `political` / `random`), blunder `epsilon`, seed | model, persona, difficulty (`relaxed` / `standard` / `sharp`), latency budget, fallback archetype, spend caps |
| Cost / latency | zero, microseconds | real money, network latency |
| Reproducible | yes — all randomness from the seeded RNG tree (root → game → seat) | no — explicitly outside the reproducibility guarantee |
| Failure posture | n/a (it *is* the floor) | degrades to its internal bot on cap-reached / API error / malformed answer; the seat-level budget race is the liveness backstop |

Agent auth is an API key in `ANTHROPIC_API_KEY` (direct Messages API; no
sidecar). Hard USD spend caps apply **per game** and **per process** (shared
across every agent seat in the process); a reached cap degrades the seat to
the bot brain rather than exceeding the cap or abandoning the seat. Prompts
are assembled exclusively from the seat's secret-free view and its bounded
observed-events transcript (drop-oldest compaction; growth is measured).

`latency_probe` measures one isolated decision against a fixture frame — use it to
pick a model for a wave-timer budget before trusting it with a seat:

```sh
ANTHROPIC_API_KEY=… cargo run -p boiling-point-ai-client --bin latency_probe -- --model claude-haiku-4-5
```

## Harness mode — night brews

At-scale unattended balance runs ("the coven ran 1000 night brews on seed 0"):

```sh
# The CI-sized pinned sample (also `make harness-sample`):
cargo run -p boiling-point-ai-client --features harness --bin balance_tester -- \
    --games 200 --seed 424242 --report target/harness-sample

# A matrix sample spec:
cargo run -p boiling-point-ai-client --features harness --bin balance_tester -- \
    --spec my-sample.toml --report target/my-sample
```

```toml
# my-sample.toml — which cells to run, how many games per cell
root_seed = 7

[[cells]]
name = "aggressive-vs-field"
games = 500
seats = [
    { brain = "bot", archetype = "aggressive" },
    { brain = "bot", archetype = "cautious", epsilon = 0.1 },
    { brain = "bot", archetype = "political" },
    { brain = "bot", archetype = "random" },   # keep the baseline in samples
]
```

Reports land as markdown (eyeballs) + JSON (diffs), keyed to a fingerprint of
the content config: explosion rate vs the 40–50% band, detonator distribution,
per-label win shares, the Peek economy, fold-to-safety, freezes, waves/cards/
pot per round, and per-seat fallback rates. Smells flag dominant cells, labels
indistinguishable from the random baseline, off-band explosion rates, and
freezes. Bot-only in-process runs are **byte-reproducible from the root seed**;
agent seats (which require `--allow-agents` — the no-accidental-spend gate) and
WebSocket runs are marked non-reproducible.

The `brewer` and `deck_archetype` seat axes are declared in the spec schema but
**rejected until `boom2-brewers` / `boom2-apothecary` land** their decision
kinds — a spec never silently runs a different experiment than written.

## Seat-filler mode — summoning familiars

Player-facing, bots present as the witch's **familiar** (Apothecary Ink
flavor) so nobody mistakes them for a human, and the name pool can't collide
with the pantry-bucket vocabulary (Sage, Bramble, Honey…). Unnamed seats get
their temperament's pairing automatically; the agent brain presents as the
**Homunculus** — the artificial brewer:

| code id (greppable) | player-facing familiar |
|---|---|
| `cautious` | Timid Toad (familiar) |
| `aggressive` | Brash Salamander (familiar) |
| `political` | Silver-tongued Raven (familiar) |
| `random` | Scatterbrained Moth (familiar) |
| agent brain | Homunculus (familiar) |

Code-level identifiers stay literal by design (constitution §II/§III) — the
flavor lives only on display surfaces.

```sh
# One bot seat into matchmaking (presents as "Silver-tongued Raven (familiar)"):
cargo run -p boiling-point-ai-client --bin familiar_summoning -- \
    --server ws://127.0.0.1:8080/ws --archetype political

# A table's worth of seats from a config:
cargo run -p boiling-point-ai-client --bin familiar_summoning -- --config seats.toml
```

See the doc comment in `src/bin/familiar_summoning.rs` for the config schema (per seat:
entry by invite code or enqueue, brain + settings, games to play, emote
palette). One process runs any number of seats concurrently, each with its own
connection. Pre-game decisions are **Delegated** to the brain by default (the
synergy hunt is the agent's skill expression once drafting lands). Transient
disconnects reconnect with the held session token; permanent failure exits
that seat cleanly without disturbing the others.

## Testing

Per constitution v2.1.1 §II this crate carries **minimal unit tests only** —
component-level coverage of its own surfaces (secret boundary, legal-set
adherence, policy routing, budget/fallback races, schema derivation, spend
caps, harness determinism/parity). The standing headless e2e suite lives
server-side and drives this client as its instrument.

```sh
cargo test -p boiling-point-ai-client                  # core (no server linked)
cargo test -p boiling-point-ai-client --all-features   # + harness validation, seat-filler e2e
make firewall-check                                    # the dependency firewall
```
