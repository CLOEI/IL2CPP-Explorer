use serde::{Deserialize, Serialize};

use super::{FieldId, MethodId};

/// Stable index into a project's type collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeId(pub usize);

/// Normalized managed type definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeDefinition {
    pub id: TypeId,
    pub namespace: String,
    pub name: String,
    pub methods: Vec<MethodId>,
    pub fields: Vec<FieldId>,
}
