use std::path::Path;

use crate::Error;
use crate::Result;
use crate::binary::{
    Architecture, BinaryFormat, BinaryImage, BinaryKind, ElfImage, Endianness, RelativeRelocation,
    SectionInfo, SegmentInfo,
};
use crate::metadata::{Metadata, MetadataVersion};
use crate::registration::{NativeMethodIndex, Registration, RegistrationInfo, RuntimeMetadata};

/// A loaded executable and its matching normalized IL2CPP metadata.
pub struct Il2CppProject {
    binary: Box<dyn BinaryImage>,
    binary_format: BinaryFormat,
    metadata: Metadata,
    registration: Option<Registration>,
    registration_info: Option<RegistrationInfo>,
    runtime_metadata: Option<RuntimeMetadata>,
    native_methods: Option<NativeMethodIndex>,
}

impl Il2CppProject {
    /// Loads an ELF64 binary and an unprotected `global-metadata.dat` file.
    pub fn load(binary_path: impl AsRef<Path>, metadata_path: impl AsRef<Path>) -> Result<Self> {
        let binary = ElfImage::open(binary_path)?;
        let metadata = Metadata::open(metadata_path)?;

        Ok(Self {
            binary: Box::new(binary),
            binary_format: BinaryFormat::Elf64,
            metadata,
            registration: None,
            registration_info: None,
            runtime_metadata: None,
            native_methods: None,
        })
    }

    /// Loads metadata without an executable. Metadata comparison/export remains available;
    /// registration, addresses, and native inspection are intentionally unavailable.
    pub fn load_metadata_only(metadata_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            binary: Box::new(MetadataOnlyBinary),
            binary_format: BinaryFormat::Elf64,
            metadata: Metadata::open(metadata_path)?,
            registration: None,
            registration_info: None,
            runtime_metadata: None,
            native_methods: None,
        })
    }

    /// Returns the executable-format family.
    pub fn binary_format(&self) -> BinaryFormat {
        self.binary_format
    }

    /// Returns the executable architecture.
    pub fn architecture(&self) -> Architecture {
        self.binary.architecture()
    }

    /// Returns the parsed metadata version.
    pub fn metadata_version(&self) -> MetadataVersion {
        self.metadata.version
    }

    /// Returns the normalized metadata model.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Returns the executable through its format-independent interface.
    pub fn binary(&self) -> &dyn BinaryImage {
        self.binary.as_ref()
    }

    /// Returns resolved registration addresses, if discovery has run.
    pub fn registration(&self) -> Option<&Registration> {
        self.registration.as_ref()
    }

    /// Returns native method mappings after [`Self::prepare_analysis`] succeeds.
    pub fn native_methods(&self) -> Option<&NativeMethodIndex> {
        self.native_methods.as_ref()
    }

    /// Returns runtime type and field metadata after [`Self::prepare_analysis`] succeeds.
    pub fn runtime_metadata(&self) -> Option<&RuntimeMetadata> {
        self.runtime_metadata.as_ref()
    }

    /// Discovers native registration roots and stores them on this project.
    pub fn discover_registration(&mut self) -> Result<&Registration> {
        if self.registration.is_none() {
            let info = RegistrationInfo::discover(self.binary.as_ref(), &self.metadata)?;
            self.registration = Some(info.registration);
            self.registration_info = Some(info);
        }
        Ok(self
            .registration
            .as_ref()
            .expect("registration was assigned"))
    }

    /// Discovers runtime registration, type data, field offsets, and native method mappings.
    pub fn prepare_analysis(&mut self) -> Result<()> {
        if self.native_methods.is_some() && self.runtime_metadata.is_some() {
            return Ok(());
        }
        self.discover_registration()?;
        let registration = self
            .registration_info
            .as_ref()
            .expect("registration info was assigned");
        self.runtime_metadata =
            Some(registration.runtime_metadata(self.binary.as_ref(), &self.metadata)?);
        self.native_methods = Some(NativeMethodIndex::build(
            self.binary.as_ref(),
            &self.metadata,
            registration,
        )?);
        Ok(())
    }
}

struct MetadataOnlyBinary;

impl BinaryImage for MetadataOnlyBinary {
    fn format(&self) -> BinaryFormat {
        BinaryFormat::Elf64
    }
    fn architecture(&self) -> Architecture {
        Architecture::Unknown
    }
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }
    fn kind(&self) -> BinaryKind {
        BinaryKind::Unknown
    }
    fn file_size(&self) -> u64 {
        0
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
        &[]
    }
    fn relative_relocations(&self) -> &[RelativeRelocation] {
        &[]
    }
    fn is_stripped(&self) -> bool {
        true
    }
    fn image_base(&self) -> u64 {
        0
    }
    fn virtual_to_offset(&self, _address: u64) -> Option<u64> {
        None
    }
    fn offset_to_virtual(&self, _offset: u64) -> Option<u64> {
        None
    }
    fn read_virtual(&self, _address: u64, _size: usize) -> Result<&[u8]> {
        Err(Error::AddressTranslationFailed)
    }
}
