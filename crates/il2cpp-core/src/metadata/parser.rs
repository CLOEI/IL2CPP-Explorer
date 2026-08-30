use crate::metadata::versions;
use crate::metadata::{Metadata, MetadataReader, string_at};
use crate::model::{
    Assembly, AssemblyId, Field, FieldId, Image, ImageId, Method, MethodId, Parameter, ParameterId,
    TypeDefinition, TypeId, TypeIndex,
};
use crate::{Error, Result};

pub(super) fn parse(data: Vec<u8>) -> Result<Metadata> {
    let reader = MetadataReader::new(&data);
    let header = reader.header()?;
    let version = versions::resolve(header.version)?;
    let raw = versions::parse_records(&reader, &header)?;

    let type_owners = assign_ranges(
        raw.images
            .iter()
            .map(|image| (image.type_start, image.type_count as usize)),
        raw.types.len(),
        "images/type definitions",
    )?;
    let field_owners = assign_ranges(
        raw.types
            .iter()
            .map(|ty| (ty.field_start, ty.field_count as usize)),
        raw.fields.len(),
        "type fields",
    )?;
    let method_owners = assign_ranges(
        raw.types
            .iter()
            .map(|ty| (ty.method_start, ty.method_count as usize)),
        raw.methods.len(),
        "type methods",
    )?;
    let parameter_owners = assign_ranges(
        raw.methods
            .iter()
            .map(|method| (method.parameter_start, method.parameter_count as usize)),
        raw.parameters.len(),
        "method parameters",
    )?;

    let assemblies = raw
        .assemblies
        .iter()
        .enumerate()
        .map(|(index, assembly)| {
            let image = checked_index(assembly.image_index, raw.images.len(), "assembly image")?;
            if raw.images[image].assembly_index != index as i32 {
                return Err(Error::InvalidMetadataTable("assembly/image relationship"));
            }
            Ok(Assembly {
                id: AssemblyId(index),
                name: string_at(&data, &header, assembly.name_index)?.to_owned(),
                image: ImageId(image),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let images = raw
        .images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            let assembly =
                checked_index(image.assembly_index, raw.assemblies.len(), "image assembly")?;
            let range = checked_range(
                image.type_start,
                image.type_count as usize,
                raw.types.len(),
                "image types",
            )?;
            Ok(Image {
                id: ImageId(index),
                assembly: AssemblyId(assembly),
                name: string_at(&data, &header, image.name_index)?.to_owned(),
                types: range.map(TypeId).collect(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let types = raw
        .types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let fields = checked_range(
                ty.field_start,
                ty.field_count as usize,
                raw.fields.len(),
                "type fields",
            )?
            .map(FieldId)
            .collect();
            let methods = checked_range(
                ty.method_start,
                ty.method_count as usize,
                raw.methods.len(),
                "type methods",
            )?
            .map(MethodId)
            .collect();
            Ok(TypeDefinition {
                id: TypeId(index),
                image: ImageId(type_owners[index]),
                namespace: string_at(&data, &header, ty.namespace_index)?.to_owned(),
                name: string_at(&data, &header, ty.name_index)?.to_owned(),
                declaring_type: optional_type_index(ty.declaring_type_index)?,
                parent: optional_type_index(ty.parent_index)?,
                generic_container_index: optional_checked_index(
                    ty.generic_container_index,
                    raw.generic_container_count,
                    "type generic container",
                )?,
                methods,
                fields,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let fields = raw
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            Ok(Field {
                id: FieldId(index),
                declaring_type: TypeId(field_owners[index]),
                name: string_at(&data, &header, field.name_index)?.to_owned(),
                field_type: required_type_index(field.type_index, "field type")?,
                token: field.token,
                offset: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let parameters = raw
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            Ok(Parameter {
                id: ParameterId(index),
                declaring_method: MethodId(parameter_owners[index]),
                name: string_at(&data, &header, parameter.name_index)?.to_owned(),
                parameter_type: required_type_index(parameter.type_index, "parameter type")?,
                token: parameter.token,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let methods = raw
        .methods
        .iter()
        .enumerate()
        .map(|(index, method)| {
            let declaring_type = checked_index(
                method.declaring_type,
                raw.types.len(),
                "method declaring type",
            )?;
            if method_owners[index] != declaring_type {
                return Err(Error::InvalidMetadataTable("method/type relationship"));
            }
            let parameters = checked_range(
                method.parameter_start,
                method.parameter_count as usize,
                raw.parameters.len(),
                "method parameters",
            )?
            .map(ParameterId)
            .collect();
            Ok(Method {
                id: MethodId(index),
                declaring_type: TypeId(declaring_type),
                name: string_at(&data, &header, method.name_index)?.to_owned(),
                return_type: required_type_index(method.return_type, "method return type")?,
                return_parameter_token: method.return_parameter_token,
                parameters,
                generic_container_index: optional_checked_index(
                    method.generic_container_index,
                    raw.generic_container_count,
                    "method generic container",
                )?,
                token: method.token,
                flags: method.flags,
                implementation_flags: method.iflags,
                slot: method.slot,
                relative_address: None,
                virtual_address: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Metadata {
        version,
        assemblies,
        images,
        types,
        methods,
        fields,
        parameters,
        header,
        data,
    })
}

fn assign_ranges(
    ranges: impl Iterator<Item = (i32, usize)>,
    item_count: usize,
    name: &'static str,
) -> Result<Vec<usize>> {
    let mut owners = vec![None; item_count];
    for (owner, (start, count)) in ranges.enumerate() {
        for index in checked_range(start, count, item_count, name)? {
            if owners[index].replace(owner).is_some() {
                return Err(Error::InvalidMetadataTable(name));
            }
        }
    }
    owners
        .into_iter()
        .map(|owner| owner.ok_or(Error::InvalidMetadataTable(name)))
        .collect()
}

fn checked_range(
    start: i32,
    count: usize,
    total: usize,
    name: &'static str,
) -> Result<std::ops::Range<usize>> {
    if count == 0 && start == -1 {
        return Ok(0..0);
    }
    let start = usize::try_from(start).map_err(|_| Error::InvalidMetadataTable(name))?;
    let end = start
        .checked_add(count)
        .ok_or(Error::InvalidMetadataTable(name))?;
    if end > total {
        return Err(Error::InvalidMetadataTable(name));
    }
    Ok(start..end)
}

fn checked_index(index: i32, total: usize, name: &'static str) -> Result<usize> {
    let index = usize::try_from(index).map_err(|_| Error::InvalidMetadataTable(name))?;
    if index >= total {
        return Err(Error::InvalidMetadataTable(name));
    }
    Ok(index)
}

fn required_type_index(index: i32, name: &'static str) -> Result<TypeIndex> {
    Ok(TypeIndex(
        usize::try_from(index).map_err(|_| Error::InvalidMetadataTable(name))?,
    ))
}

fn optional_type_index(index: i32) -> Result<Option<TypeIndex>> {
    optional_index(index).map(|index| index.map(TypeIndex))
}

fn optional_checked_index(index: i32, total: usize, name: &'static str) -> Result<Option<usize>> {
    match optional_index(index)? {
        Some(index) if index < total => Ok(Some(index)),
        Some(_) => Err(Error::InvalidMetadataTable(name)),
        None => Ok(None),
    }
}

fn optional_index(index: i32) -> Result<Option<usize>> {
    if index == -1 {
        return Ok(None);
    }
    usize::try_from(index)
        .map(Some)
        .map_err(|_| Error::InvalidMetadata)
}
