## ADDED Requirements

### Requirement: Pure Renderer Over The Protocol

The web client SHALL connect to the server over WebSocket using the existing
MessagePack-encoded protocol, perform the `JoinRoom` handshake carrying its supported
`protocol_version`, and proceed only after `RoomJoined`. It SHALL render server-reported
state and send only player **intents**; it MUST NOT compute scores, thresholds, outcomes,
or advance game phases on its own (Constitution §I).

#### Scenario: Handshake proceeds on a compatible version

- **WHEN** the client connects and the server replies `RoomJoined`
- **THEN** the client leaves the connecting state and renders the phase the server reports

#### Scenario: Incompatible version is surfaced

- **WHEN** the server replies with an `Error` describing a `protocol_version` mismatch
- **THEN** the client shows a clear, human-readable message and does not enter any game screen

#### Scenario: Client never self-advances

- **WHEN** no phase-changing message has been received
- **THEN** the client keeps rendering the current phase and computes no outcome locally

### Requirement: Phase-Driven Scene Rendering

The client SHALL render exactly one scene corresponding to the server-reported phase
(lobby, round-start, playing/table, depile, scoring/boom, deathmatch, game-over) and
SHALL switch scenes when the server advances the phase.

#### Scenario: Server advance switches the scene

- **WHEN** the server signals the round has entered the depile
- **THEN** the client switches from the table scene to the depile scene

### Requirement: Blind Cauldron

Ambient cauldron animation (steam, bubbles, ember glow) SHALL be **statistically
independent** of the hidden volatility and boiling point: it MUST look and move
identically whether the pot is near-empty or one card from the edge. The client MUST NOT
render any gauge, meter, or cue that discloses hidden pot state (game-design §4/§15).

#### Scenario: Ambient motion reveals nothing

- **WHEN** the same scene is rendered for a near-safe pot and for a pot at the brink
- **THEN** the cauldron's ambient animation is drawn from the same distribution and is
  not distinguishable by the player

### Requirement: Readability Priority On Card Faces

A card face SHALL present its attributes at the priority **volatility › color › points ›
effect**: volatility as the loudest mark, **color carried by a shape/sigil as well as
hue** (color-blind / low-color safe), points as countable marks, and any effect shown by
**name**. Full effect rules MAY live in a tooltip; the face stays clean.

#### Scenario: Color is legible without hue

- **WHEN** a card is rendered in a grayscale/low-color context
- **THEN** its color is still identifiable from the element sigil, and volatility remains
  the most prominent mark

### Requirement: Selectable And Accessible Text Via A DOM Overlay

The client SHALL render text that a player may need to copy, find, translate, or have read
aloud (at minimum the room/invite code, chat, player names, and scores) as real DOM
elements composited over the Pixi canvas, selectable by the user and exposed to assistive
technology. The Pixi canvas MUST NOT be the sole carrier of any such text.

#### Scenario: The room code is selectable and announced

- **WHEN** the player selects the room code and a screen reader inspects it
- **THEN** the code is selectable/copyable as text and is announced by the screen reader
  (it is a DOM element, not a canvas glyph)

### Requirement: Web And Mobile Packaging From One Source

The client SHALL build as a static web bundle that runs in a browser and as an installable
PWA, and the **same source** SHALL package into iOS and Android builds via a hybrid
(Capacitor) WebView shell. No platform target may require a separate client codebase.

#### Scenario: One build runs on web and mobile

- **WHEN** the web bundle is loaded in a desktop browser and inside the mobile WebView shell
- **THEN** both render the same client from the same source with no platform-specific fork

### Requirement: Deterministic Animation Clock

All time-based animation SHALL advance from an injectable animation clock so that, under a
pinned clock, rendering is byte-stable for visual snapshot tests (Constitution §II,
Layer 3).

#### Scenario: Pinned clock yields a stable screenshot

- **WHEN** the animation clock is pinned to a fixed phase and a scene is captured twice
- **THEN** the two screenshots are identical

### Requirement: Idle Rendering When Static

When no animation is active, the render loop SHALL idle (stop requesting frames) and
resume on the next state change or animation, to avoid needless GPU/battery use on mobile.

#### Scenario: Static phase stops the ticker

- **WHEN** the client is showing a static phase with no running animation
- **THEN** it is not continuously re-rendering frames, and it resumes rendering when the
  next server message or animation arrives
