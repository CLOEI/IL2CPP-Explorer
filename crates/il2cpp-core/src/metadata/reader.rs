use super::{METADATA_SANITY, Metadata, MetadataHeader, versions};
use crate::{Error, Result};

/// Reader for an unprotected `global-metadata.dat` payload.
#[derive(Debug, Clone, Copy)]
pub struct MetadataReader<'data> {
    data: &'data [u8],
}

impl<'data> MetadataReader<'data> {
    /// Creates a checked reader over metadata bytes.
    pub fn new(data: &'data [u8]) -> Self {
        Self { data }
    }

    /// Returns source byte length.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns whether source is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Reads checked bytes without crossing file bounds.
    pub fn read_bytes(&self, offset: usize, length: usize) -> Result<&'data [u8]> {
        let end = offset
            .checked_add(length)
            .ok_or(Error::MetadataOutOfBounds { offset, length })?;
        self.data
            .get(offset..end)
            .ok_or(Error::MetadataOutOfBounds { offset, length })
    }

    /// Reads a little-endian unsigned 16-bit value.
    pub fn read_u16(&self, offset: usize) -> Result<u16> {
        let bytes = self.read_bytes(offset, 2)?;
        Ok(u16::from_le_bytes(
            bytes.try_into().map_err(|_| Error::InvalidMetadata)?,
        ))
    }

    /// Reads a little-endian unsigned 32-bit value.
    pub fn read_u32(&self, offset: usize) -> Result<u32> {
        let bytes = self.read_bytes(offset, 4)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().map_err(|_| Error::InvalidMetadata)?,
        ))
    }

    /// Reads a little-endian signed 32-bit value.
    pub fn read_i32(&self, offset: usize) -> Result<i32> {
        let bytes = self.read_bytes(offset, 4)?;
        Ok(i32::from_le_bytes(
            bytes.try_into().map_err(|_| Error::InvalidMetadata)?,
        ))
    }

    /// Reads a little-endian unsigned 64-bit value.
    pub fn read_u64(&self, offset: usize) -> Result<u64> {
        let bytes = self.read_bytes(offset, 8)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().map_err(|_| Error::InvalidMetadata)?,
        ))
    }

    /// Reads and validates the version-independent header prefix.
    pub fn header(&self) -> Result<MetadataHeader> {
        let sanity = self.read_u32(0)?;
        let version = self.read_u32(4)?;
        if sanity != METADATA_SANITY {
            return Err(Error::InvalidMetadata);
        }
        versions::resolve(version)?;
        versions::parse_header(self, sanity, version)
    }

    /// Parses currently supported metadata information.
    pub fn read(&self) -> Result<Metadata> {
        Metadata::from_bytes(self.data)
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
        let mut bytes = metadata_prefix(METADATA_SANITY, 31);
        bytes.resize(256, 0);
        let header = MetadataReader::new(&bytes).header().expect("valid header");

        assert_eq!(header.version, MetadataVersion::V31.raw());
    }

    #[test]
    fn rejects_invalid_sanity() {
        let bytes = metadata_prefix(0, 31);
        let error = MetadataReader::new(&bytes)
            .header()
            .expect_err("bad sanity");

        assert!(matches!(error, Error::InvalidMetadata));
    }

    #[test]
    fn rejects_unsupported_version() {
        let bytes = metadata_prefix(METADATA_SANITY, 30);
        let error = MetadataReader::new(&bytes)
            .header()
            .expect_err("unsupported version");

        assert!(matches!(error, Error::UnsupportedMetadataVersion(30)));
    }

    #[test]
    fn checked_reads_reject_overflow_and_truncation() {
        let reader = MetadataReader::new(&[1, 2, 3, 4]);

        assert_eq!(reader.read_u16(1).expect("in bounds"), 0x0302);
        assert!(matches!(
            reader.read_u64(0),
            Err(Error::MetadataOutOfBounds { .. })
        ));
        assert!(matches!(
            reader.read_bytes(usize::MAX, 2),
            Err(Error::MetadataOutOfBounds { .. })
        ));
    }
}
