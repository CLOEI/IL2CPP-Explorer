use std::fmt;

use serde::{Deserialize, Serialize};

/// Sanity value at the start of an unprotected IL2CPP metadata file.
pub const METADATA_SANITY: u32 = 0xFAB1_1BAF;

/// Supported metadata format families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetadataVersion {
    V24,
    V27,
    V29,
    Unknown(u32),
}

impl MetadataVersion {
    /// Returns the integer stored in the metadata header.
    pub const fn raw(self) -> u32 {
        match self {
            Self::V24 => 24,
            Self::V27 => 27,
            Self::V29 => 29,
            Self::Unknown(version) => version,
        }
    }
}

impl fmt::Display for MetadataVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw().fmt(formatter)
    }
}

/// Common prefix of an IL2CPP metadata header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetadataHeader {
    pub sanity: u32,
    pub version: u32,
}
