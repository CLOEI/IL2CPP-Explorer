use serde::{Deserialize, Serialize};

use super::{AssemblyId, TypeId};

/// Stable index into a project's managed image collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageId(pub usize);

/// Normalized managed image belonging to an assembly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Image {
    pub id: ImageId,
    pub assembly: AssemblyId,
    pub name: String,
    pub types: Vec<TypeId>,
}
