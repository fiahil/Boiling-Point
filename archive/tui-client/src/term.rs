//! Terminal lifecycle: enter raw mode + the alternate screen on start, and
//! restore cooked mode on a clean exit *and* on panic (a panic hook), so a
//! crash never leaves the user's terminal wedged (`tui-client-shell`).

use std::io::{self, Stdout};

use crossterm::{
    cursor::Show,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

/// Enter raw mode + the alternate screen, install a panic hook that restores
/// the terminal, and return a ratatui terminal bound to stdout.
pub(crate) fn init() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        prev(info);
    }));

    Terminal::new(CrosstermBackend::new(io::stdout()))
}

/// Restore the terminal to cooked mode and leave the alternate screen.
pub(crate) fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, Show)
}
