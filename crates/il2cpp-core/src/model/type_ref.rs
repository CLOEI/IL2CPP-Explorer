use serde::{Deserialize, Serialize};

use super::{GenericParameterId, TypeId};

/// Version-independent IL2CPP type expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeRef {
    Void,
    Boolean,
    Char,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    String,
    Object,
    IntPtr,
    UIntPtr,
    TypedReference,
    Type(TypeId),
    Array {
        element: Box<TypeRef>,
        rank: usize,
        zero_based: bool,
    },
    Pointer(Box<TypeRef>),
    ByRef(Box<TypeRef>),
    GenericParameter {
        id: GenericParameterId,
        name: String,
        method: bool,
    },
    GenericInstance {
        base: Box<TypeRef>,
        arguments: Vec<TypeRef>,
    },
    FunctionPointer,
    Unknown {
        type_index: Option<usize>,
        raw_type: Option<u8>,
    },
}
