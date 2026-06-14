# Identity, Rating & Skill-Based Matchmaking (`boom2-identity`)

The v2 **identity stack**, layered additively over v1's anonymous sessions and
the table-filling queue (the seams documented in
[02_server-infrastructure.md](02_server-infrastructure.md) and
[05_roadmap.md — Identity](../05_roadmap.md)). Authoritative requirements live in
the change [`openspec/changes/boom2-identity`](../../openspec/changes/boom2-identity/);
this is the human-facing rationale and the Principle-IV tuning record.

## What landed

- **Persistent accounts** (`server/src/lobby/accounts.rs`) — an optional upgrade
  from an anonymous session, in **three kinds**: a **device-bound anonymous**
  account (a durable token, no credentials — the lightest path), a **passkey**
  account (a pseudonym + a WebAuthn credential, no password and no password
  backup — portable), and an **OAuth** account (**Google, Apple, Microsoft,
  Discord** — portable). A device/passkey upgrade *binds* the existing player
  UUID; an OAuth sign-in adopts the provider's own account. Anonymous play stays
  the default. The in-memory store is authoritative at runtime and writes
  through to PostgreSQL when a database is configured (hydrated on boot), so the
  e2e suite needs no DB.
  - **Privacy-first:** accounts carry **no email and no real name**. OAuth
    requests **no profile scopes** and reads only the stable subject; every
    account is auto-assigned a unique, themed pseudonym (e.g. `simmering-ruby-
    newt`) that the player may change **once**. An account is bound to **one**
    identity — there is **no provider linking and no conflicts** (same provider
    identity ⇒ same account; a new identity ⇒ a fresh account).
  - **Deletion:** players may delete their account — identity-only erasure (the
    account, its rating, and its player record); shared anonymous game replays
    are immutable records and are left intact. The server records each account's
    **last-login** timestamp.
  - **Credential verification (`server/src/lobby/verifiers.rs`):** the OIDC
    providers (Google/Apple/Microsoft) are verified by validating the **id
    token** (JWT) against the provider's JWKS with `jsonwebtoken` — Apple has no
    userinfo endpoint, so this is the only option; Discord is verified by a
    `users/@me` call. Everything sits behind verifier **seams** so the headless
    tests use stubs (no network). The passkey WebAuthn ceremony (`webauthn-rs` +
    a server-issued challenge) is client-coupled and lands with the web client
    (`adopt-pixi-client`); the account model and seam ship here. `openidconnect`
    is the heavier full-flow alternative we did not need.
- **FFA rating** (`server/src/rating.rs`) — a **Weng-Lin** Bayesian online rating
  (the Bradley-Terry full-pair model; the TrueSkill family, *not* 2-player Elo),
  so one 4-player finishing order updates all four ratings in a single consistent
  computation. Attached to **accounts only**; anonymous participants neither gain
  nor affect durable rating. Updated server-side from **finished** games; an
  incomplete game (no declared winner) does not move ratings.
- **Skill-based matchmaking** (`server/src/lobby/policy.rs`) — a swappable
  `MatchPolicy` on the *same* anchor-and-fill queue. `FirstCome` reproduces v1
  exactly (the default and the unrated fallback); `SkillBased` seats the
  tightest-rated four when everyone in the decision is rated, and falls back to
  first-come the moment any participant is unrated. The queue's shape (exactly
  four, member/guest rules) is unchanged.

The wire vocabulary is protocol **v8** (`protocol/src/account.rs`): an optional
`AccountCredential` on entry messages signs in; `CreateDeviceAccount` / `LinkOAuth`
upgrade in-session; `AccountEstablished` and `RatingUpdate` are the private
readouts. The conservative rating shown to players is `round((mu − 3·sigma)·40) +
1000` — a skill estimate discounted by its own uncertainty, so it only firms up
with games.

## Rating model parameters (`[needs playtesting]`)

The Weng-Lin defaults are the standard TrueSkill/openskill starting points,
validated for this game by the rated-population simulation below. They remain
hypotheses until live data (Principle IV); no number here is sacred.

| Parameter | Value | Meaning |
|---|---|---|
| `mu0` | 25.0 | fresh skill mean |
| `sigma0` | 25/3 ≈ 8.333 | fresh uncertainty |
| `beta` | `sigma0`/2 ≈ 4.167 | per-game performance noise |
| `tau` | `sigma0`/100 ≈ 0.083 | dynamics (keeps a settled rating responsive) |
| `kappa` | 0.0001 | variance-shrink floor |
| provisional | games < 5 | the "still settling" flag |

## Principle-IV validation — the rated-population simulation

The §IV instrument is the AI client's `rating_sim`
([`clients/ai/src/harness/rating_sim.rs`](../../clients/ai/src/harness/rating_sim.rs),
binary `rating_sim`, harness feature). It exercises the **real** server rating
model and `SkillBased` policy against **synthetic** finishing orders: a population
carries a hidden true skill, tables form by the production policy, and each
table's finish is sampled from those true skills (a Thurstonian model:
performance = true skill + N(0,1), best performance finishes first). Thousands of
seeded games run in milliseconds and the results are deterministic.

### Convergence + match quality (200 players, 20 000 games, seed default)

| metric | skill matching | first-come (control) |
|---|---|---|
| Spearman (final rating vs hidden true skill) | **0.990** | 0.988 |
| mean within-table true-skill spread | **0.736** | 2.106 |
| cold-start games (strong newcomer → above median) | 3 | 2 |

The rating recovers the true skill order near-perfectly (Spearman ≈ 0.99), and a
genuinely strong newcomer climbs above the population median within a couple of
games — the wide fresh `sigma0` makes early results move the rating fast.

### Match quality vs queue depth (the wait trade-off)

`SkillBased` seats the tightest four of the *currently queued* pool, so match
quality improves with how many players are waiting to choose from — which is the
quality-vs-wait trade-off in one number (120 players, 6 000 games):

| choosable pool | mean within-table spread (skill) | (control) |
|---|---|---|
| 4 | 2.200 | 2.200 |
| 8 | 0.806 | 2.201 |
| 16 | 0.474 | 2.194 |

At a pool of 4 the policy has no choice (it seats all four), so skill matching
equals first-come — exactly the boundary behaviour the spec wants. Each extra
waiter tightens the table; convergence (Spearman ≈ 0.99) is unaffected by pool
size. **Tuning takeaway:** the skill policy needs no tolerance knob — seating the
tightest four already degrades gracefully to first-come as the queue thins, and a
deeper queue buys tighter matches at the cost of wait. The wait cap stays a
deployment knob (`[needs playtesting]` against real queue depth).

Reproduce: `cargo run -p boiling-point-ai-client --features harness --bin rating_sim`.
