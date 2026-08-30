//! Metadata-version dispatch. Format differences belong in this module tree.

mod v24;
mod v27;
mod v29;

use super::MetadataVersion;
use crate::{Error, Result};

pub(crate) fn resolve(version: u32) -> Result<MetadataVersion> {
    match version {
        v24::VERSION => Ok(MetadataVersion::V24),
        v27::VERSION => Ok(MetadataVersion::V27),
        v29::VERSION => Ok(MetadataVersion::V29),
        unsupported => Err(Error::UnsupportedMetadataVersion(unsupported)),
    }
}
