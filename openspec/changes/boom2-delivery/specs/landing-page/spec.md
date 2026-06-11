## ADDED Requirements

### Requirement: Public Landing Page With A Play CTA

There SHALL be a public, static landing page that explains what the game is (with screenshots/trailer) and offers a **"play now"** call-to-action that leads into the web client's create/join-room flow.

#### Scenario: A visitor can go from the landing page into a game

- **WHEN** a visitor clicks "play now" on the landing page
- **THEN** they are taken into the web client's create-or-join-room flow

### Requirement: Landing Page Is Static And Independent Of The Game Server

The landing page SHALL be a **static** asset (no game logic), deployable alongside or in front of the `web-client/`, so it can be served and cached independently of the game server.

#### Scenario: Landing page serves without the game server

- **WHEN** the landing page is requested
- **THEN** it is served as static content and does not require a live game-server session to render
