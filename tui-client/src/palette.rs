//! Player-colour palette.
//!
//! Every colour is rendered as a single **letter** as well as a colour, so the
//! table stays legible on 16-colour terminals and for colour-blind players —
//! colour is never the sole signal (research R4). Truecolor values are used
//! where the terminal supports them; ratatui downsamples to the nearest ANSI
//! colour otherwise, so no explicit capability branch is needed in render code.

use boiling_point_protocol::Color as Wire;
use ratatui::style::Color;

/// The single-letter tag for a colour (R/B/G/A/W), shown alongside its colour.
pub(crate) fn letter(c: Wire) -> char {
    match c {
        Wire::Ruby => 'R',
        Wire::Sapphire => 'B',
        Wire::Emerald => 'G',
        Wire::Amethyst => 'A',
        Wire::Wild => 'W',
    }
}

/// The human-readable name of a colour.
pub(crate) fn name(c: Wire) -> &'static str {
    match c {
        Wire::Ruby => "Ruby",
        Wire::Sapphire => "Sapphire",
        Wire::Emerald => "Emerald",
        Wire::Amethyst => "Amethyst",
        Wire::Wild => "Wild",
    }
}

/// The ratatui colour used to paint a wire colour. Truecolor; ratatui maps it to
/// the terminal's nearest available colour on low-colour terminals.
pub(crate) fn style(c: Wire) -> Color {
    match c {
        Wire::Ruby => Color::Rgb(220, 50, 70),
        Wire::Sapphire => Color::Rgb(60, 120, 230),
        Wire::Emerald => Color::Rgb(50, 190, 110),
        Wire::Amethyst => Color::Rgb(180, 100, 220),
        Wire::Wild => Color::Rgb(190, 190, 190),
    }
}
