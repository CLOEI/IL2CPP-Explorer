use serde::{Deserialize, Serialize};

use crate::native::NativeDiff;
use crate::{DiffStatus, MethodIdentity, TypeIdentity, TypeIdentityRef};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssemblyDiff {
    pub name: String,
    pub status: DiffStatus,
    pub old_present: bool,
    pub new_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDiff {
    pub name: String,
    pub status: DiffStatus,
    pub old_type: Option<TypeIdentityRef>,
    pub new_type: Option<TypeIdentityRef>,
    pub old_offset: Option<i32>,
    pub new_offset: Option<i32>,
    pub old_static: Option<bool>,
    pub new_static: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDiff {
    pub name: String,
    pub status: DiffStatus,
    pub old_getter: Option<String>,
    pub new_getter: Option<String>,
    pub old_setter: Option<String>,
    pub new_setter: Option<String>,
    pub old_attributes: Option<u32>,
    pub new_attributes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodDiff {
    pub identity: MethodIdentity,
    pub status: DiffStatus,
    pub old_rva: Option<u64>,
    pub new_rva: Option<u64>,
    pub old_va: Option<u64>,
    pub new_va: Option<u64>,
    pub old_token: Option<u32>,
    pub new_token: Option<u32>,
    pub native: Option<NativeDiff>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeDiff {
    pub identity: TypeIdentity,
    pub status: DiffStatus,
    pub old_base: Option<TypeIdentityRef>,
    pub new_base: Option<TypeIdentityRef>,
    pub old_interfaces: Vec<TypeIdentityRef>,
    pub new_interfaces: Vec<TypeIdentityRef>,
    pub old_flags: Option<u32>,
    pub new_flags: Option<u32>,
    pub fields: Vec<FieldDiff>,
    pub properties: Vec<PropertyDiff>,
    pub methods: Vec<MethodDiff>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub assemblies_added: usize,
    pub assemblies_removed: usize,
    pub types_added: usize,
    pub types_removed: usize,
    pub types_changed: usize,
    pub methods_added: usize,
    pub methods_removed: usize,
    pub methods_changed: usize,
    pub methods_moved: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDiff {
    pub assemblies: Vec<AssemblyDiff>,
    pub types: Vec<TypeDiff>,
    pub summary: DiffSummary,
    pub native_available: bool,
}
