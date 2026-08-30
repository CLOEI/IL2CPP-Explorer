use super::{METADATA_SANITY, Metadata, MetadataHeader, versions};
use crate::{Error, Result};

/// Reader for an unprotected `global-metadata.dat` payload.
#[derive(Debug, Clone, Copy)]
pub struct MetadataReader<'data> {
    data: &'data [u8],
}

impl<'data> MetadataReader<'data> {
    pub fn new(data: &'data [u8]) -> Self {
        Self { data }
    }

    /// Reads and validates the version-independent header prefix.
    pub fn header(&self) -> Result<MetadataHeader> {
        let prefix = self.data.get(..8).ok_or(Error::InvalidMetadata)?;
        let sanity = u32::from_le_bytes(
            prefix[0..4]
                .try_into()
                .map_err(|_| Error::InvalidMetadata)?,
        );
        let version = u32::from_le_bytes(
            prefix[4..8]
                .try_into()
                .map_err(|_| Error::InvalidMetadata)?,
        );
        if sanity != METADATA_SANITY {
            return Err(Error::InvalidMetadata);
        }

        Ok(MetadataHeader { sanity, version })
    }

    /// Parses currently supported metadata information.
    pub fn read(&self) -> Result<Metadata> {
        let header = self.header()?;
        let version = versions::resolve(header.version)?;

        Ok(Metadata {
            version,
            assemblies: Vec::new(),
            images: Vec::new(),
            types: Vec::new(),
            methods: Vec::new(),
            fields: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataVersion;

    fn metadata_prefix(sanity: u32, version: u32) -> Vec<u8> {
        [sanity.to_le_bytes(), version.to_le_bytes()].concat()
    }

    #[test]
    fn reads_supported_header() {
        let bytes = metadata_prefix(METADATA_SANITY, 29);
        let metadata = MetadataReader::new(&bytes).read().expect("valid header");

        assert_eq!(metadata.version, MetadataVersion::V29);
    }

    #[test]
    fn rejects_invalid_sanity() {
        let bytes = metadata_prefix(0, 29);
        let error = MetadataReader::new(&bytes).read().expect_err("bad sanity");

        assert!(matches!(error, Error::InvalidMetadata));
    }

    #[test]
    fn rejects_unsupported_version() {
        let bytes = metadata_prefix(METADATA_SANITY, 30);
        let error = MetadataReader::new(&bytes)
            .read()
            .expect_err("unsupported version");

        assert!(matches!(error, Error::UnsupportedMetadataVersion(30)));
    }
}
