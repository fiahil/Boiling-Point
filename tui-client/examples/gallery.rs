//! Render a few key screens to stdout as plain text — a quick visual gallery.
//!
//! `cargo run -p boiling-point-tui --example gallery`
//!
//! This reuses the same `TestBackend` rendering the snapshot tests use, so it
//! needs no terminal and no server (research R5).

use boiling_point_tui::{App, fixtures};

/// Render `app` to a fixed-size buffer and print it, trimming trailing spaces.
fn show(title: &str, app: &App) {
    let buf = app.render_to_buffer(92, 26);
    println!("== {title} ==");
    let area = buf.area;
    for y in 0..area.height {
        let mut line = String::new();
        for x in 0..area.width {
            line.push_str(buf[(x, y)].symbol());
        }
        println!("{}", line.trim_end());
    }
    println!();
}

fn main() {
    // A live wave in round 2 (Thin Ice active), two cards already in the pot.
    let mut playing = App::new();
    for msg in fixtures::demo_game().into_iter().take(6) {
        playing.on_server(&msg);
    }
    show("PLAYING — round 1, wave open", &playing);

    // The dramatic moment: an explosion depile, fully revealed.
    let mut boom = App::new();
    boom.on_server(&fixtures::room_joined());
    boom.on_server(&fixtures::game_starting());
    boom.on_server(&fixtures::your_hand());
    boom.on_server(&fixtures::wave_open(1, 1, 30_000));
    boom.on_server(&fixtures::depile_boom());
    boom.on_tick(5_000); // reveal every card
    show(
        "DEPILE — explosion (boiling point revealed, crossing marked)",
        &boom,
    );

    // Game over, decided by a Deathmatch.
    let mut over = App::new();
    over.on_server(&fixtures::room_joined());
    over.on_server(&fixtures::deathmatch_started());
    over.on_server(&fixtures::game_over());
    show("GAME OVER — decided by Deathmatch", &over);
}
