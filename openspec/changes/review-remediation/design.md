## Context

The engine (`game/*`) is deeply tested and deterministic; the **async** game loop
(`session.rs::run_game`) that actually drives live games re-derives the same
orchestration and is only coarsely covered by transport integration tests. Three of
the four findings here are about closing the gap between "the tested thing" and "the
shipping thing"; F1 is a direct §I contract gap. None requires new game rules.

## Goals / Non-Goals

**Goals:**
- Make invalid in-wave actions observable to the client as errors, with no state change
  and no information leak (F1).
- Make the secret-routing safety rail load-bearing, plus a whole-game no-leak test (F3).
- Have one orchestration core behind both the sync and async paths, or equivalent test
  coverage of the async path (F2).
- Remove production `unwrap()`s and the doc nit (F5).

**Non-Goals:**
- Persistence/replays (the `persistence-and-replays` change).
- The room→group rename or any lobby model change (the `group-model` change).
- New game mechanics, cards, or balance changes.

## Decisions

- **D1 — Emit errors, don't silently drop (F1).** The constitution §I is explicit:
  "Invalid actions receive an error response with no state change." The
  anti-leak concern (an error revealing hidden state) is handled by the fact that
  `NotYourCard`/`LockedOut`/`WrongPhase`/`InvalidEmote` carry only the *reason*, never
  pot/volatility/boiling-point state. Resolve the lobby-vs-wave emote inconsistency the
  same way (errors in both).
- **D2 — Single orchestration core (F2).** Prefer driving `Game` (the tested sync
  runner) from `run_game` via a network-backed `Decider`/`DeathmatchDecider` that
  awaits real commits and surfaces the broadcasts the wire needs, over maintaining two
  parallel loops. If full convergence is too invasive for one change, the fallback is
  engine-level tests asserting the async path matches the sync path for a fixed seed.
- **D3 — Enforce routing through `Outbound`/`Audience` (F3).** Replace the hand-written
  `broadcast`/`send_to` helpers with construction of `Outbound`, so
  `is_private_only()` guards every send; add a whole-game test that decodes every
  broadcast frame and asserts no secret field (boiling point, opponents' hands, deck)
  ever appears outside a legitimate private/explosion disclosure.

## Risks / Trade-offs

- **[Risk] F2 convergence is the riskiest edit** → it touches the live game loop. Keep
  it behind the green test suite; if scope balloons, ship D2's fallback (async-path
  tests) and split convergence into a follow-up. The other findings are independent and
  low-risk.
- **[Risk] F1 error timing could be a faint side-channel** (an immediate error tells you
  *your own* action was invalid) → acceptable: it concerns only the acting player's own
  move legality, never others' hidden state, and matches the §I contract.

## Migration Plan

Server-internal; no wire-breaking change (clients already handle `Error`). Ship behind
`make check`. Order: F5 (mechanical) → F1 (error replies + test) → F3 (routing + leak
scan) → F2 (convergence, largest). Each step independently mergeable.

## Open Questions

- Does full F2 convergence fit one change, or should it be staged behind the async-path
  tests? (Resolve during implementation against the size of the `Decider` seam.)
