//! Server bootstrap: load and validate the content config, build the registry,
//! and (in later tasks) start the transport and room services.
//!
//! Fail-fast: an invalid content config aborts startup before any port is bound.

use boiling_point_server::config::ContentConfig;

/// The default content config, embedded so the binary always has a valid baseline.
const DEFAULT_CONFIG: &str = include_str!("../content.toml");

fn main() {
    let config = match ContentConfig::from_toml(DEFAULT_CONFIG) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to parse content config: {e}");
            std::process::exit(1);
        }
    };

    let registry = match config.build_registry() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("invalid content config: {e}");
            std::process::exit(1);
        }
    };

    let modifier_copies: u32 = registry
        .modifier_pool()
        .iter()
        .map(|(_, c)| *c as u32)
        .sum();
    println!(
        "Boiling Point server — content OK: {} cards in deck, {} modifiers in pool. \
         (transport/rooms/persistence land in later tasks)",
        registry.deck_size(),
        modifier_copies,
    );
}
