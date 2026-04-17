mod colors;
mod evaluate;
mod lookup;
mod tuple;
mod types;
mod variants;

pub use evaluate::evaluate_construct;
pub use lookup::StaticLookup;
pub use types::Datatype;
pub use variants::EnumItemFromNameAndValueName;
