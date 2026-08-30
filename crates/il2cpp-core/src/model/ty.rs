use serde::{Deserialize, Serialize};

use super::{FieldId, ImageId, MethodId};

/// Stable index into a project's type collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeId(pub usize);

/// Index into runtime IL2CPP type metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeIndex(pub usize);

/// Normalized managed type definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeDefinition {
    pub id: TypeId,
    pub image: ImageId,
    pub namespace: String,
    pub name: String,
    pub declaring_type: Option<TypeIndex>,
    pub parent: Option<TypeIndex>,
    pub generic_container_index: Option<usize>,
    pub methods: Vec<MethodId>,
    pub fields: Vec<FieldId>,
}
