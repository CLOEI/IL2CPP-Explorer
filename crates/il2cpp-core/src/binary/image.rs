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

    fn is_stripped(&self) -> bool;

    fn image_base(&self) -> u64;

    fn virtual_to_offset(&self, address: u64) -> Option<u64>;

    fn offset_to_virtual(&self, offset: u64) -> Option<u64>;

    fn read_virtual(&self, address: u64, size: usize) -> Result<&[u8]>;
}
