use std::path::Path;

use object::read::elf::ProgramHeader as _;
use object::{BinaryFormat as ObjectFormat, Object, ObjectSection, ObjectSegment};

use super::{
    Architecture, BinaryFormat, BinaryImage, BinaryKind, Endianness, Permissions, SectionInfo,
    SegmentInfo,
};
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
    endianness: Endianness,
    kind: BinaryKind,
    entry_point: u64,
    image_base: u64,
    segments: Vec<Segment>,
    program_segments: Vec<SegmentInfo>,
    sections: Vec<SectionInfo>,
    section_count: usize,
    stripped: bool,
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
        let endianness = if file.is_little_endian() {
            Endianness::Little
        } else {
            Endianness::Big
        };
        let object_kind = file.kind();
        let entry_point = file.entry();
        let stripped = file.symbol_table().is_none();

        let sections = file
            .sections()
            .map(|section| {
                if let Some((offset, size)) = section.file_range() {
                    validate_range(offset, size, data.len() as u64)?;
                }
                section
                    .address()
                    .checked_add(section.size())
                    .ok_or(Error::InvalidBinary)?;
                let permissions = match section.flags() {
                    object::SectionFlags::Elf { sh_flags, .. } => Permissions {
                        read: sh_flags.contains(object::elf::SHF_ALLOC),
                        write: sh_flags.contains(object::elf::SHF_WRITE),
                        execute: sh_flags.contains(object::elf::SHF_EXECINSTR),
                    },
                    _ => Permissions::default(),
                };
                Ok(SectionInfo {
                    name: section.name().unwrap_or("<invalid>").to_owned(),
                    file_offset: section.file_range().map(|(offset, _)| offset),
                    virtual_address: section.address(),
                    size: section.size(),
                    permissions,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let (program_segments, section_count) = match &file {
            object::File::Elf64(elf) => {
                let endian = elf.endian();
                let segments = elf
                    .elf_program_headers()
                    .iter()
                    .map(|segment| {
                        let flags = segment.p_flags(endian);
                        let file_offset = segment.p_offset(endian);
                        let file_size = segment.p_filesz(endian);
                        let virtual_address = segment.p_vaddr(endian);
                        let virtual_size = segment.p_memsz(endian);
                        validate_range(file_offset, file_size, data.len() as u64)?;
                        if segment.p_type(endian) == object::elf::PT_LOAD
                            && file_size > virtual_size
                        {
                            return Err(Error::InvalidBinary);
                        }
                        virtual_address
                            .checked_add(virtual_size)
                            .ok_or(Error::InvalidBinary)?;
                        Ok(SegmentInfo {
                            kind: program_type_name(segment.p_type(endian)).to_owned(),
                            file_offset,
                            file_size,
                            virtual_address,
                            virtual_size,
                            alignment: segment.p_align(endian),
                            permissions: Permissions {
                                read: flags.contains(object::elf::PF_R),
                                write: flags.contains(object::elf::PF_W),
                                execute: flags.contains(object::elf::PF_X),
                            },
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                (segments, elf.elf_section_table().len())
            }
            _ => return Err(Error::InvalidBinary),
        };

        let segments: Vec<_> = file
            .segments()
            .map(|segment| {
                let (file_offset, file_size) = segment.file_range();
                validate_range(file_offset, file_size, data.len() as u64)?;
                if file_size > segment.size() {
                    return Err(Error::InvalidBinary);
                }
                segment
                    .address()
                    .checked_add(segment.size())
                    .ok_or(Error::InvalidBinary)?;
                Ok(Segment {
                    virtual_address: segment.address(),
                    virtual_size: segment.size(),
                    file_offset,
                    file_size,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let kind = match object_kind {
            object::ObjectKind::Dynamic
                if program_segments
                    .iter()
                    .any(|segment| segment.kind == "INTERP") =>
            {
                BinaryKind::Executable
            }
            object::ObjectKind::Dynamic => BinaryKind::SharedObject,
            object::ObjectKind::Executable => BinaryKind::Executable,
            object::ObjectKind::Relocatable => BinaryKind::Relocatable,
            _ => BinaryKind::Unknown,
        };
        let image_base = segments
            .iter()
            .map(|segment| segment.virtual_address)
            .min()
            .unwrap_or(0);

        Ok(Self {
            data,
            architecture,
            endianness,
            kind,
            entry_point,
            image_base,
            segments,
            program_segments,
            sections,
            section_count,
            stripped,
        })
    }
}

impl BinaryImage for ElfImage {
    fn format(&self) -> BinaryFormat {
        BinaryFormat::Elf64
    }

    fn architecture(&self) -> Architecture {
        self.architecture
    }

    fn endianness(&self) -> Endianness {
        self.endianness
    }

    fn kind(&self) -> BinaryKind {
        self.kind
    }

    fn file_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn entry_point(&self) -> u64 {
        self.entry_point
    }

    fn section_count(&self) -> usize {
        self.section_count
    }

    fn sections(&self) -> &[SectionInfo] {
        &self.sections
    }

    fn segments(&self) -> &[SegmentInfo] {
        &self.program_segments
    }

    fn is_stripped(&self) -> bool {
        self.stripped
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

fn program_type_name(program_type: object::elf::ProgramType) -> &'static str {
    if program_type == object::elf::PT_LOAD {
        "LOAD"
    } else if program_type == object::elf::PT_PHDR {
        "PHDR"
    } else if program_type == object::elf::PT_DYNAMIC {
        "DYNAMIC"
    } else if program_type == object::elf::PT_INTERP {
        "INTERP"
    } else if program_type == object::elf::PT_NOTE {
        "NOTE"
    } else if program_type == object::elf::PT_GNU_RELRO {
        "GNU_RELRO"
    } else if program_type == object::elf::PT_GNU_EH_FRAME {
        "GNU_EH_FRAME"
    } else if program_type == object::elf::PT_GNU_STACK {
        "GNU_STACK"
    } else {
        "OTHER"
    }
}

fn validate_range(offset: u64, size: u64, file_size: u64) -> Result<()> {
    let end = offset.checked_add(size).ok_or(Error::InvalidBinary)?;
    if offset > file_size || end > file_size {
        return Err(Error::InvalidBinary);
    }
    Ok(())
}
