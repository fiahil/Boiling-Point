//! All rendering. Every screen is a pure function of [`App`] state, so each can
//! be snapshot-tested through ratatui's `TestBackend` (research R5). The renderer
//! shows only what the view model holds — it has no access to secrets, so it
//! cannot leak one.

use boiling_point_protocol::{
    PlayerId,
    server::ScoringOutcome,
    vocab::{CardView, Color as Wire, EffectKind, ModifierKind},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, Conn, Selection, emote_label};
use crate::palette;
use crate::view::Phase;

/// Minimum supported terminal width.
const MIN_W: u16 = 80;
/// Minimum supported terminal height.
const MIN_H: u16 = 24;

/// Draw the whole UI for the current frame.
pub(crate) fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_W || area.height < MIN_H {
        too_small(frame, area);
        return;
    }

    match app.phase {
        Phase::Entry => entry(frame, area, app),
        Phase::JoinCode => join_code(frame, area, app),
        Phase::Connecting | Phase::Queue | Phase::Lobby => lobby(frame, area, app),
        Phase::RoundStart => round_start(frame, area, app),
        Phase::Playing => playing(frame, area, app),
        Phase::Depile => depile(frame, area, app),
        Phase::Scoring => scoring(frame, area, app),
        Phase::GameOver => game_over(frame, area, app),
    }

    // Overlays, painted last (top-most).
    if app.boom_ms > 0 && app.phase == Phase::Scoring {
        boom(frame, area, app);
    }
    if app.peek_modal_ms > 0
        && let Some(bp) = app.vm.my_peek
    {
        peek_modal(frame, area, bp);
    }
    if let Some(prompt) = &app.recall {
        recall_modal(frame, area, prompt);
    }
    if app.emote_open {
        emote_palette(frame, area);
    }
    // The reconnect/abandoned overlay only makes sense once seated at a table;
    // on the pre-game menu a dropped socket must not strand the player behind it.
    let seated = matches!(
        app.phase,
        Phase::Lobby | Phase::RoundStart | Phase::Playing | Phase::Depile | Phase::Scoring
    );
    if seated && !matches!(app.conn, Conn::Connected) {
        reconnect_overlay(frame, area, app);
    }
    toasts(frame, area, app);
    if app.debug {
        debug_overlay(frame, area, app);
    }
}

fn too_small(frame: &mut Frame, area: Rect) {
    let msg = format!(
        "Terminal too small.\nResize to at least {MIN_W}x{MIN_H}\n(now {}x{})",
        area.width, area.height
    );
    frame.render_widget(
        Paragraph::new(msg).alignment(Alignment::Center),
        center(area, 40, 5),
    );
}

// ---- pre-connection screens ---------------------------------------------

fn entry(frame: &mut Frame, area: Rect, app: &App) {
    let [title, body, foot] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .areas(area);
    banner(frame, title);

    let items = [
        ("Quick match", "drop into the queue; the table fills to 4"),
        ("Create a group", "get an invite code to share"),
        ("Join with a code", "enter a friend's BREW-XXXX"),
    ];
    let mut lines = vec![
        Line::from(Span::styled(
            format!("name: {}_", app.name_input),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
    ];
    for (i, (label, desc)) in items.iter().enumerate() {
        let marker = if i == app.menu_index { "▸ " } else { "  " };
        let style = if i == app.menu_index {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker}{label:<18}"), style),
            Span::styled(*desc, Style::default().fg(Color::DarkGray)),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines).block(bordered("How do you want to play?")),
        body,
    );
    hint(
        frame,
        foot,
        "↑/↓ choose   type a name   ↵ select   Esc quit   F12 debug",
    );
}

fn join_code(frame: &mut Frame, area: Rect, app: &App) {
    let [title, body, foot] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .areas(area);
    banner(frame, title);
    let lines = vec![
        Line::raw("Enter an invite code:"),
        Line::raw(""),
        Line::from(Span::styled(
            format!("  {}_", app.code_input),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(bordered("Join a group")), body);
    hint(frame, foot, "type code   ↵ join   Esc back");
}

fn lobby(frame: &mut Frame, area: Rect, app: &App) {
    let [head, body, foot] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .areas(area);

    let code = app.vm.group_code.clone().unwrap_or_else(|| "…".into());
    let status = match app.phase {
        Phase::Queue => "assembling a table…".to_string(),
        Phase::Connecting => "connecting…".to_string(),
        _ => format!("waiting for players  ({}/4)", app.vm.players.len()),
    };
    let head_lines = vec![
        Line::from(Span::styled(
            format!("Group {code}"),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(status, Style::default().fg(Color::Cyan))),
        Line::from(Span::styled(
            format!("invite code: {code}"),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(head_lines).block(bordered("Lobby")), head);

    // Four seats, by colour order, filled from the roster.
    let mut lines = Vec::new();
    for (i, color) in Wire::PLAYER_COLORS.iter().enumerate() {
        let seat = app.vm.players.get(i);
        let (name, occupied) = match seat {
            Some(p) => (p.name.clone(), true),
            None => ("—".to_string(), false),
        };
        let dot = if occupied { "●" } else { "◌" };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} {:<9}", palette::glyph(*color), palette::name(*color)),
                Style::default().fg(palette::style(*color)),
            ),
            Span::raw(format!("{name:<22}")),
            Span::styled(
                format!("{dot} {}", if occupied { "ready" } else { "waiting" }),
                Style::default().fg(if occupied {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "Starts automatically at 4 players. No host, no settings.",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(Paragraph::new(lines).block(bordered("Seats")), body);
    hint(frame, foot, "Ctrl-C quit");
}

// ---- round screens -------------------------------------------------------

fn round_start(frame: &mut Frame, area: Rect, app: &App) {
    let [head, body, hand_area, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(6),
        Constraint::Length(2),
    ])
    .areas(area);
    header(frame, head, app, "round begins");

    let mut lines = Vec::new();
    match app.vm.new_modifier {
        Some(m) => {
            lines.push(Line::from(Span::styled(
                "NEW MODIFIER DRAWN",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    modifier_label(m),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  — {}", modifier_desc(m)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        None => lines.push(Line::from(Span::styled(
            "Round 1 — a clean cauldron, no modifiers.",
            Style::default().fg(Color::DarkGray),
        ))),
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "ACTIVE MODIFIERS",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    if app.vm.active_modifiers.is_empty() {
        lines.push(Line::from(Span::styled(
            "   none",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let mut spans = vec![Span::raw("   ")];
        for m in &app.vm.active_modifiers {
            spans.push(Span::raw(format!("{}   ", modifier_label(*m))));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "Boiling point: base range 8–14, shifted by the active modifiers (exact value hidden).",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .block(bordered("Round start"))
            .wrap(Wrap { trim: false }),
        body,
    );

    hand_row(frame, hand_area, app, usize::MAX, true);
    hint(frame, foot, "wave 1 opens automatically");
}

fn playing(frame: &mut Frame, area: Rect, app: &App) {
    let [head, opp, cauldron_area, me_area, hand_area, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(5),
        Constraint::Min(3),
        Constraint::Length(2),
        Constraint::Length(6),
        Constraint::Length(2),
    ])
    .areas(area);

    let dm = if app.vm.deathmatch { "DEATHMATCH " } else { "" };
    let fw = if app.vm.final_wave {
        " · FINAL WAVE"
    } else {
        ""
    };
    header(
        frame,
        head,
        app,
        &format!("{dm}wave {}{fw}", app.vm.wave_number),
    );
    opponents(frame, opp, app);
    cauldron(frame, cauldron_area, app);
    self_line(frame, me_area, app);

    if app.vm.deathmatch {
        hand_row(frame, hand_area, app, app.cursor, false);
        hint(
            frame,
            foot,
            "←/→ choose   ↵ commit (no pass — you must play)   e emote",
        );
    } else {
        hand_row(frame, hand_area, app, app.cursor, false);
        hint(
            frame,
            foot,
            "←/→ move  ↵ commit  p pass  L lock-in  e emote  F12 debug",
        );
    }
}

fn depile(frame: &mut Frame, area: Rect, app: &App) {
    let [head, bar_area, list_area, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(4),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .areas(area);
    header(frame, head, app, "the depile");

    let Some(d) = &app.vm.last_depile else {
        return;
    };
    let shown = app.depile_shown.min(d.reveals.len());

    // Descending volatility bar: starts at the pot total and drops as cards peel.
    let value = if shown == 0 {
        d.total_volatility
    } else {
        d.reveals[shown - 1].running_volatility
    };
    let scale = d.total_volatility.max(d.boiling_point.unwrap_or(0)).max(1) as usize;
    let width = 28usize;
    let filled = (value as usize * width / scale).min(width);
    let bar: String = "█".repeat(filled) + &"░".repeat(width - filled);
    let mut bar_lines = vec![Line::from(vec![
        Span::raw("volatility  "),
        Span::styled(bar, Style::default().fg(Color::Rgb(230, 140, 40))),
        Span::raw(format!("  {value}")),
    ])];
    if d.exploded {
        if let Some(bp) = d.boiling_point {
            let pos = (bp as usize * width / scale).min(width.saturating_sub(1));
            let mut marker = " ".repeat(width);
            marker.replace_range(pos..pos + 1, "|");
            bar_lines.push(Line::from(vec![
                Span::raw("threshold   "),
                Span::styled(marker, Style::default().fg(Color::Red)),
                Span::styled(format!("  bp {bp}"), Style::default().fg(Color::Red)),
            ]));
        }
    } else {
        bar_lines.push(Line::from(Span::styled(
            "boiling point stays hidden on a safe brew",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(bar_lines).block(bordered("revealing last-played first")),
        bar_area,
    );

    let mut lines = Vec::new();
    for (i, e) in d.reveals.iter().take(shown).enumerate() {
        let owner = app
            .vm
            .player(e.player)
            .map(|p| palette::name(p.color))
            .unwrap_or("?");
        let crossed = d.exploded && d.crossing_index == Some(i);
        let mut spans = vec![Span::styled(
            format!("{owner:<9}"),
            Style::default().fg(card_color(&e.card)),
        )];
        spans.extend(card_spans(&e.card));
        spans.push(Span::raw(format!("   (vol→{})", e.running_volatility)));
        if crossed {
            spans.push(Span::styled(
                "   <= crossed the boiling point",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).block(bordered("the pot")), list_area);
    hint(frame, foot, "↵/space skip the reveal");
}

fn scoring(frame: &mut Frame, area: Rect, app: &App) {
    let [head, body, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .areas(area);
    header(frame, head, app, "round results");

    let mut lines = Vec::new();
    if let Some(s) = &app.vm.last_scoring {
        let outcome = match &s.outcome {
            ScoringOutcome::Domination { winner } => {
                format!("DOMINATION — {} takes the pot", palette::name(*winner))
            }
            ScoringOutcome::Split { colors } => {
                let names: Vec<&str> = colors.iter().map(|c| palette::name(*c)).collect();
                format!("SPLIT — {} share the pot", names.join(" & "))
            }
        };
        lines.push(Line::from(Span::styled(
            outcome,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(""));
        for a in &s.awards {
            lines.push(score_line(app, a.player, Some(a.score)));
        }
    } else if let Some(x) = &app.vm.last_explosion {
        lines.push(Line::from(Span::styled(
            format!("EXPLOSION — pot was {} — everyone loses it", x.pot_value),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(""));
        for d in &x.deltas {
            lines.push(score_line(app, d.player, Some(d.score)));
        }
        if !x.shielded.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "shielded (no loss): ".to_string()
                    + &x.shielded
                        .iter()
                        .filter_map(|p| app.vm.player(*p).map(|v| v.name.clone()))
                        .collect::<Vec<_>>()
                        .join(", "),
                Style::default().fg(Color::Cyan),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines).block(bordered("results")), body);
    hint(frame, foot, "the next round begins automatically");
}

fn game_over(frame: &mut Frame, area: Rect, app: &App) {
    let [head, body, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .areas(area);
    header(frame, head, app, "game over");

    let mut scores = app.vm.final_scores.clone();
    scores.sort_by_key(|s| std::cmp::Reverse(s.score));
    let mut lines = vec![Line::from(Span::styled(
        "FINAL STANDINGS",
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::raw(""));
    for s in &scores {
        let win = app.vm.winners.contains(&s.player);
        let crown = if win { "🏆 " } else { "   " };
        let mut spans = vec![Span::raw(crown)];
        spans.extend(score_line(app, s.player, None).spans);
        lines.push(Line::from(spans));
    }
    if app.vm.deathmatch {
        lines.push(Line::raw(""));
        let names: Vec<&str> = app
            .vm
            .dm_participants
            .iter()
            .filter_map(|p| app.vm.player(*p).map(|v| palette::name(v.color)))
            .collect();
        lines.push(Line::from(Span::styled(
            format!("⚔ decided by Deathmatch ({})", names.join(" vs ")),
            Style::default().fg(Color::Magenta),
        )));
    }
    frame.render_widget(Paragraph::new(lines).block(bordered("game over")), body);
    hint(frame, foot, "↵ back to lobby   r rematch queue");
}

// ---- shared widgets ------------------------------------------------------

fn banner(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "B O I L I N G   P O I N T",
            Style::default()
                .fg(Color::Rgb(230, 140, 40))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "an alchemical game of nerve",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

fn header(frame: &mut Frame, area: Rect, app: &App, right: &str) {
    // Keep the group invite code visible through every in-game phase (the lobby
    // screen, where it otherwise lived, is gone the moment play begins).
    let code = app.vm.group_code.clone().unwrap_or_else(|| "…".into());
    let left = format!(
        " Group {}   Round {}/{}   {}",
        code,
        app.vm.round_number.max(1),
        app.vm.round_count.max(1),
        right
    );
    let mods: String = app
        .vm
        .active_modifiers
        .iter()
        .map(|m| modifier_short(*m))
        .collect::<Vec<_>>()
        .join(" ");
    let line = Line::from(vec![
        Span::styled(left, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("    "),
        Span::styled(mods, Style::default().fg(Color::Magenta)),
    ]);
    frame.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

fn opponents(frame: &mut Frame, area: Rect, app: &App) {
    let others: Vec<_> = app
        .vm
        .players
        .iter()
        .filter(|p| Some(p.id) != app.vm.me)
        .collect();
    if others.is_empty() {
        return;
    }
    let cols = Layout::horizontal(vec![
        Constraint::Ratio(1, others.len() as u32);
        others.len()
    ])
    .split(area);
    for (p, col) in others.iter().zip(cols.iter()) {
        let title = Span::styled(
            format!(" {} {} ", palette::glyph(p.color), p.name),
            Style::default()
                .fg(palette::style(p.color))
                .add_modifier(Modifier::BOLD),
        );
        let mut lines = vec![Line::from(vec![
            Span::raw("score "),
            Span::styled(
                format!("{}", p.score),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ])];
        lines.push(Line::from(format!(
            "in pot {}",
            "▮".repeat(p.contributed as usize)
        )));
        if !p.connected {
            lines.push(Line::from(Span::styled(
                "disconnected",
                Style::default().fg(Color::DarkGray),
            )));
        }
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
            *col,
        );
    }
}

fn cauldron(frame: &mut Frame, area: Rect, app: &App) {
    let mut chips: Vec<Span> = Vec::new();
    for p in &app.vm.players {
        if p.contributed > 0 {
            chips.push(Span::styled(
                "▮".repeat(p.contributed as usize),
                Style::default().fg(palette::style(p.color)),
            ));
            chips.push(Span::raw(" "));
        }
    }
    if chips.is_empty() {
        chips.push(Span::styled("empty", Style::default().fg(Color::DarkGray)));
    }
    let lines = vec![
        Line::from(Span::styled(
            "~ ~ ~  T H E   C A U L D R O N  ~ ~ ~",
            Style::default().fg(Color::Rgb(120, 200, 200)),
        ))
        .alignment(Alignment::Center),
        Line::from(format!("{} cards in the pot", app.vm.cauldron_count))
            .alignment(Alignment::Center),
        Line::from("volatility  ?? / ??").alignment(Alignment::Center),
        Line::from(chips).alignment(Alignment::Center),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn self_line(frame: &mut Frame, area: Rect, app: &App) {
    let me = app.vm.me.and_then(|id| app.vm.player(id));
    let (letter, name, color, score) = match me {
        Some(p) => (
            palette::glyph(p.color),
            p.name.clone(),
            palette::style(p.color),
            p.score,
        ),
        None => ("·", "you".into(), Color::White, 0),
    };
    let committed = match app.committed {
        Selection::None => "deciding".to_string(),
        Selection::Pass => "PASS (locked out)".to_string(),
        Selection::Card(_) => format!(
            "committed{}",
            if app.locked_in { " · locked in" } else { "" }
        ),
    };
    let timer = app
        .countdown_ms
        .map(fmt_ms)
        .unwrap_or_else(|| "--:--".into());
    let line = Line::from(vec![
        Span::styled(
            format!(" {letter} {name}  "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("score {score}    ")),
        Span::styled(format!("⏱ {timer}    "), Style::default().fg(Color::Yellow)),
        Span::styled(committed, Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// Render the hand as a row of bracketed card tokens plus a Pass slot.
/// Card box geometry.
const CARD_W: u16 = 9;
const CARD_H: u16 = 4;

/// Render the hand as a row of rounded mini-cards with soft drop shadows, under
/// a faint label. The cursor card is highlighted; a committed card glows green;
/// a Pass card closes the row (except in Deathmatch).
fn hand_row(frame: &mut Frame, area: Rect, app: &App, cursor: usize, round_start: bool) {
    let label = if round_start {
        "your hand — refilled to 5 (✦ newly drawn)"
    } else {
        "your hand"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(label, Style::default().fg(Color::DarkGray))),
        Rect { height: 1, ..area },
    );

    let mut x = area.x + 1;
    let y = area.y + 1;
    for (i, c) in app.vm.hand.iter().enumerate() {
        if x + CARD_W + 1 > area.right() {
            break;
        }
        let rect = Rect {
            x,
            y,
            width: CARD_W,
            height: CARD_H,
        };
        let committed = matches!(app.committed, Selection::Card(id) if id == c.id);
        let newmark = if round_start && app.vm.is_new(c.id) {
            "✦"
        } else {
            ""
        };
        let eff = if c.view.effect.is_some() { "◆" } else { "" };
        draw_card(
            frame,
            rect,
            CardFace {
                label: format!("{}{}", i + 1, newmark),
                glyph: palette::glyph(c.view.color),
                color: card_color(&c.view),
                line2: format!("v{} p{} {}", c.view.volatility, c.view.points, eff),
                selected: !round_start && i == cursor,
                committed,
            },
        );
        x += CARD_W + 2;
    }
    if !round_start && !app.vm.deathmatch && x + 7 < area.right() {
        let rect = Rect {
            x,
            y,
            width: 7,
            height: CARD_H,
        };
        draw_card(
            frame,
            rect,
            CardFace {
                label: "P".into(),
                glyph: "",
                color: Color::Gray,
                line2: "pass".into(),
                selected: cursor >= app.vm.hand.len(),
                committed: false,
            },
        );
    }
}

/// The face of a card to draw: its corner label, colour glyph, attribute line,
/// and selection/commit state.
struct CardFace {
    label: String,
    glyph: &'static str,
    color: Color,
    line2: String,
    selected: bool,
    committed: bool,
}

/// Draw one rounded card with a soft drop shadow into `rect`.
fn draw_card(frame: &mut Frame, rect: Rect, face: CardFace) {
    let CardFace {
        label,
        glyph,
        color,
        line2,
        selected,
        committed,
    } = face;
    // Soft drop shadow: an L of dim cells one column right and one row down.
    {
        let shadow = Style::default().fg(Color::Rgb(60, 60, 72));
        let buf = frame.buffer_mut();
        let bounds = buf.area;
        let right = rect.right();
        let bottom = rect.bottom();
        for sy in (rect.y + 1)..=bottom {
            if right < bounds.right() && sy < bounds.bottom() {
                buf[(right, sy)].set_symbol("░").set_style(shadow);
            }
        }
        for sx in (rect.x + 1)..=right {
            if bottom < bounds.bottom() && sx < bounds.right() {
                buf[(sx, bottom)].set_symbol("░").set_style(shadow);
            }
        }
    }

    let border_style = if selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if committed {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(color)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(label, border_style));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(glyph, Style::default().fg(color))),
            Line::from(Span::styled(line2, Style::default().fg(color))),
        ]),
        inner,
    );
}

// ---- overlays ------------------------------------------------------------

fn boom(frame: &mut Frame, area: Rect, app: &App) {
    let pot = app
        .vm
        .last_explosion
        .as_ref()
        .map(|x| x.pot_value)
        .unwrap_or(0);
    let r = center(area, 50, 9);
    frame.render_widget(Clear, r);
    let lines = vec![
        Line::raw(""),
        Line::from("####   B O O M   ####").alignment(Alignment::Center),
        Line::raw(""),
        Line::from(format!("the cauldron erupts — everyone loses {pot}"))
            .alignment(Alignment::Center),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White).bg(Color::Red)),
        ),
        r,
    );
}

fn peek_modal(frame: &mut Frame, area: Rect, bp: u8) {
    let r = center(area, 46, 6);
    frame.render_widget(Clear, r);
    let lines = vec![
        Line::from("PEEK").alignment(Alignment::Center),
        Line::raw(""),
        Line::from(format!("the cauldron's boiling point is {bp}")).alignment(Alignment::Center),
        Line::from(Span::styled(
            "others were told only \"someone peeked\"",
            Style::default().fg(Color::DarkGray),
        ))
        .alignment(Alignment::Center),
    ];
    frame.render_widget(Paragraph::new(lines).block(bordered("private")), r);
}

fn recall_modal(frame: &mut Frame, area: Rect, prompt: &crate::app::RecallPrompt) {
    let r = center(area, 60, 8);
    frame.render_widget(Clear, r);
    let mut spans = vec![Span::raw(" ")];
    for (i, c) in prompt.targets.iter().enumerate() {
        let mut style = Style::default().fg(card_color(&c.view));
        if i == prompt.cursor {
            style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
        }
        spans.push(Span::styled(format!("[{}]", card_text(&c.view)), style));
        spans.push(Span::raw("  "));
    }
    let lines = vec![
        Line::from("RECALL — pull one of your own pot cards back").alignment(Alignment::Center),
        Line::raw(""),
        Line::from(spans),
        Line::raw(""),
        Line::from(Span::styled(
            "←/→ choose   ↵ confirm   Esc cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(bordered("recall")), r);
}

fn emote_palette(frame: &mut Frame, area: Rect) {
    let r = center(area, 64, 4);
    frame.render_widget(Clear, r);
    let mut spans = vec![Span::raw(" ")];
    for id in 1..=6u16 {
        let (icon, label) = emote_label(id);
        spans.push(Span::raw(format!("{id} {icon} {label}   ")));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(bordered("emote — press 1-6, Esc to close")),
        r,
    );
}

fn reconnect_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let r = center(area, 50, 7);
    frame.render_widget(Clear, r);
    let lines = match app.conn {
        Conn::Reconnecting { remaining_ms } => vec![
            Line::from("connection lost — reconnecting…").alignment(Alignment::Center),
            Line::raw(""),
            Line::from(format!("seat held for {}", fmt_ms(remaining_ms)))
                .alignment(Alignment::Center),
            Line::from(Span::styled(
                "you auto-pass every wave while away",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center),
        ],
        Conn::Abandoned => vec![
            Line::raw(""),
            Line::from("seat abandoned — the game continued without you")
                .alignment(Alignment::Center),
        ],
        Conn::Connected => return,
    };
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" reconnection ")
                .style(Style::default().fg(Color::Yellow)),
        ),
        r,
    );
}

fn toasts(frame: &mut Frame, area: Rect, app: &App) {
    if app.toasts.is_empty() {
        return;
    }
    let n = app.toasts.len().min(3) as u16;
    let r = Rect {
        x: area.x + 1,
        // Sit clear above the two-row footer hint so it stays readable.
        y: area.bottom().saturating_sub(n + 2),
        width: area.width.saturating_sub(2),
        height: n,
    };
    let lines: Vec<Line> = app
        .toasts
        .iter()
        .rev()
        .take(3)
        .map(|t| {
            // An explicit light fg keeps the dark toast legible on light
            // terminals, where the default foreground would be near-black.
            Line::from(Span::styled(
                format!(" {} ", t.text),
                Style::default()
                    .fg(Color::Rgb(235, 235, 245))
                    .bg(Color::Rgb(40, 40, 55)),
            ))
        })
        .collect();
    frame.render_widget(Clear, r);
    frame.render_widget(Paragraph::new(lines), r);
}

fn debug_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let r = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height.min(12),
    };
    frame.render_widget(Clear, r);
    let mut lines = vec![Line::from(Span::styled(
        format!(
            " phase {:?}   in {}  out {}   round {} wave {} ",
            app.phase, app.in_count, app.out_count, app.vm.round_number, app.vm.wave_number
        ),
        Style::default().fg(Color::Black).bg(Color::Cyan),
    ))];
    for entry in app.msg_log.iter().rev().take(8) {
        lines.push(Line::from(Span::styled(
            entry.clone(),
            Style::default().fg(Color::Cyan),
        )));
    }
    frame.render_widget(Paragraph::new(lines).block(bordered("DEBUG (F12)")), r);
}

// ---- small helpers -------------------------------------------------------

fn score_line(app: &App, id: PlayerId, delta: Option<i32>) -> Line<'static> {
    let (letter, name, color, score) = match app.vm.player(id) {
        Some(p) => (
            palette::glyph(p.color),
            p.name.clone(),
            palette::style(p.color),
            p.score,
        ),
        None => ("·", "?".into(), Color::White, 0),
    };
    let mut spans = vec![Span::styled(
        format!("{letter} {name:<14}"),
        Style::default().fg(color),
    )];
    spans.push(Span::raw(format!("{score:>4}")));
    if let Some(d) = delta {
        let sign = if d > 0 {
            format!("+{d}")
        } else {
            format!("{d}")
        };
        let dc = if d > 0 {
            Color::Green
        } else if d < 0 {
            Color::Red
        } else {
            Color::DarkGray
        };
        spans.push(Span::styled(format!("   {sign}"), Style::default().fg(dc)));
    }
    Line::from(spans)
}

/// Compact card text like `R v2 p3 ◆` (effect marker only when present).
fn card_text(c: &CardView) -> String {
    let eff = if c.effect.is_some() { " ◆" } else { "" };
    format!(
        "{} v{} p{}{}",
        palette::glyph(c.color),
        c.volatility,
        c.points,
        eff
    )
}

fn card_spans(c: &CardView) -> Vec<Span<'static>> {
    vec![Span::styled(
        format!("[{}]", card_text(c)),
        Style::default().fg(card_color(c)),
    )]
}

/// A full human label for a card, used in toasts (e.g. an Exposed card).
pub(crate) fn card_label(c: &CardView) -> String {
    let eff = match c.effect {
        Some(e) => format!(" {}", effect_name(e)),
        None => String::new(),
    };
    format!(
        "{} v{} p{}{}",
        palette::name(c.color),
        c.volatility,
        c.points,
        eff
    )
}

fn card_color(c: &CardView) -> Color {
    palette::style(c.color)
}

fn effect_name(e: EffectKind) -> &'static str {
    match e {
        EffectKind::Peek => "Peek",
        EffectKind::Dampen => "Dampen",
        EffectKind::VolatileSurge => "Volatile Surge",
        EffectKind::Shield => "Shield",
        EffectKind::Expose => "Expose",
        EffectKind::Copycat => "Copycat",
        EffectKind::Recall => "Recall",
        EffectKind::DoubleDown => "Double Down",
    }
}

fn modifier_label(m: ModifierKind) -> &'static str {
    match m {
        ModifierKind::Residue => "Residue",
        ModifierKind::ThinIce => "Thin Ice",
        ModifierKind::DeepCauldron => "Deep Cauldron",
        ModifierKind::BountifulBrew => "Bountiful Brew",
        ModifierKind::DoubleStakes => "Double Stakes",
        ModifierKind::Reversal => "Reversal",
    }
}

fn modifier_short(m: ModifierKind) -> &'static str {
    match m {
        ModifierKind::Residue => "[Residue]",
        ModifierKind::ThinIce => "[ThinIce]",
        ModifierKind::DeepCauldron => "[DeepCauldron]",
        ModifierKind::BountifulBrew => "[Bountiful]",
        ModifierKind::DoubleStakes => "[2xStakes]",
        ModifierKind::Reversal => "[Reversal]",
    }
}

/// The qualitative effect of a modifier (direction only — magnitudes are
/// server-side balance config and never cross the wire).
fn modifier_desc(m: ModifierKind) -> &'static str {
    match m {
        ModifierKind::Residue => "cauldron starts hotter",
        ModifierKind::ThinIce => "boiling point lower — explosions likelier",
        ModifierKind::DeepCauldron => "boiling point higher — explosions rarer",
        ModifierKind::BountifulBrew => "every card adds to the pot's value",
        ModifierKind::DoubleStakes => "all pot points doubled — win and loss",
        ModifierKind::Reversal => "the lowest colour present wins instead",
    }
}

fn fmt_ms(ms: u32) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn bordered(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
}

fn hint(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(Span::styled(text, Style::default().fg(Color::DarkGray)))
            .block(Block::default().borders(Borders::TOP)),
        area,
    );
}

/// A `w`×`h` rectangle centred within `area` (clamped to fit).
fn center(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}
