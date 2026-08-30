use crate::metadata::versions;
use crate::metadata::{Metadata, MetadataReader, string_at};
use crate::model::{
    Assembly, AssemblyId, Field, FieldId, GenericContainer, GenericContainerId, GenericOwner,
    GenericParameter, GenericParameterId, Image, ImageId, Method, MethodId, Parameter, ParameterId,
    Property, PropertyId, TypeDefinition, TypeId, TypeIndex,
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
    let property_owners = assign_ranges(
        raw.types
            .iter()
            .map(|ty| (ty.property_start, ty.property_count as usize)),
        raw.properties.len(),
        "type properties",
    )?;
    let mut nested_types = vec![Vec::new(); raw.types.len()];
    let mut nested_in = vec![None; raw.types.len()];
    for (owner, ty) in raw.types.iter().enumerate() {
        for entry in checked_range(
            ty.nested_types_start,
            ty.nested_type_count as usize,
            raw.nested_types.len(),
            "type nested types",
        )? {
            let nested = checked_index(
                raw.nested_types[entry],
                raw.types.len(),
                "nested type definition",
            )?;
            if nested == owner || nested_in[nested].replace(TypeId(owner)).is_some() {
                return Err(Error::InvalidMetadataTable("nested type ownership"));
            }
            nested_types[owner].push(TypeId(nested));
        }
    }
    for index in 0..raw.types.len() {
        let mut current = Some(TypeId(index));
        for _ in 0..=raw.types.len() {
            let Some(type_id) = current else {
                break;
            };
            current = nested_in[type_id.0];
        }
        if current.is_some() {
            return Err(Error::InvalidMetadataTable("nested type ownership"));
        }
    }

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
            let properties = checked_range(
                ty.property_start,
                ty.property_count as usize,
                raw.properties.len(),
                "type properties",
            )?
            .map(PropertyId)
            .collect();
            let interfaces = checked_range(
                ty.interfaces_start,
                ty.interfaces_count as usize,
                raw.interfaces.len(),
                "type interfaces",
            )?
            .map(|index| required_type_index(raw.interfaces[index], "interface type"))
            .collect::<Result<Vec<_>>>()?;
            Ok(TypeDefinition {
                id: TypeId(index),
                image: ImageId(type_owners[index]),
                namespace: string_at(&data, &header, ty.namespace_index)?.to_owned(),
                name: string_at(&data, &header, ty.name_index)?.to_owned(),
                byval_type: required_type_index(ty.byval_type_index, "type byval type")?,
                declaring_type: optional_type_index(ty.declaring_type_index)?,
                parent: optional_type_index(ty.parent_index)?,
                element_type: optional_type_index(ty.element_type_index)?,
                generic_container_index: optional_checked_index(
                    ty.generic_container_index,
                    raw.generic_containers.len(),
                    "type generic container",
                )?,
                flags: ty.flags,
                bitfield: ty.bitfield,
                token: ty.token,
                methods,
                fields,
                properties,
                nested_types: nested_types[index].clone(),
                nested_in: nested_in[index],
                interfaces,
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
                    raw.generic_containers.len(),
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

    let properties = raw
        .properties
        .iter()
        .enumerate()
        .map(|(index, property)| {
            let declaring_type = TypeId(property_owners[index]);
            let owner = &raw.types[declaring_type.0];
            let getter = property_accessor(
                property.getter,
                owner.method_start,
                owner.method_count,
                raw.methods.len(),
            )?;
            let setter = property_accessor(
                property.setter,
                owner.method_start,
                owner.method_count,
                raw.methods.len(),
            )?;
            Ok(Property {
                id: PropertyId(index),
                declaring_type,
                name: string_at(&data, &header, property.name_index)?.to_owned(),
                getter,
                setter,
                attributes: property.attributes,
                token: property.token,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let generic_parameters = raw
        .generic_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let container = checked_index(
                parameter.owner_index,
                raw.generic_containers.len(),
                "generic parameter owner",
            )?;
            let constraints = checked_range(
                i32::from(parameter.constraints_start),
                usize::try_from(parameter.constraints_count)
                    .map_err(|_| Error::InvalidMetadataTable("generic parameter constraints"))?,
                raw.generic_parameter_constraints.len(),
                "generic parameter constraints",
            )?
            .map(|constraint| {
                required_type_index(
                    raw.generic_parameter_constraints[constraint],
                    "generic parameter constraint type",
                )
            })
            .collect::<Result<Vec<_>>>()?;
            Ok(GenericParameter {
                id: GenericParameterId(index),
                container: GenericContainerId(container),
                name: string_at(&data, &header, parameter.name_index)?.to_owned(),
                position: parameter.position,
                flags: parameter.flags,
                constraints,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let generic_containers = raw
        .generic_containers
        .iter()
        .enumerate()
        .map(|(index, container)| {
            let count = usize::try_from(container.type_argument_count)
                .map_err(|_| Error::InvalidMetadataTable("generic container argument count"))?;
            let parameters = checked_range(
                container.generic_parameter_start,
                count,
                raw.generic_parameters.len(),
                "generic container parameters",
            )?
            .map(|parameter| {
                if generic_parameters[parameter].container.0 != index {
                    return Err(Error::InvalidMetadataTable(
                        "generic parameter/container relationship",
                    ));
                }
                Ok(GenericParameterId(parameter))
            })
            .collect::<Result<Vec<_>>>()?;
            let owner = match container.is_method {
                0 => GenericOwner::Type(TypeId(checked_index(
                    container.owner_index,
                    raw.types.len(),
                    "generic container type owner",
                )?)),
                1 => GenericOwner::Method(MethodId(checked_index(
                    container.owner_index,
                    raw.methods.len(),
                    "generic container method owner",
                )?)),
                _ => return Err(Error::InvalidMetadataTable("generic container owner kind")),
            };
            Ok(GenericContainer {
                id: GenericContainerId(index),
                owner,
                parameters,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    for ty in &types {
        if let Some(container) = ty.generic_container_index
            && generic_containers[container].owner != GenericOwner::Type(ty.id)
        {
            return Err(Error::InvalidMetadataTable("type generic container owner"));
        }
    }
    for method in &methods {
        if let Some(container) = method.generic_container_index
            && generic_containers[container].owner != GenericOwner::Method(method.id)
        {
            return Err(Error::InvalidMetadataTable(
                "method generic container owner",
            ));
        }
    }

    Ok(Metadata {
        version,
        assemblies,
        images,
        types,
        methods,
        fields,
        parameters,
        properties,
        generic_containers,
        generic_parameters,
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

fn property_accessor(
    relative: i32,
    method_start: i32,
    method_count: u16,
    method_total: usize,
) -> Result<Option<MethodId>> {
    let Some(relative) = optional_index(relative)? else {
        return Ok(None);
    };
    if relative >= usize::from(method_count) {
        return Err(Error::InvalidMetadataTable("property accessor"));
    }
    let start = usize::try_from(method_start)
        .map_err(|_| Error::InvalidMetadataTable("property accessor"))?;
    let index = start
        .checked_add(relative)
        .filter(|index| *index < method_total)
        .ok_or(Error::InvalidMetadataTable("property accessor"))?;
    Ok(Some(MethodId(index)))
}
