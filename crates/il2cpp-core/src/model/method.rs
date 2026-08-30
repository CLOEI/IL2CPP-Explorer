use serde::{Deserialize, Serialize};

use super::TypeId;

/// Stable index into a project's method collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MethodId(pub usize);

/// Normalized managed method and optional native address mapping.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Method {
    pub id: MethodId,
    pub declaring_type: TypeId,
    pub name: String,
    pub relative_address: Option<u64>,
    pub virtual_address: Option<u64>,
}
