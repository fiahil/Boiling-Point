//! Game *content* — the part that churns during balance playtesting, kept
//! strictly separate from the game loop.
//!
//! Three distinct kinds, never merged into one union:
//! - [`card`]: dealt card data,
//! - [`effect`]: special-effect behaviour (Strategy),
//! - [`modifier`]: cauldron-modifier behaviour (Strategy).
//!
//! The [`registry::ContentRegistry`] is the single lookup the loop consults.

pub mod card;
pub mod effect;
pub mod modifier;
pub mod registry;

pub use card::CardDef;
pub use effect::{Effect, EffectCategory, EffectCtx};
pub use modifier::Modifier;
pub use registry::ContentRegistry;
