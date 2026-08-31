use thiserror::Error;

/// Errors produced while loading or analyzing IL2CPP data.
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid IL2CPP metadata")]
    InvalidMetadata,
    #[error("metadata read at offset {offset:#x} with length {length:#x} is out of bounds")]
    MetadataOutOfBounds { offset: usize, length: usize },
    #[error("invalid metadata table {0}")]
    InvalidMetadataTable(&'static str),
    #[error("invalid metadata string index {0}")]
    InvalidMetadataString(u32),
    #[error("invalid string literal record {0}")]
    InvalidStringLiteral(usize),
    #[error("unsupported IL2CPP metadata version {0}")]
    UnsupportedMetadataVersion(u32),
    #[error("invalid or unsupported executable binary")]
    InvalidBinary,
    #[error("unsupported executable architecture")]
    UnsupportedArchitecture,
    #[error("could not translate binary address")]
    AddressTranslationFailed,
    #[error("IL2CPP registration structures were not found")]
    RegistrationNotFound,
    #[error("invalid IL2CPP runtime type index {0}")]
    InvalidTypeIndex(usize),
    #[error("recursive IL2CPP runtime type at {0:#x}")]
    RecursiveTypeReference(u64),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Result type used by `il2cpp-core`.
pub type Result<T> = std::result::Result<T, Error>;
