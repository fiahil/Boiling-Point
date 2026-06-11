# tui-client-shell — delta

## REMOVED Requirements

### Requirement: Protocol Handshake On Connect
**Reason**: The TUI reference client is retired to `archive/tui-client/` (change `retire-v1-harnesses`; constitution v2.0.0).
**Migration**: None — no live consumer. The web client (`clients/web/`, change `adopt-pixi-client`) is the forward renderer; revive from `archive/tui-client/` and restore this capability from this archived change's delta files if needed.

### Requirement: Phase-Driven Screen Routing
**Reason**: Retired with the TUI (`archive/tui-client/`).
**Migration**: None — restore on TUI revival.

### Requirement: Player-Visible View Model Only
**Reason**: Retired with the TUI (`archive/tui-client/`).
**Migration**: None — restore on TUI revival.

### Requirement: Responsive Layout And Minimum Size
**Reason**: Retired with the TUI (`archive/tui-client/`).
**Migration**: None — restore on TUI revival.

### Requirement: Reconnection Overlay
**Reason**: Retired with the TUI (`archive/tui-client/`).
**Migration**: None — restore on TUI revival.

### Requirement: Clean Terminal Teardown
**Reason**: Retired with the TUI (`archive/tui-client/`).
**Migration**: None — restore on TUI revival.
