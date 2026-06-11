# balance-analysis — delta

## REMOVED Requirements

### Requirement: Seeded Deterministic Batch Runner
**Reason**: The balance-analysis layer lived in the bot harness, retired to `archive/bot-harness/` (change `retire-v1-harnesses`; constitution v2.0.0 §IV requires reinstating at-scale runs before large balance reworks ship).
**Migration**: None — no live consumer. Revive `archive/bot-harness/` and restore this capability from this archived change's delta files.

### Requirement: Balance Statistics Aggregation
**Reason**: Retired with the bot harness (`archive/bot-harness/`).
**Migration**: None — restore on harness revival.

### Requirement: Degenerate-Strategy and Balance-Smell Detection
**Reason**: Retired with the bot harness (`archive/bot-harness/`).
**Migration**: None — restore on harness revival.
