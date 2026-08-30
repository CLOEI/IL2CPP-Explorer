use serde::{Deserialize, Serialize};

use super::{MethodId, TypeId};

/// Stable index into a project's property collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PropertyId(pub usize);

/// Normalized managed property.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Property {
    pub id: PropertyId,
    pub declaring_type: TypeId,
    pub name: String,
    pub getter: Option<MethodId>,
    pub setter: Option<MethodId>,
    pub attributes: u32,
    pub token: u32,
}
