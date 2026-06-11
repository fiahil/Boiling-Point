//! Game *content* — the part that churns during balance playtesting, kept
//! strictly separate from the game loop.
//!
//! Three distinct kinds, never merged into one union:
//! - [`card`]: dealt ingredient data (pantry slots),
//! - [`spell`]: grimoire composition + tunable spell magnitudes,
//! - [`modifier`]: cauldron-modifier behaviour (Strategy).
//!
//! The [`registry::ContentRegistry`] is the single lookup the loop consults.

pub mod card;
pub mod modifier;
pub mod registry;
pub mod spell;

pub use card::{IngredientDef, PantrySlot};
pub use modifier::Modifier;
pub use registry::ContentRegistry;
pub use spell::{SpellDef, SpellValues};
