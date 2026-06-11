## ADDED Requirements

### Requirement: Pot Value And The P-Symmetry

The pot value **P** SHALL equal the sum of colored Vote points in the cauldron. On a **safe brew** the dominant color wins **+P**. On an **explosion** the **detonator(s)** lose **−P** and no color is awarded the pot. A player who contributed nothing scores **0** either way.

#### Scenario: Safe brew awards P to the dominant color

- **WHEN** a round ends safely with red holding the strictly highest color point total
- **THEN** the red player gains +P (the full pot value)

#### Scenario: Explosion costs only the detonator

- **WHEN** a round explodes
- **THEN** the detonator(s) lose −P, no color is awarded the pot, and non-liable players lose nothing

### Requirement: Safe-Brew Dominance Scoring

On a safe brew the winner SHALL be the color with the strictly highest sum of colored Vote points (winner-takes-all). A tie for highest SHALL split P equally among the tied colors, **rounded down**; the scoreboard is integer-only.

#### Scenario: A tie splits the pot, rounded down

- **WHEN** two colors tie for the highest point total in a 7-point pot
- **THEN** those two players each gain 3 (the leftover point evaporates)

### Requirement: Detonator Identification — Fatal-Wave Ascending-Volatility Sort

On explosion the engine SHALL consider **only the fatal wave's** ingredients, applied on top of the hidden pre-wave total, sorted **ascending by effective volatility**. The **trigger** is the first card that pushes the cumulative total **past** the boiling point. The trigger player **and every player in that wave holding a higher-volatility card** are liable and **split −P equally**. **Equal-volatility cards are simultaneous** — all are liable if one triggers. Cauldron-level volatility modifiers (e.g. Dampen/Surge) sit in the running total, not the per-card sort. Players not in the fatal wave SHALL NOT be liable.

#### Scenario: The heaviest fatal-wave cards split the loss

- **WHEN** the fatal wave holds cards of volatility 1, 4, and 6 and the cumulative crosses the boiling point at the 4
- **THEN** the 4 and 6 players split −P and the 1 player is not liable

#### Scenario: Folding before the fatal wave is safe

- **WHEN** a player played a heavy card in an earlier wave then passed before the fatal wave
- **THEN** that player is not liable for the explosion

#### Scenario: Effective volatility drives the sort

- **WHEN** a card's effective volatility differs from its printed value
- **THEN** the sort and the trigger use the effective value, consistent with the explosion check

### Requirement: Wards Modify Detonator Damage

A detonator with an active **Ward** SHALL have their −P modified at resolution: **Cap** limits the loss to a fixed small amount; **Halve** halves it; **Redirect** transfers the full −P to a chosen player, cascading through that player's own wards. A ward is consumed on use.

#### Scenario: Redirect shoves the loss onto a rival

- **WHEN** a detonator has an active Redirect ward and names a target
- **THEN** the detonator takes no loss and the target takes −P, cascading if the target also has a ward

### Requirement: The Depile Reveals By Volatility And Reveals The Boiling Point Every Round

After **every** round, boom or safe, the engine SHALL reveal the pot in a **volatility-ascending** depile and SHALL reveal the **boiling point**. On a boom the running total visibly crosses the boiling point and the liable cards are marked; on a safe brew the running total stops short of the revealed boiling point. (Peek's value is unaffected — it is in-round; the depile is post-round.)

#### Scenario: A safe brew reveals the near miss

- **WHEN** a round ends safely
- **THEN** the depile reveals the boiling point and shows the running volatility stopping short of it

#### Scenario: A boom marks the culprits

- **WHEN** a round explodes
- **THEN** the depile reveals the boiling point, the crossing point, and which cards were liable
