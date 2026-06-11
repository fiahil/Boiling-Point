## Why

v1 runs on **anonymous per-session tokens** with **no persistent accounts and no rating** ([docs/05_roadmap.md — Identity](../../../docs/05_roadmap.md), [02 §14](../../../docs/02_game-design.md)). That is enough to fill tables, but it leaves no cross-game identity to hang progression, rating, or skill-based matchmaking on. This change adds the v2 **identity stack** — persistent accounts (the unlock), an FFA rating, and a skill-based matching policy — as **additive** layers over the existing seams (anonymous auth and the table-filling queue), per the roadmap ordering.

## What Changes

- **Persistent player accounts** — an optional, additive upgrade from an anonymous session to a durable identity (lightweight **device-bound anonymous** account or **OAuth**, e.g. Google/Discord). The anonymous path stays the default; no account is required to play.
- **Player rating** — a **multiplayer free-for-all** rating model (**Weng-Lin / TrueSkill**, *not* 2-player Elo), attached to accounts and updated from game results. (The v1 schema deliberately carries no rating column — this adds it.)
- **Skill-based matchmaking** — when ratings exist, the existing auto-match queue MAY order/group by rating. The queue's *shape* is unchanged (anchor-and-fill, exactly-4); only the **matching policy** gains a skill dimension.
- **Ordering (phased in tasks):** accounts → rating → skill-based matchmaking.

## Capabilities

### New Capabilities

- `player-accounts` — durable cross-game identity: the additive anonymous→account upgrade, device-bound-anonymous and OAuth paths, and the attachment point for rating and (later) profiles.
- `player-rating` — an FFA multiplayer rating (Weng-Lin/TrueSkill) attached to accounts and updated from finished games.

### Modified Capabilities

- `lobby-and-matchmaking` — **Anonymous Session Authentication** gains an optional account-upgrade path (anonymous stays the default); **Auto-Match Queue** gains an optional skill-based ordering when ratings exist (same queue shape).

## Impact

- **Auth/identity:** an account store + token model; OAuth integration is the heaviest new dependency. Anonymous sessions remain and can be linked to an account.
- **Persistence:** ratings and the account→player link join the schema; the v1 `persistence-and-observability` post-game write now also updates ratings. `player profiles` (career stats) are a **separate later** roadmap item that builds on accounts — out of this change.
- **Matchmaking:** the queue’s policy becomes pluggable; default FIFO/anchor-fill stays for unrated play, skill-ordering applies when both sides are rated.
- **Out of scope:** horizontal scaling, profiles UI, and any change to the game loop. Rating tuning (the model’s parameters and convergence) is a Principle-IV data task.
