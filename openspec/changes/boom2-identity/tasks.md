## 1. Persistent accounts (the unlock)

- [x] 1.1 Add an account store + the account→player-UUID link to the schema (backward-compatible with anonymous records).
- [x] 1.2 Implement the **device-bound anonymous** account (durable token, no credentials) as the lightest path.
- [x] 1.3 Implement **OAuth** sign-in (Google/Discord): provider integration, token handling, account resolution.
- [x] 1.4 Implement the anonymous→account **upgrade** that binds the existing player UUID; keep anonymous play the default.

## 2. Player rating

- [x] 2.1 Add the rating model (Weng-Lin/TrueSkill) and per-account rating storage.
- [x] 2.2 Compute rating updates from authoritative finished-game results (full finishing order); define the incomplete-game rule.
- [x] 2.3 Apply updates only to accounts; leave anonymous participants unrated.

## 3. Skill-based matchmaking

- [x] 3.1 Make the auto-match queue's matching policy pluggable (default = v1 first-come anchor-and-fill).
- [x] 3.2 Add the skill-based ordering policy used when participants are rated; preserve exactly-4 and the guest/member rules.
- [x] 3.3 Fall back to first-come for unrated play.

## 4. Protocol & clients

- [x] 4.1 Add account/auth and rating-display messages to the `protocol` crate; regenerate client wire types.
- [x] 4.2 Clients: account create/link/sign-in flow and a rating readout; anonymous remains one-tap.

## 5. Balance & validation (Principle IV)

- [x] 5.1 Bot-harness rated-population simulation: rating convergence, cold-start behavior, and match-quality vs queue wait.
- [x] 5.2 Tune the rating parameters and skill-match tolerance; record results.
