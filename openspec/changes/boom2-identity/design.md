## Context

v1 deliberately keeps identity minimal — anonymous session tokens, a table-filling queue, no rating ([02 §14](../../../docs/02_game-design.md)) — with the seams documented for v2 ([05_roadmap.md](../../../docs/05_roadmap.md)). This change fills those seams **additively**: accounts are the unlock, rating attaches to accounts, skill-based matchmaking is a policy on the existing queue.

## Goals / Non-Goals

**Goals**
- Persistent accounts (device-bound anonymous *or* OAuth) as an optional upgrade; anonymous stays default.
- An FFA multiplayer rating (Weng-Lin/TrueSkill) attached to accounts.
- A skill-based matching policy layered on the existing queue without changing its shape.

**Non-Goals**
- **Player profiles / career-stats UI** (a separate roadmap item that depends on accounts).
- Horizontal scaling; any game-loop change; leaderboards.
- Final rating parameters (a Principle-IV tuning task).

## Decisions

### D1: Accounts are the unlock, and they are additive

Anonymous sessions remain the default and the fallback. A device or passkey account *binds* the existing player UUID rather than replacing it, so upgrading never disrupts a session; an OAuth sign-in adopts the provider's own account (find-or-create). *Alternative rejected:* mandatory accounts — breaks the "join by invite link, play in seconds" ethos.

### D1a: Privacy-first identity (review)

Accounts carry **no email and no real name**. Three kinds: **device-bound** (durable token), **passkey** (pseudonym + WebAuthn, no password, no password backup), and **OAuth** (Google/Apple/Microsoft/Discord). OAuth requests **no profile scopes** and reads only the stable subject; every account is auto-assigned a unique, themed pseudonym, changeable **once**. An account is bound to **one** identity — no provider linking and **no conflicts** (same provider identity ⇒ same account; a new identity ⇒ a fresh account). Players may **delete** their account (identity-only erasure: account, rating, player record; shared anonymous replays are immutable and left intact). The server records each account's **last-login** timestamp. *Library:* OIDC id-tokens (Google/Apple/Microsoft) are verified with `jsonwebtoken` + the provider JWKS (Apple has no userinfo endpoint); Discord uses a `users/@me` call; passkeys use `webauthn-rs` (the ceremony lands with the web client). `openidconnect` is the heavier full-flow alternative we did not need.

### D2: FFA rating, not Elo

A 4-player result must update four ratings consistently; 2-player Elo cannot. Weng-Lin/TrueSkill-style is the model the v1 schema's *absence* of a rating column was reserving for. *Alternative rejected:* pairwise-Elo approximations — statistically wrong for FFA.

### D3: Skill-based matchmaking is a policy, not a new queue

The v1 anchor-and-fill queue is the seam; v2 swaps the *ordering policy* when ratings exist, keeping the exactly-4 shape and the guest/member rules. Unrated play uses the v1 first-come policy. *Alternative rejected:* a separate ranked queue — duplicates infra before demand justifies it.

### D4: Phasing

accounts → rating → skill-based matchmaking (each task block gates the next), so the change can land and be validated incrementally even though it ships as one section-level change.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I — Server-authoritative** | Accounts, rating computation, and match policy are all server-side; clients present credentials/intents and render results. Ratings derive only from authoritative finished-game results. |
| **II — Agent-driven** | Account/rating/matchmaking are source-defined and harness-drivable; the bot harness can simulate rated populations to validate convergence and match quality. |
| **III — Start simple** | Every layer is **additive over a documented seam** — anonymous auth and the table-filling queue stay; accounts/rating/SBMM layer on. Device-bound-anonymous accounts are offered as the *lightest* path before OAuth. **Rejected simpler alternative:** stay anonymous-only — but then there is nothing to rate or match on, which is the whole point of the v2 identity work. |
| **IV — Playtest-driven** | Rating model parameters and the skill-match tolerance are `[needs playtesting]`; the bot harness simulates rated populations to check convergence, fairness, and queue health before launch. |

## Risks / Migration

- **OAuth is the heaviest dependency** (provider integration, token handling); device-bound-anonymous is the cheaper first step and can ship before OAuth.
- **Schema migration:** add accounts + rating + the account→player link to the existing post-game schema; backward-compatible with anonymous records.
- **Cold-start ratings:** new accounts need a sane default/uncertainty so early matches aren't degenerate — a tuning task.
