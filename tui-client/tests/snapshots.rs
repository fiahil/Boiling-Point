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
    app.on_server(&fixtures::group_joined());
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
    app.on_server(&fixtures::group_joined());
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
fn group_code_visible_during_play() {
    // Regression: the invite code must stay on screen through gameplay (in the
    // persistent header), not vanish with the lobby the instant the table fills.
    let app = reach_playing();
    let s = screen(&app);
    assert_has(&s, "Group");
    assert_has(&s, "BREW-7K3F");
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
fn resolution_plays_out_before_next_round_clobbers_it() {
    // Regression: the server sends the end-of-round burst (Depile → Explosion →
    // …) AND the next round's opening back-to-back. The next round must not
    // overwrite the resolution before the player has seen it.
    let mut app = reach_playing();
    app.on_server(&fixtures::depile_boom());
    app.on_server(&fixtures::explosion());
    // Next round arrives immediately (this is what used to clobber the depile and
    // wipe the captured data via reset_pot).
    app.on_server(&fixtures::modifier_thin_ice());
    app.on_server(&fixtures::your_hand());
    app.on_server(&fixtures::wave_open(2, 1, 30_000));

    // Immediately: still on the depile reveal, not the next wave.
    let s = screen(&app);
    assert_has(&s, "the depile");
    assert_lacks(&s, "your hand"); // not clobbered into the next round's playing screen

    // The cards peel until the crossing card is revealed — and it does NOT
    // auto-advance: it waits on the depile for the player to continue.
    let mut saw_crossing = false;
    for _ in 0..200 {
        app.on_tick(50);
        if screen(&app).contains("crossed the boiling point") {
            saw_crossing = true;
            break;
        }
    }
    assert!(
        saw_crossing,
        "the crossing card must be revealed during the depile"
    );
    assert_has(&screen(&app), "the depile"); // still waiting, not auto-advanced

    // Press continue → the explosion result shows (after the boom overlay clears).
    app.advance_resolution();
    let mut saw_explosion = false;
    for _ in 0..40 {
        app.on_tick(50);
        if screen(&app).contains("EXPLOSION") {
            saw_explosion = true;
            break;
        }
    }
    assert!(
        saw_explosion,
        "explosion result must appear after continuing"
    );

    // After the result's hold, it drains to the buffered next round (now playing).
    for _ in 0..200 {
        app.on_tick(50);
    }
    let s = screen(&app);
    assert_has(&s, "your hand");
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
fn final_wave_indicator_shows() {
    let mut app = reach_playing();
    app.on_server(&fixtures::wave_final());
    let s = screen(&app);
    assert_has(&s, "FINAL WAVE");
}

#[test]
fn deathmatch_announced_on_game_over() {
    let mut app = App::new();
    app.on_server(&fixtures::group_joined());
    app.on_server(&fixtures::deathmatch_started());
    app.on_server(&fixtures::game_over());
    let s = screen(&app);
    assert_has(&s, "FINAL STANDINGS");
    assert_has(&s, "Deathmatch");
}

#[test]
fn game_over_shows_standings_and_winner() {
    let mut app = App::new();
    app.on_server(&fixtures::group_joined());
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
fn reconnect_overlay_hidden_on_entry_menu() {
    // Regression: a dropped socket on the pre-game menu must not strand the
    // player behind the reconnect overlay — the overlay is for seated tables.
    let mut app = App::new(); // Entry phase, no table joined
    app.set_reconnecting(42_000);
    let s = screen(&app);
    assert_lacks(&s, "reconnecting");
    assert_has(&s, "How do you want to play?");
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
    // The resolution gate buffers each round's end (and the next round) behind the
    // depile animation; the real event loop ticks ~30fps to play it. The depile now
    // waits for a keypress to continue, so press through it as a player would.
    for _ in 0..400 {
        app.on_tick(60);
        if screen(&app).contains("the depile") {
            app.advance_resolution();
        }
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

#[test]
fn card_face_shows_effect_name_and_pips() {
    // Cards now name their effect (not a bare ◆) and show points as pips.
    let app = reach_playing();
    let s = screen(&app);
    assert_has(&s, "Peek"); // effect name on the Wild-Peek card face (hand idx 2)
    assert_has(&s, "Recall"); // effect name on the Amethyst-Recall card (hand idx 4)
    assert_has(&s, "●"); // points pips
}

#[test]
fn inspector_explains_selected_effect() {
    let mut app = reach_playing();
    app.set_cursor(2); // the Wild Peek card
    let s = screen(&app);
    assert_has(&s, "inspect");
    assert_has(&s, "Peek");
    assert_has(&s, "threshold"); // Peek described in value-free terms
    // Secret boundary holds: the playing screen still never prints "boiling".
    assert_lacks(&s, "boiling");
}

#[test]
fn inspector_explains_pass() {
    let mut app = reach_playing();
    app.set_cursor(99); // past the hand → the Pass slot
    let s = screen(&app);
    assert_has(&s, "inspect");
    assert_has(&s, "locked out");
    assert_has(&s, "lose the pot");
}

#[test]
fn inspector_follows_cursor() {
    let mut app = reach_playing();
    app.set_cursor(0); // Ruby ingredient
    let a = screen(&app);
    assert_has(&a, "Ruby");
    app.set_cursor(4); // Amethyst Recall
    let b = screen(&app);
    assert_has(&b, "Recall");
    assert_ne!(a, b, "the inspector must change as the cursor moves");
}

#[test]
fn codex_lists_effects_and_modifiers() {
    let mut app = reach_playing();
    app.open_codex();
    let s = screen(&app);
    assert_has(&s, "Codex");
    assert_has(&s, "Peek");
    assert_has(&s, "Double Down");
    assert_has(&s, "Reversal");
    assert_has(&s, "Residue");
    // Modifiers show DIRECTION only — never the server-side magnitude.
    assert_has(&s, "boiling point lower"); // Thin Ice's qualitative effect
    assert_lacks(&s, "-4"); // not the magnitude
    assert_lacks(&s, "+4");
    assert_lacks(&s, "+3"); // Residue's hidden +3
}

#[test]
fn ambient_animation_is_deterministic_and_blind() {
    // The cauldron animates, but its motion carries no information and a render at
    // a fixed animation phase is stable (so the snapshot layer stays deterministic).
    let app = reach_playing();
    let a = screen(&app); // anim_ms == 0
    let app2 = reach_playing();
    let a2 = screen(&app2);
    assert_eq!(a, a2, "rendering at animation phase 0 must be identical");

    let mut app3 = reach_playing();
    app3.on_tick(300); // advance the ambient clock
    let b = screen(&app3);
    // The public, state-bearing content is unchanged by the animation.
    assert_has(&b, "cards in the pot");
    assert_has(&b, "?? / ??");
}

/// Drive the app to an open wave 1 of round 1.
fn reach_playing() -> App {
    let mut app = App::new();
    app.on_server(&fixtures::group_joined());
    app.on_server(&fixtures::game_starting());
    app.on_server(&fixtures::your_hand());
    app.on_server(&fixtures::wave_open(1, 1, 30_000));
    app
}
