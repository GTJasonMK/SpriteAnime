mod erase;
mod export;
mod io;
mod matting;
mod sprite_layout;

use std::sync::atomic::AtomicU64;

static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub use erase::*;
pub use export::*;
pub use io::*;
pub use matting::*;
pub use sprite_layout::*;

#[cfg(test)]
mod tests;
