use std::collections::BTreeMap;
use std::io::Write;

use anyhow::Result;
use il2cpp_core::analysis::{FieldSignature, MethodSignature, ResolvedParameter};
use il2cpp_core::model::{Method, TypeDefinition, TypeId, TypeIndex, TypeKind, TypeRef};

use crate::{ExportContext, Exporter};

use super::identifier::{identifier, without_generic_arity};
use super::options::DumpCsOptions;
use super::type_renderer::CSharpTypeRenderer;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DumpCsSummary {
    pub assemblies: usize,
    pub types: usize,
    pub fields: usize,
    pub properties: usize,
    pub methods: usize,
    pub native_methods: usize,
    pub unresolved_type_references: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DumpCsExporter {
    pub options: DumpCsOptions,
}

impl DumpCsExporter {
    pub fn new(options: DumpCsOptions) -> Self {
        Self { options }
    }
}

impl Exporter for DumpCsExporter {
    type Summary = DumpCsSummary;

    fn export<W: Write>(
        &self,
        context: &ExportContext<'_>,
        writer: &mut W,
    ) -> Result<Self::Summary> {
        Renderer::new(context, writer, self.options).render()
    }
}

struct Renderer<'a, 'writer, W> {
    context: &'a ExportContext<'a>,
    writer: &'writer mut W,
    options: DumpCsOptions,
    type_renderer: CSharpTypeRenderer<'a>,
    summary: DumpCsSummary,
}

impl<'a, 'writer, W: Write> Renderer<'a, 'writer, W> {
    fn new(context: &'a ExportContext<'a>, writer: &'writer mut W, options: DumpCsOptions) -> Self {
        let metadata = context.metadata;
        Self {
            context,
            writer,
            options,
            type_renderer: CSharpTypeRenderer::new(metadata, options.fully_qualified_types),
            summary: DumpCsSummary {
                assemblies: metadata.assemblies.len(),
                types: metadata.types.len(),
                fields: metadata.fields.len(),
                properties: metadata.properties.len(),
                methods: metadata.methods.len(),
                native_methods: context
                    .native_methods
                    .map_or(0, |methods| methods.mapped_method_count()),
                unresolved_type_references: 0,
            },
        }
    }

    fn render(mut self) -> Result<DumpCsSummary> {
        writeln!(self.writer, "// IL2CPP Explorer dump.cs")?;
        writeln!(
            self.writer,
            "// Metadata version: {}",
            self.context.metadata.version.raw()
        )?;
        writeln!(self.writer)?;

        for image in &self.context.metadata.images {
            writeln!(
                self.writer,
                "// ========================================================"
            )?;
            writeln!(self.writer, "// Assembly: {}", image.name)?;
            writeln!(
                self.writer,
                "// ========================================================"
            )?;
            writeln!(self.writer)?;

            let mut namespaces = BTreeMap::<&str, Vec<TypeId>>::new();
            for type_id in &image.types {
                let ty = &self.context.metadata.types[type_id.0];
                if ty.nested_in.is_none() {
                    namespaces.entry(&ty.namespace).or_default().push(*type_id);
                }
            }
            for (namespace, types) in namespaces {
                writeln!(
                    self.writer,
                    "// Namespace: {}",
                    if namespace.is_empty() {
                        "<global>"
                    } else {
                        namespace
                    }
                )?;
                for type_id in types {
                    self.render_type(type_id, 0)?;
                    writeln!(self.writer)?;
                }
            }
        }
        Ok(self.summary)
    }

    fn render_type(&mut self, type_id: TypeId, depth: usize) -> Result<()> {
        let ty = &self.context.metadata.types[type_id.0];
        let indent = indent(depth);
        if self.options.include_indices {
            writeln!(self.writer, "{indent}// TypeDefIndex: {}", type_id.0)?;
        }
        if self.options.include_tokens {
            writeln!(self.writer, "{indent}// Token: {:#010X}", ty.token)?;
        }
        let raw_name = without_generic_arity(&ty.name);
        let name = identifier(raw_name);
        if name.changed {
            writeln!(self.writer, "{indent}// Metadata name: {}", ty.name)?;
        }

        let mut declaration = type_modifiers(ty);
        declaration.push_str(match ty.kind() {
            TypeKind::Class => "class ",
            TypeKind::Struct => "struct ",
            TypeKind::Interface => "interface ",
            TypeKind::Enum => "enum ",
        });
        declaration.push_str(&name.rendered);
        let inherited_generic_parameters = self.inherited_generic_parameter_count(ty);
        declaration.push_str(&self.generic_declaration(ty, inherited_generic_parameters));
        let enum_underlying = self.enum_underlying_type(ty);
        if ty.kind() == TypeKind::Enum
            && let Some(underlying) = &enum_underlying
            && !is_enum_underlying_type(underlying)
        {
            let underlying = self.format_type(underlying);
            writeln!(self.writer, "{indent}// Underlying type: {underlying}")?;
        }
        let bases = self.type_bases(ty, enum_underlying.as_ref());
        if !bases.is_empty() {
            declaration.push_str(" : ");
            declaration.push_str(&bases.join(", "));
        }
        writeln!(self.writer, "{indent}{declaration}")?;
        for constraint in
            self.generic_constraints(ty.generic_container_index, inherited_generic_parameters)
        {
            writeln!(self.writer, "{indent}    {constraint}")?;
        }
        writeln!(self.writer, "{indent}{{")?;

        if !ty.fields.is_empty() {
            writeln!(self.writer, "{indent}    // Fields")?;
            writeln!(self.writer)?;
            for field in &ty.fields {
                self.render_field(*field, ty.kind(), depth + 1)?;
            }
        }
        if !ty.properties.is_empty() {
            writeln!(self.writer, "{indent}    // Properties")?;
            writeln!(self.writer)?;
            for property in &ty.properties {
                self.render_property(*property, depth + 1)?;
            }
        }
        if !ty.methods.is_empty() {
            writeln!(self.writer, "{indent}    // Methods")?;
            writeln!(self.writer)?;
            for method in &ty.methods {
                self.render_method(*method, depth + 1)?;
            }
        }
        for nested in &ty.nested_types {
            writeln!(self.writer)?;
            self.render_type(*nested, depth + 1)?;
        }
        writeln!(self.writer, "{indent}}}")?;
        Ok(())
    }

    fn render_field(
        &mut self,
        field_id: il2cpp_core::model::FieldId,
        owner_kind: TypeKind,
        depth: usize,
    ) -> Result<()> {
        const LITERAL: u16 = 0x40;
        let field = &self.context.metadata.fields[field_id.0];
        let signature = self.field_signature(field);
        let ty = self.format_type(&signature.ty);
        let name = identifier(&field.name);
        let indent = indent(depth);
        if owner_kind == TypeKind::Enum && field.name == "value__" {
            return Ok(());
        }
        if name.changed {
            writeln!(self.writer, "{indent}// Metadata name: {}", field.name)?;
        }
        if self.options.include_indices {
            writeln!(self.writer, "{indent}// FieldIndex: {}", field.id.0)?;
        }
        if self.options.include_tokens {
            writeln!(self.writer, "{indent}// Token: {:#010X}", field.token)?;
        }
        if owner_kind == TypeKind::Enum {
            writeln!(
                self.writer,
                "{indent}// {} {} // constant value unavailable",
                ty, name.rendered
            )?;
            return Ok(());
        }

        let mut declaration = field_modifiers(signature.attributes);
        declaration.push_str(&ty);
        declaration.push(' ');
        declaration.push_str(&name.rendered);
        declaration.push(';');
        if self.options.include_field_offsets
            && signature.attributes & LITERAL == 0
            && let Some(offset) = signature.offset.filter(|offset| *offset >= 0)
        {
            declaration.push_str(&format!(" // 0x{offset:X}"));
        }
        if signature.attributes & LITERAL != 0 {
            writeln!(
                self.writer,
                "{indent}// {declaration} // constant value unavailable"
            )?;
        } else {
            writeln!(self.writer, "{indent}{declaration}")?;
        }
        Ok(())
    }

    fn render_property(
        &mut self,
        property_id: il2cpp_core::model::PropertyId,
        depth: usize,
    ) -> Result<()> {
        let property = &self.context.metadata.properties[property_id.0];
        let getter = property
            .getter
            .map(|method| &self.context.metadata.methods[method.0]);
        let setter = property
            .setter
            .map(|method| &self.context.metadata.methods[method.0]);
        let accessor = match (getter, setter) {
            (Some(getter), Some(setter)) => {
                Some(if access_rank(getter.flags) >= access_rank(setter.flags) {
                    getter
                } else {
                    setter
                })
            }
            (Some(getter), None) => Some(getter),
            (None, Some(setter)) => Some(setter),
            (None, None) => None,
        };
        let flags = accessor.map_or(0, |method| method.flags);
        let (property_type, flags) = if let Some(getter) = getter {
            (self.resolve_type(getter.return_type), flags)
        } else if let Some(setter) = setter {
            let parameter = setter
                .parameters
                .last()
                .map(|parameter| self.context.metadata.parameters[parameter.0].parameter_type);
            (
                parameter.map_or(
                    TypeRef::Unknown {
                        type_index: None,
                        raw_type: None,
                    },
                    |index| self.resolve_type(index),
                ),
                flags,
            )
        } else {
            (
                TypeRef::Unknown {
                    type_index: None,
                    raw_type: None,
                },
                0,
            )
        };
        let property_type = self.format_type(&property_type);
        let name = identifier(&property.name);
        let indent = indent(depth);
        if name.changed {
            writeln!(self.writer, "{indent}// Metadata name: {}", property.name)?;
        }
        if self.options.include_indices {
            writeln!(self.writer, "{indent}// PropertyIndex: {}", property.id.0)?;
        }
        if self.options.include_tokens {
            writeln!(self.writer, "{indent}// Token: {:#010X}", property.token)?;
        }
        let mut accessors = Vec::new();
        if let Some(getter) = getter {
            accessors.push(property_accessor("get", getter.flags, flags));
        }
        if let Some(setter) = setter {
            accessors.push(property_accessor("set", setter.flags, flags));
        }
        writeln!(
            self.writer,
            "{indent}{}{} {} {{ {} }}",
            property_modifiers(flags),
            property_type,
            name.rendered,
            accessors.join(" ")
        )?;
        Ok(())
    }

    fn render_method(
        &mut self,
        method_id: il2cpp_core::model::MethodId,
        depth: usize,
    ) -> Result<()> {
        let method = &self.context.metadata.methods[method_id.0];
        let signature = self.method_signature(method);
        let indent = indent(depth);
        if self.options.include_indices {
            writeln!(self.writer, "{indent}// MethodIndex: {}", method.id.0)?;
        }
        if self.options.include_tokens {
            writeln!(self.writer, "{indent}// Token: {:#010X}", method.token)?;
        }
        if self.options.include_addresses
            && let Some(address) = self
                .context
                .native_methods
                .and_then(|methods| methods.address_of(method.id))
        {
            writeln!(
                self.writer,
                "{indent}// RVA: {:#X}",
                address.relative_address
            )?;
            writeln!(
                self.writer,
                "{indent}// VA:  {:#X}",
                address.virtual_address
            )?;
            if self.options.include_file_offsets {
                writeln!(
                    self.writer,
                    "{indent}// File Offset: {:#X}",
                    address.file_offset
                )?;
            }
        }

        let constructor = method.name == ".ctor" || method.name == ".cctor";
        let method_name = if constructor {
            let owner = &self.context.metadata.types[method.declaring_type.0];
            identifier(without_generic_arity(&owner.name))
        } else {
            identifier(&method.name)
        };
        if method_name.changed {
            writeln!(self.writer, "{indent}// Metadata name: {}", method.name)?;
        }
        let mut declaration = if method.name == ".cctor" {
            "static ".to_owned()
        } else {
            method_modifiers(method)
        };
        if !constructor {
            declaration.push_str(&self.format_type(&signature.return_type));
            declaration.push(' ');
        }
        declaration.push_str(&method_name.rendered);
        declaration.push_str(&self.method_generic_declaration(&signature));
        declaration.push('(');
        declaration.push_str(
            &signature
                .parameters
                .iter()
                .map(|parameter| self.format_parameter(parameter))
                .collect::<Vec<_>>()
                .join(", "),
        );
        declaration.push(')');
        for constraint in self.generic_constraints(method.generic_container_index, 0) {
            declaration.push(' ');
            declaration.push_str(&constraint);
        }
        declaration.push(';');
        writeln!(self.writer, "{indent}{declaration}")?;
        Ok(())
    }

    fn type_bases(
        &mut self,
        ty: &TypeDefinition,
        enum_underlying: Option<&TypeRef>,
    ) -> Vec<String> {
        if ty.kind() == TypeKind::Enum {
            return enum_underlying
                .filter(|underlying| is_enum_underlying_type(underlying))
                .map(|underlying| vec![self.format_type(underlying)])
                .unwrap_or_default();
        }
        let mut bases = Vec::new();
        if let Some(parent) = ty.parent {
            let parent = self.resolve_type(parent);
            let skip = matches!(&parent, TypeRef::Object)
                || matches!(
                    &parent,
                    TypeRef::Type(id)
                        if matches!(
                            (self.context.metadata.types[id.0].namespace.as_str(), self.context.metadata.types[id.0].name.as_str()),
                            ("System", "Object" | "ValueType" | "Enum")
                        )
                );
            if !skip {
                bases.push(self.format_type(&parent));
            }
        }
        for interface in &ty.interfaces {
            let interface = self.resolve_type(*interface);
            bases.push(self.format_type(&interface));
        }
        bases
    }

    fn enum_underlying_type(&self, ty: &TypeDefinition) -> Option<TypeRef> {
        if ty.kind() != TypeKind::Enum {
            return None;
        }
        ty.fields
            .iter()
            .map(|field| &self.context.metadata.fields[field.0])
            .find(|field| field.name == "value__")
            .map(|field| {
                self.context
                    .types
                    .field_signature(field)
                    .map(|signature| signature.ty)
                    .unwrap_or(TypeRef::Unknown {
                        type_index: Some(field.field_type.0),
                        raw_type: None,
                    })
            })
    }

    fn generic_declaration(&self, ty: &TypeDefinition, skip: usize) -> String {
        let Some(container) = ty
            .generic_container_index
            .and_then(|index| self.context.metadata.generic_containers.get(index))
        else {
            return String::new();
        };
        let parameters = container
            .parameters
            .iter()
            .skip(skip)
            .map(|parameter| {
                identifier(&self.context.metadata.generic_parameters[parameter.0].name)
                    .rendered
                    .into_owned()
            })
            .collect::<Vec<_>>()
            .join(", ");
        if parameters.is_empty() {
            String::new()
        } else {
            format!("<{parameters}>")
        }
    }

    fn inherited_generic_parameter_count(&self, ty: &TypeDefinition) -> usize {
        ty.nested_in
            .and_then(|parent| self.context.metadata.types[parent.0].generic_container_index)
            .and_then(|container| self.context.metadata.generic_containers.get(container))
            .map_or(0, |container| container.parameters.len())
    }

    fn method_generic_declaration(&self, signature: &MethodSignature) -> String {
        if signature.generic_parameters.is_empty() {
            return String::new();
        }
        let parameters = signature
            .generic_parameters
            .iter()
            .map(|parameter| {
                identifier(&self.context.metadata.generic_parameters[parameter.0].name)
                    .rendered
                    .into_owned()
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{parameters}>")
    }

    fn generic_constraints(&mut self, container_index: Option<usize>, skip: usize) -> Vec<String> {
        const REFERENCE_TYPE: u16 = 0x4;
        const VALUE_TYPE: u16 = 0x8;
        const DEFAULT_CONSTRUCTOR: u16 = 0x10;
        let Some(container) =
            container_index.and_then(|index| self.context.metadata.generic_containers.get(index))
        else {
            return Vec::new();
        };
        container
            .parameters
            .iter()
            .skip(skip)
            .filter_map(|parameter| {
                let parameter = &self.context.metadata.generic_parameters[parameter.0];
                let mut constraints = Vec::new();
                if parameter.flags & REFERENCE_TYPE != 0 {
                    constraints.push("class".to_owned());
                }
                if parameter.flags & VALUE_TYPE != 0 {
                    constraints.push("struct".to_owned());
                }
                for constraint in &parameter.constraints {
                    let constraint = self.resolve_type(*constraint);
                    let is_value_type = matches!(
                        constraint,
                        TypeRef::Type(id)
                            if self.context.metadata.types[id.0].namespace == "System"
                                && self.context.metadata.types[id.0].name == "ValueType"
                    );
                    if !is_value_type {
                        constraints.push(self.format_type(&constraint));
                    }
                }
                if parameter.flags & DEFAULT_CONSTRUCTOR != 0 && parameter.flags & VALUE_TYPE == 0 {
                    constraints.push("new()".to_owned());
                }
                (!constraints.is_empty()).then(|| {
                    format!(
                        "where {} : {}",
                        identifier(&parameter.name).rendered,
                        constraints.join(", ")
                    )
                })
            })
            .collect()
    }

    fn format_parameter(&mut self, parameter: &ResolvedParameter) -> String {
        let name = if parameter.name.is_empty() {
            format!("param_{}", parameter.position)
        } else {
            identifier(&parameter.name).rendered.into_owned()
        };
        format!("{} {name}", self.format_type(&parameter.ty))
    }

    fn field_signature(&mut self, field: &il2cpp_core::model::Field) -> FieldSignature {
        self.context
            .types
            .field_signature(field)
            .unwrap_or(FieldSignature {
                ty: TypeRef::Unknown {
                    type_index: Some(field.field_type.0),
                    raw_type: None,
                },
                attributes: 0,
                offset: None,
            })
    }

    fn method_signature(&mut self, method: &Method) -> MethodSignature {
        self.context
            .types
            .method_signature(method)
            .unwrap_or_else(|_| MethodSignature {
                return_type: TypeRef::Unknown {
                    type_index: Some(method.return_type.0),
                    raw_type: None,
                },
                return_attributes: 0,
                parameters: method
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(position, parameter)| {
                        let parameter = &self.context.metadata.parameters[parameter.0];
                        ResolvedParameter {
                            id: parameter.id,
                            name: parameter.name.clone(),
                            ty: TypeRef::Unknown {
                                type_index: Some(parameter.parameter_type.0),
                                raw_type: None,
                            },
                            position,
                            attributes: 0,
                        }
                    })
                    .collect(),
                generic_parameters: Vec::new(),
            })
    }

    fn resolve_type(&mut self, index: TypeIndex) -> TypeRef {
        self.context
            .types
            .resolve(index)
            .unwrap_or(TypeRef::Unknown {
                type_index: Some(index.0),
                raw_type: None,
            })
    }

    fn format_type(&mut self, ty: &TypeRef) -> String {
        self.summary.unresolved_type_references += unresolved_count(ty);
        self.type_renderer.format(ty)
    }
}

fn unresolved_count(ty: &TypeRef) -> usize {
    match ty {
        TypeRef::Unknown { .. } => 1,
        TypeRef::Array { element, .. } | TypeRef::Pointer(element) | TypeRef::ByRef(element) => {
            unresolved_count(element)
        }
        TypeRef::GenericInstance { base, arguments } => {
            unresolved_count(base) + arguments.iter().map(unresolved_count).sum::<usize>()
        }
        _ => 0,
    }
}

fn is_enum_underlying_type(ty: &TypeRef) -> bool {
    matches!(
        ty,
        TypeRef::I8
            | TypeRef::U8
            | TypeRef::I16
            | TypeRef::U16
            | TypeRef::I32
            | TypeRef::U32
            | TypeRef::I64
            | TypeRef::U64
    )
}

fn indent(depth: usize) -> String {
    "    ".repeat(depth)
}

fn type_modifiers(ty: &TypeDefinition) -> String {
    const VISIBILITY_MASK: u32 = 0x7;
    const PUBLIC: u32 = 0x1;
    const NESTED_PUBLIC: u32 = 0x2;
    const NESTED_PRIVATE: u32 = 0x3;
    const NESTED_FAMILY: u32 = 0x4;
    const NESTED_ASSEMBLY: u32 = 0x5;
    const NESTED_FAMILY_AND_ASSEMBLY: u32 = 0x6;
    const NESTED_FAMILY_OR_ASSEMBLY: u32 = 0x7;
    const ABSTRACT: u32 = 0x80;
    const SEALED: u32 = 0x100;

    let mut result = match ty.flags & VISIBILITY_MASK {
        PUBLIC | NESTED_PUBLIC => "public ".to_owned(),
        NESTED_PRIVATE => "private ".to_owned(),
        NESTED_FAMILY => "protected ".to_owned(),
        NESTED_ASSEMBLY => "internal ".to_owned(),
        NESTED_FAMILY_AND_ASSEMBLY => "private protected ".to_owned(),
        NESTED_FAMILY_OR_ASSEMBLY => "protected internal ".to_owned(),
        _ => "internal ".to_owned(),
    };
    if ty.kind() == TypeKind::Class && ty.flags & ABSTRACT != 0 && ty.flags & SEALED != 0 {
        result.push_str("static ");
    } else {
        if ty.flags & ABSTRACT != 0 && ty.kind() == TypeKind::Class {
            result.push_str("abstract ");
        }
        if ty.flags & SEALED != 0 && ty.kind() == TypeKind::Class {
            result.push_str("sealed ");
        }
    }
    result
}

fn member_access(flags: u16) -> &'static str {
    match flags & 0x7 {
        1 => "private ",
        2 => "private protected ",
        3 => "internal ",
        4 => "protected ",
        5 => "protected internal ",
        6 => "public ",
        _ => "",
    }
}

fn access_rank(flags: u16) -> u8 {
    match flags & 0x7 {
        6 => 6,
        5 => 5,
        4 => 4,
        3 => 3,
        2 => 2,
        1 => 1,
        _ => 0,
    }
}

fn property_accessor(kind: &str, accessor_flags: u16, property_flags: u16) -> String {
    if accessor_flags & 0x7 == property_flags & 0x7 {
        format!("{kind};")
    } else {
        format!("{}{kind};", member_access(accessor_flags))
    }
}

fn property_modifiers(flags: u16) -> String {
    let mut result = member_access(flags).to_owned();
    result.push_str(&virtual_modifiers(flags));
    result
}

fn field_modifiers(flags: u16) -> String {
    const STATIC: u16 = 0x10;
    const INIT_ONLY: u16 = 0x20;
    const LITERAL: u16 = 0x40;
    let mut result = member_access(flags).to_owned();
    if flags & LITERAL != 0 {
        result.push_str("const ");
    } else {
        if flags & STATIC != 0 {
            result.push_str("static ");
        }
        if flags & INIT_ONLY != 0 {
            result.push_str("readonly ");
        }
    }
    result
}

fn method_modifiers(method: &Method) -> String {
    const PINVOKE: u16 = 0x2000;
    const INTERNAL_CALL: u16 = 0x1000;
    let mut result = member_access(method.flags).to_owned();
    result.push_str(&virtual_modifiers(method.flags));
    if method.flags & PINVOKE != 0 || method.implementation_flags & INTERNAL_CALL != 0 {
        result.push_str("extern ");
    }
    result
}

fn virtual_modifiers(flags: u16) -> String {
    const STATIC: u16 = 0x10;
    const FINAL: u16 = 0x20;
    const VIRTUAL: u16 = 0x40;
    const NEW_SLOT: u16 = 0x100;
    const ABSTRACT: u16 = 0x400;

    if flags & STATIC != 0 {
        "static ".to_owned()
    } else if flags & ABSTRACT != 0 {
        "abstract ".to_owned()
    } else if flags & VIRTUAL != 0 && flags & NEW_SLOT == 0 {
        if flags & FINAL != 0 {
            "sealed override ".to_owned()
        } else {
            "override ".to_owned()
        }
    } else if flags & VIRTUAL != 0 && flags & FINAL == 0 {
        "virtual ".to_owned()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use il2cpp_core::analysis::TypeResolver;
    use il2cpp_core::metadata::{METADATA_SANITY, Metadata};

    use super::*;

    #[test]
    fn renders_small_normalized_model() {
        let metadata = Metadata::from_bytes(&fixture()).unwrap();
        let resolved = [TypeRef::I32, TypeRef::Void];
        let attributes = [6, 0];
        let resolver =
            TypeResolver::from_resolved(&metadata, &resolved, Some(&attributes)).unwrap();
        let context = ExportContext {
            metadata: &metadata,
            types: &resolver,
            native_methods: None,
        };
        let options = DumpCsOptions {
            include_addresses: false,
            include_file_offsets: false,
            include_tokens: false,
            include_indices: false,
            include_field_offsets: false,
            fully_qualified_types: false,
        };
        let mut output = Vec::new();
        DumpCsExporter::new(options)
            .export(&context, &mut output)
            .unwrap();
        let output = String::from_utf8(output).unwrap();

        assert!(output.contains(
            "public class Player\n{\n    // Fields\n\n    public int health;\n    // Methods\n\n    public void Update();\n}"
        ));
    }

    #[test]
    fn reconstructs_static_properties_and_virtual_slots() {
        assert_eq!(property_modifiers(0x16), "public static ");
        assert_eq!(property_accessor("set", 0x1, 0x6), "private set;");
        assert_eq!(virtual_modifiers(0x146), "virtual ");
        assert_eq!(virtual_modifiers(0x46), "override ");
        assert_eq!(virtual_modifiers(0x66), "sealed override ");
        assert_eq!(virtual_modifiers(0x166), "");
    }

    fn fixture() -> Vec<u8> {
        const STRINGS: usize = 2;
        const METHODS: usize = 5;
        const FIELDS: usize = 11;
        const TYPES: usize = 19;
        const IMAGES: usize = 20;
        const ASSEMBLIES: usize = 21;
        let mut data = vec![0; 256];
        write_u32(&mut data, 0, METADATA_SANITY);
        write_u32(&mut data, 4, 31);
        add_table(
            &mut data,
            STRINGS,
            b"\0Test.dll\0Test\0Game\0Player\0health\0Update\0",
        );

        let mut method = vec![0; 36];
        write_u32(&mut method, 0, 34);
        write_i32(&mut method, 4, 0);
        write_i32(&mut method, 8, 1);
        write_i32(&mut method, 16, -1);
        write_i32(&mut method, 20, -1);
        write_u32(&mut method, 24, 0x0600_0001);
        write_u16(&mut method, 28, 6);
        add_table(&mut data, METHODS, &method);

        let mut field = vec![0; 12];
        write_u32(&mut field, 0, 27);
        write_i32(&mut field, 4, 0);
        write_u32(&mut field, 8, 0x0400_0001);
        add_table(&mut data, FIELDS, &field);

        let mut ty = vec![0; 88];
        write_u32(&mut ty, 0, 20);
        write_u32(&mut ty, 4, 15);
        write_i32(&mut ty, 8, 0);
        write_i32(&mut ty, 12, -1);
        write_i32(&mut ty, 16, -1);
        write_i32(&mut ty, 20, -1);
        write_i32(&mut ty, 24, -1);
        write_u32(&mut ty, 28, 1);
        write_i32(&mut ty, 32, 0);
        write_i32(&mut ty, 36, 0);
        write_u16(&mut ty, 64, 1);
        write_u16(&mut ty, 68, 1);
        write_u32(&mut ty, 84, 0x0200_0001);
        add_table(&mut data, TYPES, &ty);

        let mut image = vec![0; 40];
        write_u32(&mut image, 0, 1);
        write_i32(&mut image, 4, 0);
        write_i32(&mut image, 8, 0);
        write_u32(&mut image, 12, 1);
        add_table(&mut data, IMAGES, &image);

        let mut assembly = vec![0; 64];
        write_i32(&mut assembly, 0, 0);
        write_u32(&mut assembly, 16, 10);
        add_table(&mut data, ASSEMBLIES, &assembly);
        data
    }

    fn add_table(data: &mut Vec<u8>, pair: usize, bytes: &[u8]) {
        let offset = data.len() as u32;
        write_u32(data, 8 + pair * 8, offset);
        write_u32(data, 12 + pair * 8, bytes.len() as u32);
        data.extend_from_slice(bytes);
    }

    fn write_u16(data: &mut [u8], offset: usize, value: u16) {
        data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(data: &mut [u8], offset: usize, value: u32) {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_i32(data: &mut [u8], offset: usize, value: i32) {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
