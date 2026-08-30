use il2cpp_core::metadata::Metadata;
use il2cpp_core::model::{TypeId, TypeRef};

use super::identifier::{identifier, without_generic_arity};

pub struct CSharpTypeRenderer<'a> {
    metadata: &'a Metadata,
    fully_qualified: bool,
}

impl<'a> CSharpTypeRenderer<'a> {
    pub fn new(metadata: &'a Metadata, fully_qualified: bool) -> Self {
        Self {
            metadata,
            fully_qualified,
        }
    }

    pub fn format(&self, ty: &TypeRef) -> String {
        match ty {
            TypeRef::Void => "void".to_owned(),
            TypeRef::Boolean => "bool".to_owned(),
            TypeRef::Char => "char".to_owned(),
            TypeRef::I8 => "sbyte".to_owned(),
            TypeRef::U8 => "byte".to_owned(),
            TypeRef::I16 => "short".to_owned(),
            TypeRef::U16 => "ushort".to_owned(),
            TypeRef::I32 => "int".to_owned(),
            TypeRef::U32 => "uint".to_owned(),
            TypeRef::I64 => "long".to_owned(),
            TypeRef::U64 => "ulong".to_owned(),
            TypeRef::F32 => "float".to_owned(),
            TypeRef::F64 => "double".to_owned(),
            TypeRef::String => "string".to_owned(),
            TypeRef::Object => "object".to_owned(),
            TypeRef::IntPtr => "System.IntPtr".to_owned(),
            TypeRef::UIntPtr => "System.UIntPtr".to_owned(),
            TypeRef::TypedReference => "System.TypedReference".to_owned(),
            TypeRef::Type(id) => self.type_name(*id, true),
            TypeRef::Array { element, rank, .. } => {
                format!(
                    "{}[{}]",
                    self.format(element),
                    ",".repeat(rank.saturating_sub(1))
                )
            }
            TypeRef::Pointer(element) => format!("{}*", self.format(element)),
            TypeRef::ByRef(element) => format!("ref {}", self.format(element)),
            TypeRef::GenericParameter { name, .. } => identifier(name).rendered.into_owned(),
            TypeRef::GenericInstance { base, arguments } => {
                if let TypeRef::Type(id) = base.as_ref() {
                    self.generic_instance_name(*id, arguments)
                } else {
                    let arguments = arguments
                        .iter()
                        .map(|argument| self.format(argument))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}<{arguments}>", self.format(base))
                }
            }
            TypeRef::FunctionPointer => "/* function pointer */ void*".to_owned(),
            TypeRef::Unknown {
                type_index,
                raw_type,
            } => match (type_index, raw_type) {
                (Some(index), _) => format!("/* unresolved type index {index} */ object"),
                (_, Some(raw)) => format!("/* unsupported IL2CPP type 0x{raw:02X} */ object"),
                _ => "/* unresolved type */ object".to_owned(),
            },
        }
    }

    pub fn type_name(&self, id: TypeId, include_definition_arguments: bool) -> String {
        let chain = self.type_chain(id);
        let mut inherited = 0;
        let mut segments = Vec::with_capacity(chain.len());
        for type_id in &chain {
            let ty = &self.metadata.types[type_id.0];
            let mut segment = identifier(without_generic_arity(&ty.name))
                .rendered
                .into_owned();
            if include_definition_arguments
                && let Some(container) = ty
                    .generic_container_index
                    .and_then(|index| self.metadata.generic_containers.get(index))
            {
                let parameters = container
                    .parameters
                    .iter()
                    .skip(inherited)
                    .map(|parameter| {
                        identifier(&self.metadata.generic_parameters[parameter.0].name)
                            .rendered
                            .into_owned()
                    })
                    .collect::<Vec<_>>();
                append_arguments(&mut segment, &parameters);
                inherited = container.parameters.len();
            }
            segments.push(segment);
        }
        self.qualify(&chain, segments.join("."))
    }

    fn generic_instance_name(&self, id: TypeId, arguments: &[TypeRef]) -> String {
        let chain = self.type_chain(id);
        let mut inherited = 0;
        let mut segments = Vec::with_capacity(chain.len());
        for type_id in &chain {
            let ty = &self.metadata.types[type_id.0];
            let mut segment = identifier(without_generic_arity(&ty.name))
                .rendered
                .into_owned();
            let count = ty
                .generic_container_index
                .and_then(|index| self.metadata.generic_containers.get(index))
                .map_or(inherited, |container| container.parameters.len());
            let parameters = arguments
                .get(inherited.min(arguments.len())..count.min(arguments.len()))
                .unwrap_or_default()
                .iter()
                .map(|argument| self.format(argument))
                .collect::<Vec<_>>();
            append_arguments(&mut segment, &parameters);
            inherited = count;
            segments.push(segment);
        }
        self.qualify(&chain, segments.join("."))
    }

    fn type_chain(&self, id: TypeId) -> Vec<TypeId> {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(type_id) = current {
            chain.push(type_id);
            current = self.metadata.types[type_id.0].nested_in;
        }
        chain.reverse();
        chain
    }

    fn qualify(&self, chain: &[TypeId], name: String) -> String {
        let namespace = &self.metadata.types[chain[0].0].namespace;
        if self.fully_qualified && !namespace.is_empty() {
            format!("{namespace}.{name}")
        } else {
            name
        }
    }
}

fn append_arguments(segment: &mut String, arguments: &[String]) {
    if !arguments.is_empty() {
        segment.push('<');
        segment.push_str(&arguments.join(", "));
        segment.push('>');
    }
}

#[cfg(test)]
mod tests {
    use il2cpp_core::metadata::{METADATA_SANITY, Metadata};
    use il2cpp_core::model::{
        GenericContainer, GenericContainerId, GenericOwner, GenericParameter, GenericParameterId,
        ImageId, TypeDefinition, TypeId, TypeIndex, TypeRef,
    };

    use super::*;

    fn empty_metadata() -> Metadata {
        let mut bytes = vec![0; 256];
        bytes[0..4].copy_from_slice(&METADATA_SANITY.to_le_bytes());
        bytes[4..8].copy_from_slice(&31_u32.to_le_bytes());
        Metadata::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn formats_primitives_arrays_pointers_byrefs_and_generics() {
        let metadata = empty_metadata();
        let renderer = CSharpTypeRenderer::new(&metadata, false);
        assert_eq!(renderer.format(&TypeRef::Void), "void");
        assert_eq!(renderer.format(&TypeRef::Boolean), "bool");
        assert_eq!(renderer.format(&TypeRef::I32), "int");
        assert_eq!(renderer.format(&TypeRef::U32), "uint");
        assert_eq!(renderer.format(&TypeRef::I64), "long");
        assert_eq!(renderer.format(&TypeRef::F32), "float");
        assert_eq!(renderer.format(&TypeRef::F64), "double");
        assert_eq!(renderer.format(&TypeRef::String), "string");
        assert_eq!(renderer.format(&TypeRef::Object), "object");
        assert_eq!(
            renderer.format(&TypeRef::Array {
                element: Box::new(TypeRef::I32),
                rank: 2,
                zero_based: false,
            }),
            "int[,]"
        );
        assert_eq!(
            renderer.format(&TypeRef::Pointer(Box::new(TypeRef::Void))),
            "void*"
        );
        assert_eq!(
            renderer.format(&TypeRef::ByRef(Box::new(TypeRef::I32))),
            "ref int"
        );
        assert_eq!(
            renderer.format(&TypeRef::GenericParameter {
                id: GenericParameterId(0),
                name: "T".to_owned(),
                method: false,
            }),
            "T"
        );
        assert_eq!(
            renderer.format(&TypeRef::Unknown {
                type_index: Some(42),
                raw_type: None,
            }),
            "/* unresolved type index 42 */ object"
        );
    }

    #[test]
    fn distributes_nested_generic_arguments_across_declaring_types() {
        let mut metadata = empty_metadata();
        metadata.generic_parameters = ["TKey", "TValue", "TKey", "TValue"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| GenericParameter {
                id: GenericParameterId(index),
                container: GenericContainerId(usize::from(index >= 2)),
                name: name.to_owned(),
                position: (index % 2) as u16,
                flags: 0,
                constraints: Vec::new(),
            })
            .collect();
        metadata.generic_containers = vec![
            GenericContainer {
                id: GenericContainerId(0),
                owner: GenericOwner::Type(TypeId(0)),
                parameters: vec![GenericParameterId(0), GenericParameterId(1)],
            },
            GenericContainer {
                id: GenericContainerId(1),
                owner: GenericOwner::Type(TypeId(1)),
                parameters: vec![GenericParameterId(2), GenericParameterId(3)],
            },
        ];
        metadata.types = vec![
            type_definition(0, "Dictionary`2", None, 0),
            type_definition(1, "Enumerator", Some(TypeId(0)), 1),
        ];
        let renderer = CSharpTypeRenderer::new(&metadata, false);

        assert_eq!(
            renderer.type_name(TypeId(1), true),
            "Dictionary<TKey, TValue>.Enumerator"
        );
        assert_eq!(
            renderer.format(&TypeRef::GenericInstance {
                base: Box::new(TypeRef::Type(TypeId(1))),
                arguments: vec![TypeRef::I32, TypeRef::String],
            }),
            "Dictionary<int, string>.Enumerator"
        );
    }

    fn type_definition(
        id: usize,
        name: &str,
        nested_in: Option<TypeId>,
        generic_container_index: usize,
    ) -> TypeDefinition {
        TypeDefinition {
            id: TypeId(id),
            image: ImageId(0),
            namespace: "System.Collections.Generic".to_owned(),
            name: name.to_owned(),
            byval_type: TypeIndex(id),
            declaring_type: None,
            parent: None,
            element_type: None,
            generic_container_index: Some(generic_container_index),
            flags: 1,
            bitfield: 0,
            token: 0,
            methods: Vec::new(),
            fields: Vec::new(),
            properties: Vec::new(),
            nested_types: Vec::new(),
            nested_in,
            interfaces: Vec::new(),
        }
    }
}
