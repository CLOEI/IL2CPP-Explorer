//! IL2CPP metadata header parsing and version dispatch.

mod header;
mod reader;
pub mod versions;

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use header::{METADATA_SANITY, MetadataHeader, MetadataVersion};
pub use reader::MetadataReader;

use crate::Result;
use crate::model::{Assembly, Field, Image, Method, TypeDefinition};

/// Normalized metadata exposed to consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub version: MetadataVersion,
    pub assemblies: Vec<Assembly>,
    pub images: Vec<Image>,
    pub types: Vec<TypeDefinition>,
    pub methods: Vec<Method>,
    pub fields: Vec<Field>,
}

impl Metadata {
    /// Opens metadata and parses its validated header.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        MetadataReader::new(&bytes).read()
    }

    /// Parses metadata from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        MetadataReader::new(bytes).read()
    }
}
