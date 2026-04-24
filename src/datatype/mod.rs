mod colors;
mod evaluate;
mod lookup;
pub mod palette;
mod tuple;
mod types;
mod variants;

pub use evaluate::evaluate_construct;
#[cfg(feature = "typechecker")]
pub(crate) use evaluate::shorthand_rebind;
pub use lookup::StaticLookup;
pub use types::{Datatype, variant_type_name};
pub use variants::EnumItemFromNameAndValueName;
