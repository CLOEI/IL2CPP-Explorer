//! Pluggable discovery of IL2CPP runtime registration structures.

mod heuristic;
mod manual;
mod resolver;
mod symbols;

use serde::{Deserialize, Serialize};

pub use heuristic::HeuristicResolver;
pub use manual::ManualResolver;
pub use resolver::RegistrationResolver;
pub use symbols::SymbolResolver;

/// Addresses of the native IL2CPP registration structures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Registration {
    pub code_registration: Option<u64>,
    pub metadata_registration: Option<u64>,
}
