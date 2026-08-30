use std::path::PathBuf;

use il2cpp_core::binary::{BinaryImage, ElfImage};
use il2cpp_core::metadata::{METADATA_SANITY, Metadata, MetadataVersion};
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

    assert_eq!(info.registration.code_registration, Some(0x36802f0));
    assert_eq!(info.registration.metadata_registration, Some(0x3729660));
    assert_eq!(info.codegen_modules, 0x3875d98);
    assert_eq!(info.modules.len(), metadata.images.len());
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
