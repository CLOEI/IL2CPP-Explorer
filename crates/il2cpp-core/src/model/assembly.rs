use serde::{Deserialize, Serialize};

/// Stable index into a project's assembly collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssemblyId(pub usize);

/// Normalized managed assembly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Assembly {
    pub id: AssemblyId,
    pub name: String,
}
