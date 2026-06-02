## Why

The constitution makes **Claude-as-player the second of three first-class testing layers** (Principle II): Layer 1 (`bot-balance-harness`) runs thousands of headless games for statistical balance, Layer 3 (`terminal-client`) renders the game, but neither lets a solo developer *feel* the game — especially its hidden-information tension and its political layer — without recruiting four humans. This change adds Layer 2 as a focused **v0**: an intelligent, tunable opponent that fills seats so the game can be playtested by one person, and `terminal-client` R2 already anticipates this harness landing.

## What Changes

- Add an **`agent-harness/` package — TypeScript/Node, _not_ a cargo crate** (a deliberate deviation from the workspace sketch in `CLAUDE.md`; justified in design: the Claude Agent SDK ships only for TS/Python, and subscription billing fits a tool a single developer runs constantly). It connects over the **same public WebSocket protocol a real client uses** (`server-release-1`'s `protocol/` catalog and `JoinRoom` version handshake), receiving only player-permitted information.
- Drive Claude through the **Claude Agent SDK** with **subscription (OAuth) auth**, exposing both game actions (commit / pass / lock-in / effect target-pick / send-emote) and analytical capabilities to Claude as **in-process MCP tools**.
- Make **difficulty exactly the granted tool set** (`allowedTools`): capability tools are gated by registration, so withholding the card-history tool genuinely removes card-counting. v0 ships a minimal set — actions plus reveal-history — with the full capability ladder deferred.
- Add **persona-driven play**: optional named archetypes (Gambler / Turtle / Bandwagoner / Trickster) that bias playstyle and select **preset emotes** (the game's only comms channel per `table-talk`), letting a solo developer probe the political layer. A dormant **blunder-injection** knob gives a reliable difficulty lever if persona alone plays too well.
- Meet the wave timers **without any server "harness mode"**: deliberate during the inter-wave gap, lock in at wave-open, and fall back to a fast local action if Claude is late.
- Run **one process per seat** via a CLI (`bp-bot --room … --difficulty … --persona …`); the developer joins as the remaining seat.

## Capabilities

### New Capabilities
- `agent-player`: the Claude-driven protocol client — a player-visible view model built only from received messages, a long-lived Agent SDK session, the in-process MCP tool registry, difficulty-as-`allowedTools`, the thin-per-turn-context gating rule, timely commitment with a local fallback, and the per-seat CLI process.
- `agent-personas`: optional archetype personas that bias playstyle, drive preset-emote selection as the table-talk channel, and the dormant epsilon blunder-injection difficulty knob.

### Modified Capabilities
- _None_ — this change consumes `server-release-1`'s protocol and `table-talk` palette without changing their requirements. (One *optional* future server ask — selecting the JSON-fallback wire mode at handshake for debuggability — is noted in research/design but is **not** a dependency: the harness uses MessagePack in JS by default.)

## Impact

- **Depends on `server-release-1`**: the `protocol/` message catalog, the `JoinRoom`/`protocol_version` handshake, the wave-open timer budget, and the `table-talk` preset-emote palette, plus a runnable server (or an in-process server the harness can point a socket at).
- **New code:** an `agent-harness/` Node package (net layer, player-visible view model, in-process MCP tools, difficulty + persona config, CLI). It lives **outside** the cargo workspace.
- **New dependencies:** `@anthropic-ai/claude-agent-sdk`, `zod` (tool schemas), `ws` (WebSocket client), protocol types generated from the Rust `protocol/` crate via `ts-rs`, and optionally `@msgpack/msgpack`.
- **Relation to `terminal-client` R2:** because this harness is TypeScript, it does **not** trigger the anticipated extraction of a Rust `client-core` crate — it re-implements its narrow player-visible view model in TS from the wire DTOs.
- **Billing:** subscription OAuth; the **Agent SDK monthly credit launching 2026-06-15** covers individual playtesting use and stops it counting against interactive Claude Code limits.
