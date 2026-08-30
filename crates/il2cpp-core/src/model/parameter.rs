use serde::{Deserialize, Serialize};

use super::{MethodId, TypeIndex};

/// Stable index into a project's parameter collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParameterId(pub usize);

/// Normalized managed method parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Parameter {
    pub id: ParameterId,
    pub declaring_method: MethodId,
    pub name: String,
    pub parameter_type: TypeIndex,
    pub token: u32,
}
