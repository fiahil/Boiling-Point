## 1. Prerequisites (companion change)

- [x] 1.1 Confirm `server-otel-control-plane` is promoted (or folded into
      `server-release-1`): `tracing`→OTEL bridge, the D3 span tree emitted with
      secret attributes, redacting OTLP exporter, in-process span-lifecycle hook,
      and command primitives on the room registry
- [x] 1.2 Author the versioned span-schema contract doc (span names, hierarchy,
      public attributes, and the authoritative set of secret attributes)

## 2. admin-span-projection

- [x] 2.1 Define the projection's data structures: open-span registry (keyed by
      room/game/round/wave), rolling aggregates, bounded replay buffer
- [x] 2.2 Implement the span-lifecycle consumer (`on_start`/`on_close`) that feeds
      the projection without blocking emission (drop/coalesce under load)
- [x] 2.3 Populate the open-span registry from start/end; derive current phase
      from the deepest open child span
- [x] 2.4 Fold completed spans into unsampled rolling aggregates (explosion rate,
      durations, cards/round, dominant-color, timeout, reconnection rates)
- [x] 2.5 Implement the bounded replay buffer with oldest-eviction
- [x] 2.6 Make the consumer tolerant of unknown spans/attributes (schema-versioned)
- [x] 2.7 Reap registry entries whose span-end was missed (tie to stuck-room age)
- [x] 2.8 Tests: live registry add/remove, unsampled aggregate accuracy under
      simulated sampling, buffer bound + eviction, read-only invariant

## 3. admin-auth

- [x] 3.1 Implement an operator auth mechanism separate from player session tokens
- [x] 3.2 Implement role-based gating (observer vs. elevated; reveal + control
      require elevated)
- [x] 3.3 Serve the admin API on isolated routes/port; reject the player wire and
      refuse to upgrade a player connection to admin
- [x] 3.4 Tests: player token denied; observer denied reveal/control; elevated
      granted; player-WS cannot reach admin endpoints

## 4. admin API (read + command transport)

- [x] 4.1 Read endpoints over the projection (fleet/room list, room detail,
      reveal, replay) with SSE/WebSocket for live updates
- [x] 4.2 Command endpoints (reload, toggle, room lifecycle) wired to the server
      command primitives
- [x] 4.3 Emit an audit span per command (operator, action, target, outcome)

## 5. room-inspector

- [x] 5.1 Live room/session/queue listing from the open-span registry, updating on
      lifecycle changes
- [x] 5.2 Hidden-state reveal from open-span attributes (boiling point, commits,
      hands, volatility, modifiers); any authenticated operator, admin-channel-only
      (never a player connection)
- [x] 5.3 Handle reveal when no round is open (report "no round in progress")
- [x] 5.4 Stuck/anomalous room detection (over-age open wave/round, error spans)
- [x] 5.5 Per-game replay from the buffer (wave by wave); handle evicted games
- [x] 5.6 Tests: live updates, reveal never served to a player, stuck-room flag

## 6. balance-dashboard

- [x] 6.1 Stand up Grafana with Prometheus as the data source for balance panels
- [x] 6.2 Build panels: explosion rate (vs ~30–40% target), durations,
      cards/round, dominant-color, timeout, reconnection, reshuffle frequency
- [x] 6.3 Embed Grafana behind admin auth (signed embed / same-origin)
- [x] 6.4 Verify figures derive from the unsampled source (projection/Prometheus),
      not a sampled trace

## 7. admin web app (thin custom UI)

- [x] 7.1 App shell + admin auth flow against the admin API
- [x] 7.2 Fleet overview + live room list (subscribe to live updates)
- [x] 7.3 Room detail + godmode reveal panel (elevated role)
- [x] 7.4 Live activity feed (explosions, game-overs, deathmatches, rejections,
      reconnects) from the span stream
- [x] 7.5 Per-game replay viewer
- [x] 7.6 Control panel (reload, toggles, room lifecycle) with confirmation via
      telemetry (room leaving the live registry, config version change)
- [x] 7.7 Embed the Grafana balance dashboard

## 8. Security & redaction verification

- [x] 8.1 Export-boundary redaction removed (the trace backend is trusted and
      operator-only); the tested control is the player-wire boundary — sensitive
      state is unreachable on any player connection (see 8.2)
- [x] 8.2 Test that the reveal is unreachable over any player connection
- [x] 8.3 Test that the read projection cannot mutate game or config state

## 9. End-to-end validation

- [x] 9.1 Run a bot-harness session; confirm the inspector shows live rooms, the
      reveal matches authoritative state, and balance figures match the bot
      harness's own counts (unsampled accuracy)
- [x] 9.2 Confirm a kill-room command is reflected by the room leaving the live
      view (loop closes through telemetry)
