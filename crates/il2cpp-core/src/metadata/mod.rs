//! IL2CPP metadata header parsing and version dispatch.

mod header;
mod parser;
mod reader;
pub mod versions;

use std::path::Path;

pub use header::{METADATA_SANITY, MetadataHeader, MetadataTable, MetadataVersion};
pub use reader::MetadataReader;

use crate::model::{
    Assembly, Field, GenericContainer, GenericParameter, Image, Method, Parameter, Property,
    TypeDefinition,
};
use crate::{Error, Result};

/// Record widths selected by metadata-version parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataRecordSizes {
    pub image: usize,
    pub assembly: usize,
    pub type_definition: usize,
    pub field: usize,
    pub method: usize,
    pub parameter: usize,
}

/// Normalized metadata exposed to consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub version: MetadataVersion,
    pub assemblies: Vec<Assembly>,
    pub images: Vec<Image>,
    pub types: Vec<TypeDefinition>,
    pub methods: Vec<Method>,
    pub fields: Vec<Field>,
    pub parameters: Vec<Parameter>,
    pub properties: Vec<Property>,
    pub generic_containers: Vec<GenericContainer>,
    pub generic_parameters: Vec<GenericParameter>,
    header: MetadataHeader,
    data: Vec<u8>,
}

impl Metadata {
    /// Opens metadata and parses its validated header.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_vec(std::fs::read(path)?)
    }

    /// Parses metadata from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::from_vec(bytes.to_vec())
    }

    fn from_vec(data: Vec<u8>) -> Result<Self> {
        parser::parse(data)
    }

    /// Returns parsed metadata header.
    pub fn header(&self) -> &MetadataHeader {
        &self.header
    }

    /// Returns source metadata byte length.
    pub fn file_size(&self) -> usize {
        self.data.len()
    }

    /// Returns record widths used to validate this metadata version.
    pub fn record_sizes(&self) -> MetadataRecordSizes {
        versions::record_sizes()
    }

    /// Resolves one null-terminated UTF-8 string within string-data table.
    pub fn string(&self, index: u32) -> Result<&str> {
        string_at(&self.data, &self.header, index)
    }
}

pub(crate) fn string_at<'data>(
    data: &'data [u8],
    header: &MetadataHeader,
    index: u32,
) -> Result<&'data str> {
    let index = index as usize;
    if index >= header.strings.byte_count {
        return Err(Error::InvalidMetadataString(index as u32));
    }
    let table = data
        .get(header.strings.offset..header.strings.offset + header.strings.byte_count)
        .ok_or(Error::InvalidMetadata)?;
    let remaining = table
        .get(index..)
        .ok_or(Error::InvalidMetadataString(index as u32))?;
    let length = remaining
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::InvalidMetadataString(index as u32))?;
    std::str::from_utf8(&remaining[..length])
        .map_err(|_| Error::InvalidMetadataString(index as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_header(strings: MetadataTable) -> MetadataHeader {
        let mut bytes = vec![0_u8; 256_usize.max(strings.offset + strings.byte_count)];
        bytes[0..4].copy_from_slice(&METADATA_SANITY.to_le_bytes());
        bytes[4..8].copy_from_slice(&31_u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&(strings.offset as u32).to_le_bytes());
        bytes[28..32].copy_from_slice(&(strings.byte_count as u32).to_le_bytes());
        MetadataReader::new(&bytes).header().expect("valid header")
    }

    #[test]
    fn string_lookup_stays_inside_string_table() {
        let header = empty_header(MetadataTable {
            offset: 256,
            byte_count: 4,
        });
        let mut data = vec![0_u8; 260];
        data[256..].copy_from_slice(b"abc\0");

        assert_eq!(string_at(&data, &header, 0).expect("valid string"), "abc");
        assert!(matches!(
            string_at(&data, &header, 4),
            Err(Error::InvalidMetadataString(4))
        ));
    }

    #[test]
    fn string_lookup_requires_terminator_inside_table() {
        let header = empty_header(MetadataTable {
            offset: 256,
            byte_count: 3,
        });
        let mut data = vec![0_u8; 260];
        data[256..260].copy_from_slice(b"abc\0");

        assert!(matches!(
            string_at(&data, &header, 0),
            Err(Error::InvalidMetadataString(0))
        ));
    }
}
