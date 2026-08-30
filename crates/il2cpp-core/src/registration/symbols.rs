use crate::binary::BinaryImage;
use crate::metadata::Metadata;
use crate::{Error, Result};

use super::{Registration, RegistrationResolver};

/// Registration strategy using exported or debug symbols.
#[derive(Debug, Default)]
pub struct SymbolResolver;

impl RegistrationResolver for SymbolResolver {
    fn resolve(&self, _binary: &dyn BinaryImage, _metadata: &Metadata) -> Result<Registration> {
        Err(Error::RegistrationNotFound)
    }
}
