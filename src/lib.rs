//! Game-neutral parsing infrastructure for Valve Source 2 demo files.
//!
//! The core crate owns the game-independent PBDEMS2 container, bit encodings,
//! flattened serializers, entity state, string tables, and coordinate helpers.
//! Game crates provide their generated protobuf types and select an entity
//! decoding dialect.

pub mod demo;
pub mod entity;
pub mod error;
pub mod io;
pub mod limits;
#[cfg(feature = "mmap")]
pub mod mmap;
pub mod playback;
pub mod position;

#[cfg(test)]
mod test_utils;

pub use entity::Entity;
pub use error::{Error, Result};
pub use limits::DecodeLimits;
#[cfg(feature = "mmap")]
pub use mmap::MappedDemo;
pub use playback::{CommandContext, DemoAdapter, DemoParser, ParserState};
