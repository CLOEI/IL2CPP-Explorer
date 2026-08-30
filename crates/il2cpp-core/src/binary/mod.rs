//! Executable-format abstractions and format-specific readers.

mod elf;
mod image;
mod macho;
mod pe;

pub use elf::ElfImage;
pub use image::{Architecture, BinaryFormat, BinaryImage};
pub use macho::MachOImage;
pub use pe::PeImage;
