//! Layer-3 visual tests: render each screen through ratatui's `TestBackend` and
//! assert on the text buffer (tasks 8.5/8.6), plus the deterministic replay
//! round-trip (task 8.3). These are the agent-readable "screenshots" — plain
//! text, no terminal, no server.

use boiling_point_tui::{App, fixtures, replay};

/// Render the app to a fixed-size buffer and flatten it to a string.
fn screen(app: &App) -> String {
    let buf = app.render_to_buffer(100, 34);
    let area = buf.area;
    let mut s = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    s
}

fn assert_has(s: &str, needle: &str) {
    assert!(s.contains(needle), "expected screen to contain {needle:?}");
}

fn assert_lacks(s: &str, needle: &str) {
    assert!(
        !s.contains(needle),
        "expected screen NOT to contain {needle:?}"
    );
}

#[test]
fn lobby_shows_seats_and_code() {
    let mut app = App::new();
    app.on_server(&fixtures::room_joined());
    let s = screen(&app);
    assert_has(&s, "Lobby");
    assert_has(&s, "BREW-7K3F");
    assert_has(&s, "Ruby");
    assert_has(&s, "mistfox");
    assert_has(&s, "Starts automatically at 4");
}

#[test]
fn round_start_shows_modifier_and_refill() {
    let mut app = App::new();
    app.on_server(&fixtures::room_joined());
    app.on_server(&fixtures::game_starting());
    app.on_server(&fixtures::modifier_thin_ice());
    app.on_server(&fixtures::your_hand());
    let s = screen(&app);
    assert_has(&s, "NEW MODIFIER");
    assert_has(&s, "Thin Ice");
    assert_has(&s, "refilled to 5");
}

#[test]
fn playing_cauldron_is_opaque() {
    let app = reach_playing();
    let s = screen(&app);
    assert_has(&s, "cards in the pot");
    assert_has(&s, "?? / ??");
    assert_has(&s, "your hand");
    // Secret boundary: no boiling-point information leaks during play (8.6).
    assert_lacks(&s, "boiling");
}

#[test]
fn depile_safe_hides_boiling_point() {
    let mut app = reach_playing();
    app.on_server(&fixtures::depile_safe());
    app.on_tick(5000); // reveal all cards
    let s = screen(&app);
    assert_has(&s, "boiling point stays hidden");
    assert_has(&s, "Ruby"); // an owner attribution
    assert_lacks(&s, "bp "); // no boiling-point value on a safe brew
}

#[test]
fn depile_explosion_marks_crossing_and_reveals_bp() {
    let mut app = reach_playing();
    app.on_server(&fixtures::depile_boom());
    app.on_tick(5000);
    let s = screen(&app);
    assert_has(&s, "bp 10");
    assert_has(&s, "crossed the boiling point");
}

#[test]
fn scoring_domination() {
    let mut app = reach_playing();
    app.on_server(&fixtures::round_scored_domination());
    let s = screen(&app);
    assert_has(&s, "DOMINATION");
}

#[test]
fn scoring_explosion_after_boom() {
    let mut app = reach_playing();
    app.on_server(&fixtures::explosion());
    app.on_tick(2000); // let the boom overlay expire
    let s = screen(&app);
    assert_has(&s, "EXPLOSION");
    assert_has(&s, "everyone loses");
}

#[test]
fn deathmatch_forces_play_no_pass() {
    let mut app = reach_playing();
    app.set_deathmatch(true);
    let s = screen(&app);
    assert_has(&s, "DEATHMATCH");
    assert_has(&s, "no pass");
    assert_lacks(&s, "[PASS]");
}

#[test]
fn game_over_shows_standings_and_winner() {
    let mut app = App::new();
    app.on_server(&fixtures::room_joined());
    app.on_server(&fixtures::game_over());
    let s = screen(&app);
    assert_has(&s, "FINAL STANDINGS");
    assert_has(&s, "mistfox"); // seat 1 = sole winner
}

#[test]
fn too_small_terminal_prompts_resize() {
    let app = App::new();
    let buf = app.render_to_buffer(40, 10);
    let area = buf.area;
    let mut s = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            s.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(s.contains("too small"));
}

#[test]
fn reconnect_overlay_renders() {
    let mut app = reach_playing();
    app.set_reconnecting(42_000);
    let s = screen(&app);
    assert_has(&s, "reconnecting");
    assert_has(&s, "auto-pass");
}

#[test]
fn state_snapshot_resumes_after_reconnect() {
    let mut app = reach_playing();
    app.set_reconnecting(30_000);
    // A snapshot arriving clears the overlay and restores allowed state.
    app.on_server(&fixtures::state_snapshot());
    let s = screen(&app);
    assert_lacks(&s, "reconnecting"); // overlay cleared by the inbound message
    assert_has(&s, "your hand"); // hand restored
    assert_has(&s, "locked out"); // reflects the missed waves
}

#[test]
fn whole_demo_game_reaches_game_over() {
    let mut app = App::new();
    for m in fixtures::demo_game() {
        app.on_server(&m);
    }
    let s = screen(&app);
    assert_has(&s, "FINAL STANDINGS");
}

#[test]
fn replay_round_trips() {
    let msgs = fixtures::demo_game();
    let path = std::env::temp_dir().join(format!(
        "bp-replay-{}-{}.jsonl",
        std::process::id(),
        msgs.len()
    ));
    replay::save(&path, &msgs).expect("save");
    let loaded = replay::load(&path).expect("load");
    assert_eq!(loaded.len(), msgs.len());
    assert_eq!(loaded, msgs);
    std::fs::remove_file(&path).ok();
}

/// Drive the app to an open wave 1 of round 1.
fn reach_playing() -> App {
    let mut app = App::new();
    app.on_server(&fixtures::room_joined());
    app.on_server(&fixtures::game_starting());
    app.on_server(&fixtures::your_hand());
    app.on_server(&fixtures::wave_open(1, 1, 30_000));
    app
}
