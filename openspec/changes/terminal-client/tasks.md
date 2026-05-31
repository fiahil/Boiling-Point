## 1. Crate Setup & Net Layer

- [x] 1.1 Add `tui-client/` to the cargo workspace; depend on `protocol/`, add `ratatui`, `crossterm`, `tokio`, `arboard` *(clipboard implemented via OSC 52 instead of `arboard` — zero native deps, cross-terminal; see `clipboard.rs`)*
- [x] 1.2 Terminal lifecycle: enter raw mode + alternate screen on start; restore on quit and via a panic hook (`tui-client-shell`)
- [x] 1.3 WebSocket task: connect, decode `ServerMessage` via `protocol/`, forward on a channel; encode `ClientMessage` intents outbound
- [x] 1.4 Main loop: `select!` over input events, incoming messages, and a ~20–30 fps render tick

## 2. View Model & Shell

- [x] 2.1 Define the client's player-visible view model (no boiling point, no opponent hands, no draw deck)
- [x] 2.2 `apply(ServerMessage)` reducer building view-model state from messages only
- [x] 2.3 Protocol handshake: send `JoinRoom` with `protocol_version` + identity; handle `RoomJoined` and version-mismatch `Error`
- [x] 2.4 Phase-driven screen router (lobby / round-start / playing / depile / scoring / deathmatch / game-over)
- [x] 2.5 Responsive layout + minimum-size (80×24) prompt; handle resize events
- [x] 2.6 Color+letter player palette with truecolor→ANSI-16 capability detection

## 3. Lobby

- [x] 3.1 Entry menu: quick match / create room / join by code + display-name capture (`tui-lobby`)
- [x] 3.2 Create-room flow: show invite code; copy shareable join string to clipboard
- [x] 3.3 Join-by-code flow with unknown-code error feedback
- [x] 3.4 Seat roster: 4 seats (color/name/occupancy), `N/4`, hostless auto-start messaging
- [x] 3.5 Quick-match waiting state → seat roster on match *(idle-timeout countdown not shown — no wire field carries it; PROTOCOL GAP)*

## 4. Round Play — Core

- [x] 4.1 Round-start reveal: new modifier, cumulative stack, refill-to-5 with new-card marks *(boiling-point range shown qualitatively by modifier direction; numeric magnitudes are server-side content and never cross the wire)*
- [x] 4.2 Opaque cauldron render: no volatility cue; total count; contributor-tagged face-down chips
- [x] 4.3 Opponent panels: color/name/score/contributed count *(opponent hand size not shown — no wire field carries it; PROTOCOL GAP)*
- [x] 4.4 Hidden changeable commit: select 0/1 card or pass, re-pickable until close, send latest only, render the timer, pass-as-lockout warning, stop input at close
- [x] 4.5 Wave-resolution reveal: who played / who passed / new count; Recall as a contribution-count drop
- [ ] 4.6 One-player final-wave indicator — DEFERRED: needs either a wire signal or client-side accumulation of locked-out players across waves; not implemented

## 5. Round Play — Effects & Emotes

- [x] 5.1 Recall: private at-commit target prompt over the player's own pot cards *(chosen target cannot be transmitted — `CommitCard` carries no target field; flagged in-UI; PROTOCOL GAP)*
- [x] 5.2 Peek: private `PeekResult` modal; anonymous "someone peeked" notice to others
- [x] 5.3 Expose: public card reveal *(rendered as a reveal toast)*
- [x] 5.4 Keep Dampen / Volatile Surge / Copycat / Double Down / Shield silent until the depile
- [x] 5.5 Preset emote palette + transient display beside sender, sending palette ids and rendering `EmoteBroadcast`

## 6. Reveal & Score

- [x] 6.1 Reverse-order depile: per-card flip (color/points/volatility/effect/owner) with a descending volatility bar; skippable (`tui-reveal-and-score`)
- [x] 6.2 Boiling-point line + crossing-card mark on explosion only; hidden on safe brew
- [x] 6.3 Boom full-screen sequence + shared-loss readout
- [x] 6.4 Round scoring screen: outcome (Domination/Split), per-player deltas + totals, continue
- [ ] 6.5 Deathmatch screens — PARTIAL: the forced-play / no-pass / volatility-only play screen is implemented and snapshot-tested; Detonator-elimination, Shield-redirect cascade, and the deathmatch *trigger* are not rendered — no wire marker carries them (co-champions fall out of `GameOver` winners). PROTOCOL GAP
- [x] 6.6 Game-over standings + back-to-lobby / rematch *(brew-summary stats omitted — not tracked on the wire)*

## 7. Reconnection

- [x] 7.1 Reconnection overlay: grace countdown + auto-pass-while-away note (`tui-client-shell`)
- [x] 7.2 Rebuild from `StateSnapshot` and resume at the reported phase; reflect missed waves as locked-out *(server added `StateSnapshot`; wired in `view.rs`/`app.rs` + snapshot-tested)*

## 8. Debug & Test

- [x] 8.1 Debug overlay: phase/wave, in/out counts, scrolling raw-message log, live view-model JSON (`tui-debug-and-test`) *(RTT not shown — no client-side ping yet)*
- [x] 8.2 Record the received message stream to a replay file (`--record`)
- [x] 8.3 Replay a recorded stream into the view model + renderer with no live server (`--replay`)
- [x] 8.4 Thin in-process mock for interactive manual testing (`--mock`, default)
- [x] 8.5 `TestBackend` snapshot tests for lobby, round-start, playing, depile (safe + explosion), scoring, deathmatch, game-over
- [x] 8.6 Assert snapshots contain no boiling point / opponent-hand contents (secret-boundary regression guard)

## 9. Cross-Change Coordination

- [x] 9.1 Confirm the finalized field shapes in `server-release-1` (`WaveOpened.timer_ms`; `EmoteBroadcast { from, emote }`) and pin the client's decode to them
- [x] 9.2 Replay fixtures cover a wave countdown (`WaveOpened.timer_ms`) and emote send/receive (`EmoteBroadcast`)

## 10. Notes & Discovered Protocol Gaps

The client is verified end-to-end against the **real server** in
`tests/live_server.rs` (in-process, ephemeral port): a real `RoomJoined`
handshake renders the lobby, and four clients drive a full game through the
actual wire to game-over. The remaining server-owned gaps below still block the
two deferred tasks (4.6, 6.5):

- ~~No `StateSnapshot`~~ — RESOLVED: the server added `StateSnapshot`; reconnection resume is wired and tested (7.2). *(Identity continuity across reconnects is still partial — no session token is delivered to the client to resume the same seat; minor.)*
- No Deathmatch/phase marker → the client cannot detect the tiebreaker or render its elimination/cascade announcements (6.5).
- `CommitCard` carries no Recall target → the chosen recalled card cannot be transmitted (5.1).
- No opponent hand-size field (4.3); no idle-timeout field (3.5); no "active players remaining" signal for the one-player final wave (4.6).
- Modifier numeric magnitudes are intentionally server-side content, so the round-start screen shows direction, not a computed numeric range (4.1).

Deviation: clipboard uses OSC 52 (no `arboard`) to keep deps minimal and the build robust (1.1).
