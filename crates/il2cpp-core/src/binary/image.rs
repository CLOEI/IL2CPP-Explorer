use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Result;

/// CPU architecture reported by a loaded executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Architecture {
    Arm64,
    X86_64,
    Unknown,
}

impl fmt::Display for Architecture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arm64 => formatter.write_str("AArch64"),
            Self::X86_64 => formatter.write_str("x86_64"),
            Self::Unknown => formatter.write_str("Unknown"),
        }
    }
}

/// Executable container format exposed by a loaded project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryFormat {
    Elf64,
}

/// Byte order used by an executable image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Endianness {
    Little,
    Big,
}

impl fmt::Display for Endianness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Little => formatter.write_str("Little"),
            Self::Big => formatter.write_str("Big"),
        }
    }
}

/// Linker-level ELF object type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryKind {
    SharedObject,
    Executable,
    Relocatable,
    Unknown,
}

impl fmt::Display for BinaryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SharedObject => formatter.write_str("Shared Object"),
            Self::Executable => formatter.write_str("Executable"),
            Self::Relocatable => formatter.write_str("Relocatable"),
            Self::Unknown => formatter.write_str("Unknown"),
        }
    }
}

/// Read, write, and execute permissions for a mapped range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl fmt::Display for Permissions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{}{}",
            if self.read { 'R' } else { '-' },
            if self.write { 'W' } else { '-' },
            if self.execute { 'X' } else { '-' },
        )
    }
}

/// ELF section information useful to analysis consumers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionInfo {
    pub name: String,
    pub file_offset: Option<u64>,
    pub virtual_address: u64,
    pub size: u64,
    pub permissions: Permissions,
}

/// ELF program-header information.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub kind: String,
    pub file_offset: u64,
    pub file_size: u64,
    pub virtual_address: u64,
    pub virtual_size: u64,
    pub alignment: u64,
    pub permissions: Permissions,
}

/// One base-relative pointer relocation applied when an image is loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelativeRelocation {
    pub address: u64,
    pub addend: i64,
}

impl fmt::Display for BinaryFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Elf64 => formatter.write_str("ELF64"),
        }
    }
}

/// Common access interface for executable images.
pub trait BinaryImage {
    fn format(&self) -> BinaryFormat;

    fn architecture(&self) -> Architecture;

    fn endianness(&self) -> Endianness;

    fn kind(&self) -> BinaryKind;

    fn file_size(&self) -> u64;

    fn entry_point(&self) -> u64;

    fn section_count(&self) -> usize;

    fn sections(&self) -> &[SectionInfo];

    fn segments(&self) -> &[SegmentInfo];

    fn relative_relocations(&self) -> &[RelativeRelocation];

    fn is_stripped(&self) -> bool;

    fn image_base(&self) -> u64;

    fn virtual_to_offset(&self, address: u64) -> Option<u64>;

    fn offset_to_virtual(&self, offset: u64) -> Option<u64>;

    fn read_virtual(&self, address: u64, size: usize) -> Result<&[u8]>;

    /// Returns the executable LOAD segment containing a virtual address.
    fn executable_segment(&self, address: u64) -> Option<&SegmentInfo> {
        self.segments().iter().find(|segment| {
            segment.kind == "LOAD"
                && segment.permissions.execute
                && address
                    .checked_sub(segment.virtual_address)
                    .is_some_and(|relative| {
                        relative < segment.virtual_size && relative < segment.file_size
                    })
        })
    }

    /// Reads a checked, file-backed range from one executable LOAD segment.
    fn read_executable(&self, address: u64, size: usize) -> Result<&[u8]> {
        let size = u64::try_from(size).map_err(|_| crate::Error::AddressTranslationFailed)?;
        let segment = self
            .executable_segment(address)
            .ok_or(crate::Error::AddressTranslationFailed)?;
        let relative = address
            .checked_sub(segment.virtual_address)
            .ok_or(crate::Error::AddressTranslationFailed)?;
        let end = relative
            .checked_add(size)
            .ok_or(crate::Error::AddressTranslationFailed)?;
        if end > segment.virtual_size || end > segment.file_size {
            return Err(crate::Error::AddressTranslationFailed);
        }
        self.read_virtual(
            address,
            usize::try_from(size).map_err(|_| crate::Error::AddressTranslationFailed)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Result};

    struct TestImage {
        data: Vec<u8>,
        segments: Vec<SegmentInfo>,
    }

    impl BinaryImage for TestImage {
        fn format(&self) -> BinaryFormat {
            BinaryFormat::Elf64
        }
        fn architecture(&self) -> Architecture {
            Architecture::Arm64
        }
        fn endianness(&self) -> Endianness {
            Endianness::Little
        }
        fn kind(&self) -> BinaryKind {
            BinaryKind::SharedObject
        }
        fn file_size(&self) -> u64 {
            self.data.len() as u64
        }
        fn entry_point(&self) -> u64 {
            0
        }
        fn section_count(&self) -> usize {
            0
        }
        fn sections(&self) -> &[SectionInfo] {
            &[]
        }
        fn segments(&self) -> &[SegmentInfo] {
            &self.segments
        }
        fn relative_relocations(&self) -> &[RelativeRelocation] {
            &[]
        }
        fn is_stripped(&self) -> bool {
            true
        }
        fn image_base(&self) -> u64 {
            0x1000
        }
        fn virtual_to_offset(&self, address: u64) -> Option<u64> {
            address.checked_sub(0x1000)
        }
        fn offset_to_virtual(&self, offset: u64) -> Option<u64> {
            0x1000_u64.checked_add(offset)
        }
        fn read_virtual(&self, address: u64, size: usize) -> Result<&[u8]> {
            let start = usize::try_from(
                address
                    .checked_sub(0x1000)
                    .ok_or(Error::AddressTranslationFailed)?,
            )
            .map_err(|_| Error::AddressTranslationFailed)?;
            let end = start
                .checked_add(size)
                .ok_or(Error::AddressTranslationFailed)?;
            self.data
                .get(start..end)
                .ok_or(Error::AddressTranslationFailed)
        }
    }

    fn test_image(execute: bool) -> TestImage {
        TestImage {
            data: (0_u8..32).collect(),
            segments: vec![SegmentInfo {
                kind: "LOAD".to_owned(),
                file_offset: 0,
                file_size: 16,
                virtual_address: 0x1000,
                virtual_size: 24,
                alignment: 0x1000,
                permissions: Permissions {
                    read: true,
                    write: false,
                    execute,
                },
            }],
        }
    }

    #[test]
    fn reads_only_ranges_contained_in_executable_file_bytes() {
        let image = test_image(true);
        assert_eq!(image.read_executable(0x1004, 4).unwrap(), &[4, 5, 6, 7]);
        assert!(image.read_executable(0x100f, 2).is_err());
        assert!(image.read_executable(0x1010, 0).is_err());
        assert!(image.read_executable(u64::MAX, 4).is_err());
    }

    #[test]
    fn rejects_non_executable_ranges() {
        assert!(test_image(false).read_executable(0x1000, 4).is_err());
    }
}
