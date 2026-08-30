use serde::{Deserialize, Serialize};

use super::{MethodId, TypeId, TypeIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenericContainerId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenericParameterId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GenericOwner {
    Type(TypeId),
    Method(MethodId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenericContainer {
    pub id: GenericContainerId,
    pub owner: GenericOwner,
    pub parameters: Vec<GenericParameterId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenericParameter {
    pub id: GenericParameterId,
    pub container: GenericContainerId,
    pub name: String,
    pub position: u16,
    pub flags: u16,
    pub constraints: Vec<TypeIndex>,
}
