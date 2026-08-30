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

impl fmt::Display for BinaryFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Elf64 => formatter.write_str("ELF64"),
        }
    }
}

/// Common access interface for executable images.
pub trait BinaryImage {
    fn architecture(&self) -> Architecture;

    fn image_base(&self) -> u64;

    fn virtual_to_offset(&self, address: u64) -> Option<u64>;

    fn offset_to_virtual(&self, offset: u64) -> Option<u64>;

    fn read_virtual(&self, address: u64, size: usize) -> Result<&[u8]>;
}
