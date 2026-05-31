# card-effects Specification

## Purpose
TBD - created by archiving change server-release-1. Update Purpose after archive.
## Requirements
### Requirement: Effect Catalog

The engine SHALL support eight special effects, each adding its own volatility and points to the pot like any card: **Peek** (privately learn the boiling point), **Dampen** (reduce cauldron volatility), **Volatile Surge** (add extra volatility), **Shield** (explosion immunity at the cost of scoring — see its own requirement), **Expose** (reveal one random face-down pot card to the table), **Copycat** (adopt the dominant color from previous waves), **Recall** (retrieve one of the player's own previously-played cards), and **Double Down** (multiply the points of its color already in the pot). Each effect's numeric volatility, points, and copy count are content/config; its behavior is defined here.

#### Scenario: Each effect resolves its defined behavior

- **WHEN** an effect card reveals in a wave
- **THEN** the engine applies that effect's specified behavior and also adds the card's own volatility and points to the pot

#### Scenario: A disabled effect is never resolved

- **WHEN** an effect is disabled in content config
- **THEN** no copies exist in the deck and the engine never resolves that effect

### Requirement: Fixed Effect Resolution Order

When multiple effects land in one wave, the engine SHALL resolve them in this fixed order against a settling pot: (1) cards added — colors, points, volatility land; (2) volatility modifiers — Dampen then Volatile Surge; (3) color/identity effects — Copycat; (4) point effects — Double Down; (5) removal effects — Recall; (6) information effects — Peek and Expose, reading the settled pot and reported last; (7) a single explosion check on the true post-effect total.

#### Scenario: Information effects see the settled pot

- **WHEN** a wave contains both a Dampen and a Peek
- **THEN** the Dampen is applied first and the Peek reports the boiling point relative to the volatility total after the Dampen resolved

#### Scenario: Exactly one explosion check per wave

- **WHEN** several cards and effects resolve in a single wave
- **THEN** the engine performs exactly one explosion check, on the final post-effect volatility total

### Requirement: Pre-Wave Snapshot Semantics

Effects that read or modify prior pot state — Copycat, Double Down, and Recall — SHALL reference the pot as it stood *before the current wave's* additions (a frozen pre-wave snapshot). Multiple modifying effects of the same kind in one wave MUST each apply against that snapshot and sum, making their combined result order-independent.

#### Scenario: Copycat ignores same-wave cards

- **WHEN** a Copycat reveals in the same wave as other colored cards
- **THEN** it adopts the dominant color of the pot from previous waves only, ignoring the cards revealing alongside it

#### Scenario: Two same-color Double Downs sum against the snapshot

- **WHEN** two Double Down cards of the same color reveal in one wave and that color's pre-wave point total is X
- **THEN** each adds X and the color's resulting total is 3X, regardless of which Double Down is processed first

### Requirement: Effects Are Silent By Default

That a special effect was played SHALL be hidden from the table until the end-of-round depile, exactly like any other card — this protects blind volatility (announcing Dampen or Volatile Surge would leak that the pot just cooled or heated). Three effects are exceptions: **Peek** is announced anonymously ("someone peeked", not the value); **Expose** is announced because revealing a card publicly is its function; and **Recall** is inferable because the public per-player contribution count drops by one. Dampen, Volatile Surge, Copycat, and Double Down MUST remain fully silent until the depile.

#### Scenario: A volatility effect leaks nothing

- **WHEN** a player plays Dampen or Volatile Surge in a wave
- **THEN** no message indicates an effect was played, and the change is revealed only at the depile

#### Scenario: Recall shows as a contribution-count drop

- **WHEN** a player plays Recall, pulling one of their own cards from the pot
- **THEN** the broadcast contribution count for that player decreases by one, while which card was reclaimed remains hidden until the depile

### Requirement: Peek Privacy

The player who plays Peek SHALL privately learn the exact boiling point; all other players learn only that someone peeked, never the value.

#### Scenario: Only the peeker sees the value

- **WHEN** a player plays Peek
- **THEN** that player receives a private message with the exact boiling point and every other player receives only a "someone peeked" notification

### Requirement: Shield Round-Scope Bet

Shield SHALL make the player who played it immune to that round's explosion loss, but if the round resolves safely that player forfeits ALL of their scoring for the round. The forfeit applies to the whole round once Shield is played, and Shield still contributes its own volatility to the pot.

#### Scenario: Shield protects on explosion

- **WHEN** a round explodes and a player had played Shield that round
- **THEN** that player takes no explosion loss while every other non-shielded player loses the full pot value

#### Scenario: Shield forfeits scoring on a safe brew

- **WHEN** a round resolves safely and a player had played Shield that round
- **THEN** that player scores zero for the round even if their color would otherwise have won or shared the pot

