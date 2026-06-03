//! Player-colour palette.
//!
//! Every colour is shown with a distinct **emoji glyph** as well as its colour,
//! so a player is identifiable by *shape* (triangle / heart / circle / square /
//! lion) and not by colour alone — keeping the table legible for colour-blind
//! players and on terminals that downsample colours (research R4). The glyphs
//! double as the player's icon throughout the UI. Truecolor values are used
//! where the terminal supports them; ratatui maps them to the nearest ANSI
//! colour otherwise, so no explicit capability branch is needed in render code.

use boiling_point_protocol::Color as Wire;
use ratatui::style::Color;

/// The emoji glyph for a colour, used as the player's icon. Each is a distinct
/// shape so colour is never the sole signal.
pub(crate) fn glyph(c: Wire) -> &'static str {
    match c {
        Wire::Ruby => "🔻",
        Wire::Sapphire => "💙",
        Wire::Emerald => "🟢",
        Wire::Amethyst => "🟪",
        Wire::Wild => "🦁",
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
