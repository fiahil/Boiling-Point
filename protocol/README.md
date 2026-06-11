# protocol

The **wire waist** of Boiling Point: the message types and codec that every client
and the server share, and nothing more. It holds **no game logic and no secrets** —
it's pure DTOs plus serialization, so it can be a dependency of untrusted clients
without leaking anything.

## What's here

| Module | Contents |
|---|---|
| `client.rs` | `ClientMessage` — everything a client can send (`CreateRoom`/`JoinRoom`/`EnqueueMatch`, `CommitCard`, `CommitPass`, `LockIn`, `Emote`, `Heartbeat`) and `PROTOCOL_VERSION`. |
| `server.rs` | `ServerMessage` — everything the server emits (`RoomJoined`, `GameStarting`, `YourHand`, `WaveOpened`, `WaveResolved`, `Depile`, `RoundScored`, `Explosion`, `GameOver`, …) and `ErrorCode`. |
| `ids.rs` | Newtype ids: `RoomCode`, `PlayerId`, `CardId`, `EmoteId`. |
| `vocab.rs` | Shared enums/DTOs: `Color`, `EffectKind`, `ModifierKind`, `CardView`, `HandCard`. |
| `codec.rs` | `encode`/`decode` — **MessagePack on the wire**, with a JSON fallback for debugging. |

## Design rules

- **No secrets ever cross here.** A `ServerMessage` carries only player-permitted
  data; the server decides what each player sees (e.g. `YourHand` is private,
  contribution counts are public, the boiling point is disclosed only on a Peek or an
  explosion depile).
- **Versioned handshake.** The first (entry) client message carries
  `protocol_version` so the server rejects incompatible clients before sharing state.
- TypeScript wire types for the web client (`clients/web/`) are **generated from
  this crate** (the `adopt-pixi-client` typegen step) — never hand-mirrored. (The
  retired `archive/agent-harness/` hand-mirrored them in v1.)

## Build / test

```sh
cargo test -p boiling-point-protocol
```

See [`docs/03_architecture/01_overview.md`](../docs/03_architecture/01_overview.md) for
how this crate sits at the center of the system.
