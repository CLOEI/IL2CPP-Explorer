use serde::{Deserialize, Serialize};

use super::TypeId;

/// Stable index into a project's field collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldId(pub usize);

/// Normalized managed field and optional runtime offset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Field {
    pub id: FieldId,
    pub declaring_type: TypeId,
    pub name: String,
    pub offset: Option<u64>,
}
