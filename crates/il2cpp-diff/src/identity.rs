use std::fmt::{self, Write};

use il2cpp_core::analysis::TypeResolver;
use il2cpp_core::metadata::Metadata;
use il2cpp_core::model::{Method, TypeId, TypeRef};
use serde::{Deserialize, Serialize};

/// Cross-project managed type key. `declaring_path` is outermost-first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TypeIdentity {
    pub assembly: String,
    pub namespace: String,
    pub declaring_path: Vec<String>,
    pub name: String,
    pub generic_arity: usize,
}

impl fmt::Display for TypeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.assembly.is_empty() {
            write!(f, "{}:", self.assembly)?;
        }
        if !self.namespace.is_empty() {
            write!(f, "{}.", self.namespace)?;
        }
        for declaring in &self.declaring_path {
            write!(f, "{}+", declaring)?;
        }
        f.write_str(&self.name)
    }
}

/// Serializable normalized type expression. Primitives use canonical C# names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TypeIdentityRef {
    Primitive(String),
    Named(TypeIdentity),
    Array {
        element: Box<Self>,
        rank: usize,
        zero_based: bool,
    },
    Pointer(Box<Self>),
    ByRef(Box<Self>),
    GenericParameter {
        name: String,
        method: bool,
    },
    GenericInstance {
        base: Box<Self>,
        arguments: Vec<Self>,
    },
    FunctionPointer,
    Unknown(String),
}

impl fmt::Display for TypeIdentityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(v) | Self::Unknown(v) => f.write_str(v),
            Self::Named(v) => v.fmt(f),
            Self::Array { element, rank, .. } => {
                element.fmt(f)?;
                f.write_char('[')?;
                for _ in 1..*rank {
                    f.write_char(',')?;
                }
                f.write_char(']')
            }
            Self::Pointer(v) => write!(f, "{v}*"),
            Self::ByRef(v) => write!(f, "{v}&"),
            Self::GenericParameter { name, method } => {
                if *method {
                    write!(f, "!!{name}")
                } else {
                    write!(f, "!{name}")
                }
            }
            Self::GenericInstance { base, arguments } => {
                write!(f, "{base}<")?;
                for (i, item) in arguments.iter().enumerate() {
                    if i != 0 {
                        f.write_str(", ")?;
                    }
                    item.fmt(f)?;
                }
                f.write_char('>')
            }
            Self::FunctionPointer => f.write_str("fnptr"),
        }
    }
}

/// Stable full method identity. Never contains RVA, token, or local IDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MethodIdentity {
    pub declaring_type: TypeIdentity,
    pub name: String,
    pub generic_arity: usize,
    pub return_type: TypeIdentityRef,
    pub parameters: Vec<TypeIdentityRef>,
    pub is_static: bool,
}

impl fmt::Display for MethodIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}(", self.declaring_type, self.name)?;
        for (i, parameter) in self.parameters.iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?;
            }
            parameter.fmt(f)?;
        }
        f.write_char(')')
    }
}

/// Primary match key excludes return type. Return type changes remain `Changed`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MethodMatchKey {
    pub declaring_type: TypeIdentity,
    pub name: String,
    pub generic_arity: usize,
    pub parameters: Vec<TypeIdentityRef>,
    pub is_static: bool,
}

pub(crate) struct IdentityCache {
    pub types: Vec<TypeIdentity>,
    pub type_indexes: std::collections::HashMap<usize, TypeIdentity>,
    pub methods: Vec<MethodIdentity>,
    pub method_keys: Vec<MethodMatchKey>,
    pub field_types: Vec<TypeIdentityRef>,
}

impl IdentityCache {
    pub fn build(metadata: &Metadata, resolver: &TypeResolver<'_>) -> Self {
        let types = (0..metadata.types.len())
            .map(|i| type_identity(metadata, TypeId(i)))
            .collect::<Vec<_>>();
        let type_indexes = metadata
            .types
            .iter()
            .map(|ty| (ty.byval_type.0, types[ty.id.0].clone()))
            .collect::<std::collections::HashMap<_, _>>();
        let field_types = metadata
            .fields
            .iter()
            .map(|field| {
                normalize_type(
                    &types,
                    &type_indexes,
                    resolver.resolve(field.field_type).ok(),
                )
            })
            .collect();
        let methods = metadata
            .methods
            .iter()
            .map(|method| method_identity(metadata, &types, &type_indexes, resolver, method))
            .collect::<Vec<_>>();
        let method_keys = methods.iter().map(MethodMatchKey::from).collect();
        Self {
            types,
            type_indexes,
            methods,
            method_keys,
            field_types,
        }
    }
}

impl From<&MethodIdentity> for MethodMatchKey {
    fn from(value: &MethodIdentity) -> Self {
        Self {
            declaring_type: value.declaring_type.clone(),
            name: value.name.clone(),
            generic_arity: value.generic_arity,
            parameters: value.parameters.clone(),
            is_static: value.is_static,
        }
    }
}

fn method_identity(
    metadata: &Metadata,
    types: &[TypeIdentity],
    type_indexes: &std::collections::HashMap<usize, TypeIdentity>,
    resolver: &TypeResolver<'_>,
    method: &Method,
) -> MethodIdentity {
    let signature = resolver.method_signature(method).ok();
    let return_type = normalize_type(
        types,
        type_indexes,
        signature.as_ref().map(|v| v.return_type.clone()),
    );
    let parameters = signature.map_or_else(
        || {
            method
                .parameters
                .iter()
                .map(|id| {
                    normalize_type(
                        types,
                        type_indexes,
                        resolver
                            .resolve(metadata.parameters[id.0].parameter_type)
                            .ok(),
                    )
                })
                .collect()
        },
        |v| {
            v.parameters
                .into_iter()
                .map(|p| normalize_type(types, type_indexes, Some(p.ty)))
                .collect()
        },
    );
    MethodIdentity {
        declaring_type: types[method.declaring_type.0].clone(),
        name: method.name.clone(),
        generic_arity: method
            .generic_container_index
            .and_then(|i| metadata.generic_containers.get(i))
            .map_or(0, |v| v.parameters.len()),
        return_type,
        parameters,
        is_static: method.flags & 0x0010 != 0,
    }
}

fn type_identity(metadata: &Metadata, id: TypeId) -> TypeIdentity {
    let ty = &metadata.types[id.0];
    let image = &metadata.images[ty.image.0];
    let mut declaring_path = Vec::new();
    let mut current = ty.nested_in;
    while let Some(parent) = current {
        let parent = &metadata.types[parent.0];
        declaring_path.push(parent.name.clone());
        current = parent.nested_in;
    }
    declaring_path.reverse();
    TypeIdentity {
        assembly: metadata.assemblies[image.assembly.0].name.clone(),
        namespace: ty.namespace.clone(),
        declaring_path,
        name: ty.name.clone(),
        generic_arity: ty
            .generic_container_index
            .and_then(|i| metadata.generic_containers.get(i))
            .map_or(0, |v| v.parameters.len()),
    }
}

fn normalize_type(
    types: &[TypeIdentity],
    type_indexes: &std::collections::HashMap<usize, TypeIdentity>,
    value: Option<TypeRef>,
) -> TypeIdentityRef {
    match value.unwrap_or(TypeRef::Unknown {
        type_index: None,
        raw_type: None,
    }) {
        TypeRef::Void => primitive("void"),
        TypeRef::Boolean => primitive("bool"),
        TypeRef::Char => primitive("char"),
        TypeRef::I8 => primitive("sbyte"),
        TypeRef::U8 => primitive("byte"),
        TypeRef::I16 => primitive("short"),
        TypeRef::U16 => primitive("ushort"),
        TypeRef::I32 => primitive("int"),
        TypeRef::U32 => primitive("uint"),
        TypeRef::I64 => primitive("long"),
        TypeRef::U64 => primitive("ulong"),
        TypeRef::F32 => primitive("float"),
        TypeRef::F64 => primitive("double"),
        TypeRef::String => primitive("string"),
        TypeRef::Object => primitive("object"),
        TypeRef::IntPtr => primitive("System.IntPtr"),
        TypeRef::UIntPtr => primitive("System.UIntPtr"),
        TypeRef::TypedReference => primitive("System.TypedReference"),
        TypeRef::Type(id) => types
            .get(id.0)
            .cloned()
            .map(TypeIdentityRef::Named)
            .unwrap_or_else(|| unknown("type")),
        TypeRef::Array {
            element,
            rank,
            zero_based,
        } => TypeIdentityRef::Array {
            element: Box::new(normalize_type(types, type_indexes, Some(*element))),
            rank,
            zero_based,
        },
        TypeRef::Pointer(v) => {
            TypeIdentityRef::Pointer(Box::new(normalize_type(types, type_indexes, Some(*v))))
        }
        TypeRef::ByRef(v) => {
            TypeIdentityRef::ByRef(Box::new(normalize_type(types, type_indexes, Some(*v))))
        }
        TypeRef::GenericParameter { name, method, .. } => {
            TypeIdentityRef::GenericParameter { name, method }
        }
        TypeRef::GenericInstance { base, arguments } => TypeIdentityRef::GenericInstance {
            base: Box::new(normalize_type(types, type_indexes, Some(*base))),
            arguments: arguments
                .into_iter()
                .map(|v| normalize_type(types, type_indexes, Some(v)))
                .collect(),
        },
        TypeRef::FunctionPointer => TypeIdentityRef::FunctionPointer,
        TypeRef::Unknown {
            type_index: Some(index),
            raw_type: None,
        } => type_indexes
            .get(&index)
            .cloned()
            .map(TypeIdentityRef::Named)
            .unwrap_or_else(|| TypeIdentityRef::Unknown(format!("unknown:{index}:none"))),
        TypeRef::Unknown {
            type_index,
            raw_type,
        } => TypeIdentityRef::Unknown(format!(
            "unknown:{}:{}",
            type_index.map_or_else(|| "none".to_owned(), |v| v.to_string()),
            raw_type.map_or_else(|| "none".to_owned(), |v| v.to_string())
        )),
    }
}

fn primitive(value: &str) -> TypeIdentityRef {
    TypeIdentityRef::Primitive(value.to_owned())
}
fn unknown(value: &str) -> TypeIdentityRef {
    TypeIdentityRef::Unknown(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(name: &str) -> TypeIdentity {
        TypeIdentity {
            assembly: "Assembly-CSharp.dll".to_owned(),
            namespace: "Game".to_owned(),
            declaring_path: vec![],
            name: name.to_owned(),
            generic_arity: 0,
        }
    }

    #[test]
    fn method_match_key_keeps_overloads_distinct() {
        let left = MethodIdentity {
            declaring_type: ty("Player"),
            name: "Move".to_owned(),
            generic_arity: 0,
            return_type: primitive("void"),
            parameters: vec![TypeIdentityRef::Named(ty("Vector3"))],
            is_static: false,
        };
        let right = MethodIdentity {
            parameters: vec![primitive("float"), primitive("float")],
            ..left.clone()
        };
        assert_ne!(MethodMatchKey::from(&left), MethodMatchKey::from(&right));
    }

    #[test]
    fn return_type_is_outside_primary_key_for_changed_detection() {
        let left = MethodIdentity {
            declaring_type: ty("Player"),
            name: "Value".to_owned(),
            generic_arity: 0,
            return_type: primitive("int"),
            parameters: vec![],
            is_static: false,
        };
        let right = MethodIdentity {
            return_type: primitive("long"),
            ..left.clone()
        };
        assert_eq!(MethodMatchKey::from(&left), MethodMatchKey::from(&right));
        assert_ne!(left, right);
    }
}
