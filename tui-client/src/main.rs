//! Binary entry point for the Boiling Point terminal client. All behaviour lives
//! in the library so it stays testable; this is a thin wrapper around
//! [`boiling_point_tui::run`].

fn main() -> std::process::ExitCode {
    boiling_point_tui::run()
}
