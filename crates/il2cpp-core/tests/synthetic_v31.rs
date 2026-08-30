use il2cpp_core::metadata::{METADATA_SANITY, Metadata, MetadataVersion};

const STRINGS_PAIR: usize = 2;
const METHODS_PAIR: usize = 5;
const PARAMETERS_PAIR: usize = 10;
const FIELDS_PAIR: usize = 11;
const TYPES_PAIR: usize = 19;
const IMAGES_PAIR: usize = 20;
const ASSEMBLIES_PAIR: usize = 21;

#[test]
fn parses_minimal_v31_model() {
    let bytes = minimal_metadata();
    let metadata = Metadata::from_bytes(&bytes).expect("minimal v31 metadata");

    assert_eq!(metadata.version, MetadataVersion::V31);
    assert_eq!(metadata.images.len(), 1);
    assert_eq!(metadata.images[0].name, "Test.dll");
    assert_eq!(metadata.assemblies[0].name, "Test");
    assert_eq!(metadata.types[0].namespace, "Ns");
    assert_eq!(metadata.types[0].name, "Type");
    assert_eq!(metadata.fields[0].name, "field");
    assert_eq!(metadata.methods[0].name, "Method");
    assert_eq!(metadata.parameters[0].name, "param");
}

#[test]
fn rejects_overlapping_v31_tables() {
    let mut bytes = minimal_metadata();
    let type_offset = read_u32(&bytes, pair_offset(TYPES_PAIR));
    write_u32(&mut bytes, pair_offset(IMAGES_PAIR), type_offset);

    assert!(Metadata::from_bytes(&bytes).is_err());
}

fn minimal_metadata() -> Vec<u8> {
    let mut data = vec![0_u8; 256];
    write_u32(&mut data, 0, METADATA_SANITY);
    write_u32(&mut data, 4, 31);

    add_table(
        &mut data,
        STRINGS_PAIR,
        b"\0Test.dll\0Test\0Ns\0Type\0field\0Method\0param\0",
    );

    let mut method = vec![0_u8; 36];
    write_u32(&mut method, 0, 29);
    write_i32(&mut method, 4, 0);
    write_i32(&mut method, 8, 0);
    write_u32(&mut method, 12, 0x0800_0001);
    write_i32(&mut method, 16, 0);
    write_i32(&mut method, 20, -1);
    write_u32(&mut method, 24, 0x0600_0001);
    write_u16(&mut method, 34, 1);
    add_table(&mut data, METHODS_PAIR, &method);

    let mut parameter = vec![0_u8; 12];
    write_u32(&mut parameter, 0, 36);
    write_u32(&mut parameter, 4, 0x0800_0002);
    write_i32(&mut parameter, 8, 0);
    add_table(&mut data, PARAMETERS_PAIR, &parameter);

    let mut field = vec![0_u8; 12];
    write_u32(&mut field, 0, 23);
    write_i32(&mut field, 4, 0);
    write_u32(&mut field, 8, 0x0400_0001);
    add_table(&mut data, FIELDS_PAIR, &field);

    let mut ty = vec![0_u8; 88];
    write_u32(&mut ty, 0, 18);
    write_u32(&mut ty, 4, 15);
    write_i32(&mut ty, 12, -1);
    write_i32(&mut ty, 16, -1);
    write_i32(&mut ty, 24, -1);
    write_i32(&mut ty, 32, 0);
    write_i32(&mut ty, 36, 0);
    write_u16(&mut ty, 64, 1);
    write_u16(&mut ty, 68, 1);
    add_table(&mut data, TYPES_PAIR, &ty);

    let mut image = vec![0_u8; 40];
    write_u32(&mut image, 0, 1);
    write_i32(&mut image, 4, 0);
    write_i32(&mut image, 8, 0);
    write_u32(&mut image, 12, 1);
    add_table(&mut data, IMAGES_PAIR, &image);

    let mut assembly = vec![0_u8; 64];
    write_i32(&mut assembly, 0, 0);
    write_u32(&mut assembly, 16, 10);
    add_table(&mut data, ASSEMBLIES_PAIR, &assembly);
    data
}

fn add_table(data: &mut Vec<u8>, pair: usize, bytes: &[u8]) {
    let offset = data.len() as u32;
    write_u32(data, pair_offset(pair), offset);
    write_u32(data, pair_offset(pair) + 4, bytes.len() as u32);
    data.extend_from_slice(bytes);
}

const fn pair_offset(pair: usize) -> usize {
    8 + pair * 8
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().expect("four bytes"))
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
