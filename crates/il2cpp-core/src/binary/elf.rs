use std::path::Path;

use object::{BinaryFormat as ObjectFormat, Object, ObjectSegment};

use super::{Architecture, BinaryImage};
use crate::{Error, Result};

#[derive(Debug)]
struct Segment {
    virtual_address: u64,
    virtual_size: u64,
    file_offset: u64,
    file_size: u64,
}

/// Loaded 64-bit ELF executable image.
#[derive(Debug)]
pub struct ElfImage {
    data: Vec<u8>,
    architecture: Architecture,
    image_base: u64,
    segments: Vec<Segment>,
}

impl ElfImage {
    /// Opens and validates a 64-bit ELF image.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::parse(std::fs::read(path)?)
    }

    /// Parses a 64-bit ELF image from owned bytes.
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        let file = object::File::parse(data.as_slice()).map_err(|_| Error::InvalidBinary)?;
        if file.format() != ObjectFormat::Elf || !file.is_64() {
            return Err(Error::InvalidBinary);
        }

        let architecture = match file.architecture() {
            object::Architecture::Aarch64 => Architecture::Arm64,
            object::Architecture::X86_64 => Architecture::X86_64,
            _ => Architecture::Unknown,
        };

        let segments: Vec<_> = file
            .segments()
            .map(|segment| {
                let (file_offset, file_size) = segment.file_range();
                Segment {
                    virtual_address: segment.address(),
                    virtual_size: segment.size(),
                    file_offset,
                    file_size,
                }
            })
            .collect();
        let image_base = segments
            .iter()
            .map(|segment| segment.virtual_address)
            .min()
            .unwrap_or(0);

        Ok(Self {
            data,
            architecture,
            image_base,
            segments,
        })
    }
}

impl BinaryImage for ElfImage {
    fn architecture(&self) -> Architecture {
        self.architecture
    }

    fn image_base(&self) -> u64 {
        self.image_base
    }

    fn virtual_to_offset(&self, address: u64) -> Option<u64> {
        self.segments.iter().find_map(|segment| {
            let relative = address.checked_sub(segment.virtual_address)?;
            (relative < segment.file_size)
                .then_some(relative)
                .and_then(|relative| segment.file_offset.checked_add(relative))
        })
    }

    fn offset_to_virtual(&self, offset: u64) -> Option<u64> {
        self.segments.iter().find_map(|segment| {
            let relative = offset.checked_sub(segment.file_offset)?;
            (relative < segment.file_size)
                .then_some(relative)
                .and_then(|relative| segment.virtual_address.checked_add(relative))
        })
    }

    fn read_virtual(&self, address: u64, size: usize) -> Result<&[u8]> {
        let size = u64::try_from(size).map_err(|_| Error::AddressTranslationFailed)?;
        let segment = self
            .segments
            .iter()
            .find(|segment| {
                address
                    .checked_sub(segment.virtual_address)
                    .and_then(|relative| relative.checked_add(size))
                    .is_some_and(|end| end <= segment.file_size && end <= segment.virtual_size)
            })
            .ok_or(Error::AddressTranslationFailed)?;
        let relative = address - segment.virtual_address;
        let start = segment
            .file_offset
            .checked_add(relative)
            .ok_or(Error::AddressTranslationFailed)?;
        let end = start
            .checked_add(size)
            .ok_or(Error::AddressTranslationFailed)?;
        let start = usize::try_from(start).map_err(|_| Error::AddressTranslationFailed)?;
        let end = usize::try_from(end).map_err(|_| Error::AddressTranslationFailed)?;

        self.data
            .get(start..end)
            .ok_or(Error::AddressTranslationFailed)
    }
}
