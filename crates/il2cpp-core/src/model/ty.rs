use serde::{Deserialize, Serialize};

use super::{FieldId, ImageId, MethodId, PropertyId};

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
    pub byval_type: TypeIndex,
    pub declaring_type: Option<TypeIndex>,
    pub parent: Option<TypeIndex>,
    pub element_type: Option<TypeIndex>,
    pub generic_container_index: Option<usize>,
    pub flags: u32,
    pub bitfield: u32,
    pub token: u32,
    pub methods: Vec<MethodId>,
    pub fields: Vec<FieldId>,
    pub properties: Vec<PropertyId>,
    pub nested_types: Vec<TypeId>,
    pub nested_in: Option<TypeId>,
    pub interfaces: Vec<TypeIndex>,
}

/// C#-relevant category encoded by one metadata type definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeKind {
    Class,
    Struct,
    Interface,
    Enum,
}

impl TypeDefinition {
    pub fn kind(&self) -> TypeKind {
        const INTERFACE: u32 = 0x20;
        const VALUE_TYPE: u32 = 1 << 0;
        const ENUM_TYPE: u32 = 1 << 1;

        if self.flags & INTERFACE != 0 {
            TypeKind::Interface
        } else if self.bitfield & ENUM_TYPE != 0 {
            TypeKind::Enum
        } else if self.bitfield & VALUE_TYPE != 0 {
            TypeKind::Struct
        } else {
            TypeKind::Class
        }
    }
}
