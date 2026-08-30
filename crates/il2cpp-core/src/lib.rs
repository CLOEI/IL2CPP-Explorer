//! IL2CPP-specific parsing, normalized models, and analysis abstractions.

pub mod analysis;
pub mod binary;
pub mod error;
pub mod metadata;
pub mod model;
pub mod registration;

pub use error::{Error, Result};
