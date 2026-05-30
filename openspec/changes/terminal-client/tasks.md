## 1. Crate Setup & Net Layer

- [ ] 1.1 Add `tui-client/` to the cargo workspace; depend on `protocol/`, add `ratatui`, `crossterm`, `tokio`, `arboard`
- [ ] 1.2 Terminal lifecycle: enter raw mode + alternate screen on start; restore on quit and via a panic hook (`tui-client-shell`)
- [ ] 1.3 WebSocket task: connect, decode `ServerMessage` via `protocol/`, forward on a channel; encode `ClientMessage` intents outbound
- [ ] 1.4 Main loop: `select!` over input events, incoming messages, and a ~20–30 fps render tick

## 2. View Model & Shell

- [ ] 2.1 Define the client's player-visible view model (no boiling point, no opponent hands, no draw deck)
- [ ] 2.2 `apply(ServerMessage)` reducer building view-model state from messages only
- [ ] 2.3 Protocol handshake: send `JoinRoom` with `protocol_version` + identity; handle `RoomJoined` and version-mismatch `Error`
- [ ] 2.4 Phase-driven screen router (lobby / round-start / playing / depile / scoring / deathmatch / game-over)
- [ ] 2.5 Responsive layout + minimum-size (80×24) prompt; handle resize events
- [ ] 2.6 Color+letter player palette with truecolor→ANSI-16 capability detection

## 3. Lobby

- [ ] 3.1 Entry menu: quick match / create room / join by code + display-name capture (`tui-lobby`)
- [ ] 3.2 Create-room flow: show invite code; copy shareable join string to clipboard
- [ ] 3.3 Join-by-code flow with unknown-code error feedback
- [ ] 3.4 Seat roster: 4 seats (color/name/occupancy), `N/4`, hostless auto-start messaging
- [ ] 3.5 Quick-match waiting state → seat roster on match; idle-timeout countdown

## 4. Round Play — Core

- [ ] 4.1 Round-start reveal: new modifier, cumulative stack, refill-to-5 with new-card marks, shifted boiling-point range (`tui-round-play`)
- [ ] 4.2 Opaque cauldron render: no volatility cue; total count; contributor-tagged face-down chips
- [ ] 4.3 Opponent panels: color/name/score/hand-size/contributed count; no live commit signal
- [ ] 4.4 Hidden changeable commit: select 0/1 card or pass, re-pickable until close, send latest only, render the timer, pass-as-lockout warning, stop input at close
- [ ] 4.5 Wave-resolution reveal: who played / who passed / new count; Recall as a contribution-count drop
- [ ] 4.6 One-player final-wave indicator

## 5. Round Play — Effects & Emotes

- [ ] 5.1 Recall: private at-commit target prompt over the player's own pot cards; must complete before close
- [ ] 5.2 Peek: private `PeekResult` modal; anonymous "someone peeked" notice to others
- [ ] 5.3 Expose: public card reveal animation
- [ ] 5.4 Keep Dampen / Volatile Surge / Copycat / Double Down / Shield silent until the depile
- [ ] 5.5 Preset emote palette + transient display beside sender, sending palette ids against `server-release-1`'s `table-talk` relay and rendering `EmoteBroadcast`

## 6. Reveal & Score

- [ ] 6.1 Reverse-order depile: per-card flip (color/points/volatility/effect/owner) with a descending volatility bar; skippable (`tui-reveal-and-score`)
- [ ] 6.2 Boiling-point line + crossing-card mark on explosion only; hidden on safe brew
- [ ] 6.3 Boom full-screen sequence + shared-loss readout
- [ ] 6.4 Round scoring screen: outcome, pot value with modifiers, per-player deltas + totals, continue
- [ ] 6.5 Deathmatch screens: forced 1/wave, no pass, volatility-only; Detonator elimination; Shield-redirect cascade; co-champions
- [ ] 6.6 Game-over standings + brew summary + back-to-lobby / rematch

## 7. Reconnection

- [ ] 7.1 Reconnection overlay: grace countdown + auto-pass-while-away note (`tui-client-shell`)
- [ ] 7.2 Rebuild from `StateSnapshot` and resume at the reported phase; reflect missed waves as locked-out

## 8. Debug & Test

- [ ] 8.1 Debug overlay: RTT, in/out counts, phase/wave, scrolling raw-message log, live view-model JSON (`tui-debug-and-test`)
- [ ] 8.2 Record the received message stream to a replay file
- [ ] 8.3 Replay a recorded stream into the view model + renderer with no live server
- [ ] 8.4 Thin mock/in-process server for interactive manual testing
- [ ] 8.5 `TestBackend` snapshot tests for lobby, round-start, playing, depile (safe + explosion), scoring, deathmatch, game-over
- [ ] 8.6 Assert snapshots contain no boiling point / opponent-hand contents (secret-boundary regression guard)

## 9. Cross-Change Coordination

- [ ] 9.1 Confirm the finalized field shapes in `server-release-1` (`wire-protocol` wave-open timer budget; `table-talk` `EmoteBroadcast`) and pin the client's decode to them
- [ ] 9.2 Add replay fixtures covering a wave countdown and emote send/receive end-to-end
