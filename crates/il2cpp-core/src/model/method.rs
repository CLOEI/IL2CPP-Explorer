use serde::{Deserialize, Serialize};

use super::{ParameterId, TypeId, TypeIndex};

/// Stable index into a project's method collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MethodId(pub usize);

/// Normalized managed method and optional native address mapping.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Method {
    pub id: MethodId,
    pub declaring_type: TypeId,
    pub name: String,
    pub return_type: TypeIndex,
    pub return_parameter_token: u32,
    pub parameters: Vec<ParameterId>,
    pub generic_container_index: Option<usize>,
    pub token: u32,
    pub flags: u16,
    pub implementation_flags: u16,
    pub slot: u16,
    pub relative_address: Option<u64>,
    pub virtual_address: Option<u64>,
}
