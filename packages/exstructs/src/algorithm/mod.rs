mod dedupe;
pub use dedupe::*;
mod mark_and_sweep;
pub use mark_and_sweep::*;
mod permute;
pub use permute::*;
mod connected_components;
pub use connected_components::*;

pub mod merge;

// == structure implementations ==
mod contains_goff;
mod fullqual;
mod map_goff;
pub use map_goff::MapGoff;
mod mark;
mod mark_non_eliminateable;
mod replace;
