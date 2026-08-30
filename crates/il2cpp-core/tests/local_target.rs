use std::path::PathBuf;

use il2cpp_core::analysis::TypeResolver;
use il2cpp_core::binary::{BinaryImage, ElfImage};
use il2cpp_core::metadata::{METADATA_SANITY, Metadata, MetadataVersion};
use il2cpp_core::model::{TypeKind, TypeRef};
use il2cpp_core::registration::RegistrationInfo;

fn workspace_file(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name)
}

#[test]
#[ignore = "requires local proprietary IL2CPP target files"]
fn parses_local_metadata_target() {
    let metadata = Metadata::open(workspace_file("global-metadata.dat")).expect("local metadata");

    assert_eq!(metadata.header().sanity, METADATA_SANITY);
    assert_eq!(metadata.version, MetadataVersion::V31);
    assert!(!metadata.images.is_empty());
    assert_eq!(metadata.images.len(), metadata.assemblies.len());
    assert!(metadata.types.len() > metadata.images.len());
    assert!(!metadata.fields.is_empty());
    assert!(!metadata.methods.is_empty());
    assert!(metadata.images.iter().all(|image| !image.name.is_empty()));
    assert!(metadata.types.iter().all(|ty| !ty.name.is_empty()));
}

#[test]
#[ignore = "requires local proprietary IL2CPP target files"]
fn translates_local_elf_load_segment_boundaries() {
    let binary = ElfImage::open(workspace_file("libil2cpp.so")).expect("local ELF");

    for segment in binary
        .segments()
        .iter()
        .filter(|segment| segment.kind == "LOAD" && segment.file_size > 0)
    {
        assert_eq!(
            binary.offset_to_virtual(segment.file_offset),
            Some(segment.virtual_address)
        );
        assert_eq!(
            binary.virtual_to_offset(segment.virtual_address),
            Some(segment.file_offset)
        );

        let last_offset = segment.file_offset + segment.file_size - 1;
        let last_address = segment.virtual_address + segment.file_size - 1;
        assert_eq!(binary.offset_to_virtual(last_offset), Some(last_address));
        assert_eq!(binary.virtual_to_offset(last_address), Some(last_offset));
    }
}

#[test]
#[ignore = "requires local proprietary IL2CPP target files"]
fn discovers_local_registrations_and_method_addresses() {
    let binary = ElfImage::open(workspace_file("libil2cpp.so")).expect("local ELF");
    let metadata = Metadata::open(workspace_file("global-metadata.dat")).expect("local metadata");
    let info = RegistrationInfo::discover(&binary, &metadata).expect("native registrations");
    let runtime = info
        .runtime_metadata(&binary, &metadata)
        .expect("runtime metadata tables");

    assert_eq!(info.registration.code_registration, Some(0x36802f0));
    assert_eq!(info.registration.metadata_registration, Some(0x3729660));
    assert_eq!(info.codegen_modules, 0x3875d98);
    assert_eq!(info.modules.len(), metadata.images.len());
    assert!(runtime.type_count() > metadata.types.len());
    assert!(
        metadata
            .fields
            .iter()
            .any(|field| runtime.field_offset(field.id).is_some())
    );
    assert!(
        metadata
            .images
            .iter()
            .all(|image| info.modules.iter().any(|module| module.name == image.name))
    );

    let resolved = metadata
        .methods
        .iter()
        .filter_map(|method| {
            info.resolve_method(&binary, &metadata, method.id)
                .expect("method resolution")
        })
        .count();
    assert!(resolved > 100_000);
}

#[test]
#[ignore = "requires local proprietary IL2CPP target files"]
fn resolves_local_runtime_types() {
    let binary = ElfImage::open(workspace_file("libil2cpp.so")).expect("local ELF");
    let metadata = Metadata::open(workspace_file("global-metadata.dat")).expect("local metadata");
    let info = RegistrationInfo::discover(&binary, &metadata).expect("native registrations");
    let runtime = info
        .runtime_metadata(&binary, &metadata)
        .expect("runtime metadata tables");
    let resolver = TypeResolver::with_runtime(&metadata, &binary, &runtime);

    let string_type = metadata
        .types
        .iter()
        .find(|ty| ty.namespace == "System" && ty.name == "String")
        .expect("System.String");
    let length_field = string_type
        .fields
        .iter()
        .map(|field| &metadata.fields[field.0])
        .find(|field| field.name == "_stringLength")
        .expect("String._stringLength");
    assert_eq!(
        resolver.field_signature(length_field).unwrap().ty,
        TypeRef::I32
    );

    let mut features = TypeFeatures::default();
    for field in &metadata.fields {
        record_type(&resolver.resolve(field.field_type).unwrap(), &mut features);
    }
    for method in &metadata.methods {
        let signature = resolver.method_signature(method).unwrap();
        record_type(&signature.return_type, &mut features);
        for parameter in signature.parameters {
            record_type(&parameter.ty, &mut features);
        }
    }
    assert_eq!(features.unknown, 0);
    assert!(features.array);
    assert!(features.by_ref);
    assert!(features.pointer);
    assert!(features.generic_parameter);
    assert!(features.generic_instance);
    assert!(
        metadata
            .types
            .iter()
            .any(|ty| ty.kind() == TypeKind::Struct)
    );
    assert!(metadata.types.iter().any(|ty| ty.kind() == TypeKind::Enum));
    assert!(
        metadata
            .types
            .iter()
            .any(|ty| ty.kind() == TypeKind::Interface)
    );
    assert!(metadata.types.iter().any(|ty| ty.nested_in.is_some()));
    assert!(
        metadata
            .types
            .iter()
            .map(|ty| ty.fields.len())
            .max()
            .unwrap()
            < 10_000
    );
    assert!(
        metadata
            .methods
            .iter()
            .map(|method| method.parameters.len())
            .max()
            .unwrap()
            < 1_024
    );
}

#[derive(Default)]
struct TypeFeatures {
    array: bool,
    by_ref: bool,
    pointer: bool,
    generic_parameter: bool,
    generic_instance: bool,
    unknown: usize,
}

fn record_type(ty: &TypeRef, features: &mut TypeFeatures) {
    match ty {
        TypeRef::Array { element, .. } => {
            features.array = true;
            record_type(element, features);
        }
        TypeRef::ByRef(element) => {
            features.by_ref = true;
            record_type(element, features);
        }
        TypeRef::Pointer(element) => {
            features.pointer = true;
            record_type(element, features);
        }
        TypeRef::GenericParameter { .. } => features.generic_parameter = true,
        TypeRef::GenericInstance { base, arguments } => {
            features.generic_instance = true;
            record_type(base, features);
            for argument in arguments {
                record_type(argument, features);
            }
        }
        TypeRef::Unknown { .. } => features.unknown += 1,
        _ => {}
    }
}
