//! Metadata-version dispatch. Format differences belong in this module tree.

mod v31;

use super::{MetadataRecordSizes, MetadataVersion};
use crate::{Error, Result};

pub(crate) fn resolve(version: u32) -> Result<MetadataVersion> {
    match version {
        v31::VERSION => Ok(MetadataVersion::V31),
        unsupported => Err(Error::UnsupportedMetadataVersion(unsupported)),
    }
}

pub(crate) use v31::{parse_header, parse_records};

pub(crate) fn record_sizes() -> MetadataRecordSizes {
    MetadataRecordSizes {
        image: v31::IMAGE_SIZE,
        assembly: v31::ASSEMBLY_SIZE,
        type_definition: v31::TYPE_DEFINITION_SIZE,
        field: v31::FIELD_SIZE,
        method: v31::METHOD_SIZE,
        parameter: v31::PARAMETER_SIZE,
    }
}
