use crate::metadata::{MetadataHeader, MetadataReader, MetadataTable};
use crate::{Error, Result};

pub(crate) const VERSION: u32 = 31;
pub(crate) const HEADER_SIZE: usize = 256;
pub(crate) const IMAGE_SIZE: usize = 40;
pub(crate) const ASSEMBLY_SIZE: usize = 64;
pub(crate) const TYPE_DEFINITION_SIZE: usize = 88;
pub(crate) const FIELD_SIZE: usize = 12;
pub(crate) const METHOD_SIZE: usize = 36;
pub(crate) const PARAMETER_SIZE: usize = 12;
pub(crate) const PROPERTY_SIZE: usize = 20;
pub(crate) const GENERIC_PARAMETER_SIZE: usize = 16;
pub(crate) const GENERIC_CONTAINER_SIZE: usize = 16;

#[derive(Debug)]
pub(crate) struct RawImage {
    pub name_index: u32,
    pub assembly_index: i32,
    pub type_start: i32,
    pub type_count: u32,
}

#[derive(Debug)]
pub(crate) struct RawAssembly {
    pub image_index: i32,
    pub name_index: u32,
}

#[derive(Debug)]
pub(crate) struct RawTypeDefinition {
    pub name_index: u32,
    pub namespace_index: u32,
    pub byval_type_index: i32,
    pub declaring_type_index: i32,
    pub parent_index: i32,
    pub element_type_index: i32,
    pub generic_container_index: i32,
    pub flags: u32,
    pub field_start: i32,
    pub method_start: i32,
    pub property_start: i32,
    pub nested_types_start: i32,
    pub interfaces_start: i32,
    pub field_count: u16,
    pub method_count: u16,
    pub property_count: u16,
    pub nested_type_count: u16,
    pub interfaces_count: u16,
    pub bitfield: u32,
    pub token: u32,
}

#[derive(Debug)]
pub(crate) struct RawField {
    pub name_index: u32,
    pub type_index: i32,
    pub token: u32,
}

#[derive(Debug)]
pub(crate) struct RawMethod {
    pub name_index: u32,
    pub declaring_type: i32,
    pub return_type: i32,
    pub return_parameter_token: u32,
    pub parameter_start: i32,
    pub generic_container_index: i32,
    pub token: u32,
    pub flags: u16,
    pub iflags: u16,
    pub slot: u16,
    pub parameter_count: u16,
}

#[derive(Debug)]
pub(crate) struct RawParameter {
    pub name_index: u32,
    pub token: u32,
    pub type_index: i32,
}

#[derive(Debug)]
pub(crate) struct RawProperty {
    pub name_index: u32,
    pub getter: i32,
    pub setter: i32,
    pub attributes: u32,
    pub token: u32,
}

#[derive(Debug)]
pub(crate) struct RawGenericContainer {
    pub owner_index: i32,
    pub type_argument_count: i32,
    pub is_method: i32,
    pub generic_parameter_start: i32,
}

#[derive(Debug)]
pub(crate) struct RawGenericParameter {
    pub owner_index: i32,
    pub name_index: u32,
    pub constraints_start: i16,
    pub constraints_count: i16,
    pub position: u16,
    pub flags: u16,
}

#[derive(Debug)]
pub(crate) struct RawRecords {
    pub images: Vec<RawImage>,
    pub assemblies: Vec<RawAssembly>,
    pub types: Vec<RawTypeDefinition>,
    pub fields: Vec<RawField>,
    pub methods: Vec<RawMethod>,
    pub parameters: Vec<RawParameter>,
    pub properties: Vec<RawProperty>,
    pub generic_containers: Vec<RawGenericContainer>,
    pub generic_parameters: Vec<RawGenericParameter>,
    pub generic_parameter_constraints: Vec<i32>,
    pub nested_types: Vec<i32>,
    pub interfaces: Vec<i32>,
}

pub(crate) fn parse_header(
    reader: &MetadataReader<'_>,
    sanity: u32,
    version: u32,
) -> Result<MetadataHeader> {
    reader.read_bytes(0, HEADER_SIZE)?;
    let mut next = 8;
    let mut table = || -> Result<MetadataTable> {
        let result = MetadataTable::from_raw(reader.read_u32(next)?, reader.read_u32(next + 4)?);
        next += 8;
        Ok(result)
    };
    let header = MetadataHeader {
        sanity,
        version,
        string_literals: table()?,
        string_literal_data: table()?,
        strings: table()?,
        events: table()?,
        properties: table()?,
        methods: table()?,
        parameter_default_values: table()?,
        field_default_values: table()?,
        default_value_data: table()?,
        field_marshaled_sizes: table()?,
        parameters: table()?,
        fields: table()?,
        generic_parameters: table()?,
        generic_parameter_constraints: table()?,
        generic_containers: table()?,
        nested_types: table()?,
        interfaces: table()?,
        vtable_methods: table()?,
        interface_offsets: table()?,
        type_definitions: table()?,
        images: table()?,
        assemblies: table()?,
        field_refs: table()?,
        referenced_assemblies: table()?,
        attribute_data: table()?,
        attribute_data_ranges: table()?,
        unresolved_virtual_call_parameter_types: table()?,
        unresolved_virtual_call_parameter_ranges: table()?,
        windows_runtime_type_names: table()?,
        windows_runtime_strings: table()?,
        exported_type_definitions: table()?,
    };
    for (_, metadata_table) in header.tables() {
        metadata_table.validate(reader.len())?;
    }
    let mut nonempty_tables: Vec<_> = header
        .tables()
        .into_iter()
        .filter(|(_, metadata_table)| metadata_table.byte_count > 0)
        .collect();
    nonempty_tables.sort_by_key(|(_, metadata_table)| metadata_table.offset);
    let mut previous_end = HEADER_SIZE;
    for (name, metadata_table) in nonempty_tables {
        if metadata_table.offset < HEADER_SIZE || metadata_table.offset < previous_end {
            return Err(Error::InvalidMetadataTable(name));
        }
        previous_end = metadata_table.validate(reader.len())?;
    }
    Ok(header)
}

pub(crate) fn parse_records(
    reader: &MetadataReader<'_>,
    header: &MetadataHeader,
) -> Result<RawRecords> {
    Ok(RawRecords {
        images: parse_images(reader, header)?,
        assemblies: parse_assemblies(reader, header)?,
        types: parse_types(reader, header)?,
        fields: parse_fields(reader, header)?,
        methods: parse_methods(reader, header)?,
        parameters: parse_parameters(reader, header)?,
        properties: parse_properties(reader, header)?,
        generic_containers: parse_generic_containers(reader, header)?,
        generic_parameters: parse_generic_parameters(reader, header)?,
        generic_parameter_constraints: parse_i32_table(
            reader,
            header.generic_parameter_constraints,
        )?,
        nested_types: parse_i32_table(reader, header.nested_types)?,
        interfaces: parse_i32_table(reader, header.interfaces)?,
    })
}

fn offsets(table: MetadataTable, size: usize, file_size: usize) -> Result<Vec<usize>> {
    let count = table.entry_count(size, file_size)?;
    (0..count)
        .map(|index| {
            index
                .checked_mul(size)
                .and_then(|relative| table.offset.checked_add(relative))
                .ok_or(Error::InvalidMetadata)
        })
        .collect()
}

fn parse_images(reader: &MetadataReader<'_>, header: &MetadataHeader) -> Result<Vec<RawImage>> {
    offsets(header.images, IMAGE_SIZE, reader.len())?
        .into_iter()
        .map(|offset| {
            Ok(RawImage {
                name_index: reader.read_u32(offset)?,
                assembly_index: reader.read_i32(offset + 4)?,
                type_start: reader.read_i32(offset + 8)?,
                type_count: reader.read_u32(offset + 12)?,
            })
        })
        .collect()
}

fn parse_assemblies(
    reader: &MetadataReader<'_>,
    header: &MetadataHeader,
) -> Result<Vec<RawAssembly>> {
    offsets(header.assemblies, ASSEMBLY_SIZE, reader.len())?
        .into_iter()
        .map(|offset| {
            Ok(RawAssembly {
                image_index: reader.read_i32(offset)?,
                name_index: reader.read_u32(offset + 16)?,
            })
        })
        .collect()
}

fn parse_types(
    reader: &MetadataReader<'_>,
    header: &MetadataHeader,
) -> Result<Vec<RawTypeDefinition>> {
    offsets(header.type_definitions, TYPE_DEFINITION_SIZE, reader.len())?
        .into_iter()
        .map(|offset| {
            Ok(RawTypeDefinition {
                name_index: reader.read_u32(offset)?,
                namespace_index: reader.read_u32(offset + 4)?,
                byval_type_index: reader.read_i32(offset + 8)?,
                declaring_type_index: reader.read_i32(offset + 12)?,
                parent_index: reader.read_i32(offset + 16)?,
                element_type_index: reader.read_i32(offset + 20)?,
                generic_container_index: reader.read_i32(offset + 24)?,
                flags: reader.read_u32(offset + 28)?,
                field_start: reader.read_i32(offset + 32)?,
                method_start: reader.read_i32(offset + 36)?,
                property_start: reader.read_i32(offset + 44)?,
                nested_types_start: reader.read_i32(offset + 48)?,
                interfaces_start: reader.read_i32(offset + 52)?,
                method_count: reader.read_u16(offset + 64)?,
                property_count: reader.read_u16(offset + 66)?,
                field_count: reader.read_u16(offset + 68)?,
                nested_type_count: reader.read_u16(offset + 72)?,
                interfaces_count: reader.read_u16(offset + 76)?,
                bitfield: reader.read_u32(offset + 80)?,
                token: reader.read_u32(offset + 84)?,
            })
        })
        .collect()
}

fn parse_fields(reader: &MetadataReader<'_>, header: &MetadataHeader) -> Result<Vec<RawField>> {
    offsets(header.fields, FIELD_SIZE, reader.len())?
        .into_iter()
        .map(|offset| {
            Ok(RawField {
                name_index: reader.read_u32(offset)?,
                type_index: reader.read_i32(offset + 4)?,
                token: reader.read_u32(offset + 8)?,
            })
        })
        .collect()
}

fn parse_methods(reader: &MetadataReader<'_>, header: &MetadataHeader) -> Result<Vec<RawMethod>> {
    offsets(header.methods, METHOD_SIZE, reader.len())?
        .into_iter()
        .map(|offset| {
            Ok(RawMethod {
                name_index: reader.read_u32(offset)?,
                declaring_type: reader.read_i32(offset + 4)?,
                return_type: reader.read_i32(offset + 8)?,
                return_parameter_token: reader.read_u32(offset + 12)?,
                parameter_start: reader.read_i32(offset + 16)?,
                generic_container_index: reader.read_i32(offset + 20)?,
                token: reader.read_u32(offset + 24)?,
                flags: reader.read_u16(offset + 28)?,
                iflags: reader.read_u16(offset + 30)?,
                slot: reader.read_u16(offset + 32)?,
                parameter_count: reader.read_u16(offset + 34)?,
            })
        })
        .collect()
}

fn parse_parameters(
    reader: &MetadataReader<'_>,
    header: &MetadataHeader,
) -> Result<Vec<RawParameter>> {
    offsets(header.parameters, PARAMETER_SIZE, reader.len())?
        .into_iter()
        .map(|offset| {
            Ok(RawParameter {
                name_index: reader.read_u32(offset)?,
                token: reader.read_u32(offset + 4)?,
                type_index: reader.read_i32(offset + 8)?,
            })
        })
        .collect()
}

fn parse_properties(
    reader: &MetadataReader<'_>,
    header: &MetadataHeader,
) -> Result<Vec<RawProperty>> {
    offsets(header.properties, PROPERTY_SIZE, reader.len())?
        .into_iter()
        .map(|offset| {
            Ok(RawProperty {
                name_index: reader.read_u32(offset)?,
                getter: reader.read_i32(offset + 4)?,
                setter: reader.read_i32(offset + 8)?,
                attributes: reader.read_u32(offset + 12)?,
                token: reader.read_u32(offset + 16)?,
            })
        })
        .collect()
}

fn parse_generic_containers(
    reader: &MetadataReader<'_>,
    header: &MetadataHeader,
) -> Result<Vec<RawGenericContainer>> {
    offsets(
        header.generic_containers,
        GENERIC_CONTAINER_SIZE,
        reader.len(),
    )?
    .into_iter()
    .map(|offset| {
        Ok(RawGenericContainer {
            owner_index: reader.read_i32(offset)?,
            type_argument_count: reader.read_i32(offset + 4)?,
            is_method: reader.read_i32(offset + 8)?,
            generic_parameter_start: reader.read_i32(offset + 12)?,
        })
    })
    .collect()
}

fn parse_generic_parameters(
    reader: &MetadataReader<'_>,
    header: &MetadataHeader,
) -> Result<Vec<RawGenericParameter>> {
    offsets(
        header.generic_parameters,
        GENERIC_PARAMETER_SIZE,
        reader.len(),
    )?
    .into_iter()
    .map(|offset| {
        Ok(RawGenericParameter {
            owner_index: reader.read_i32(offset)?,
            name_index: reader.read_u32(offset + 4)?,
            constraints_start: reader.read_i16(offset + 8)?,
            constraints_count: reader.read_i16(offset + 10)?,
            position: reader.read_u16(offset + 12)?,
            flags: reader.read_u16(offset + 14)?,
        })
    })
    .collect()
}

fn parse_i32_table(reader: &MetadataReader<'_>, table: MetadataTable) -> Result<Vec<i32>> {
    offsets(table, 4, reader.len())?
        .into_iter()
        .map(|offset| reader.read_i32(offset))
        .collect()
}
