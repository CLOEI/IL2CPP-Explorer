use crate::binary::BinaryImage;
use crate::metadata::Metadata;
use crate::{Error, Result};

use super::{Registration, RegistrationResolver};

/// Registration strategy using executable patterns and metadata counts.
#[derive(Debug, Default)]
pub struct HeuristicResolver;

impl RegistrationResolver for HeuristicResolver {
    fn resolve(&self, _binary: &dyn BinaryImage, _metadata: &Metadata) -> Result<Registration> {
        Err(Error::RegistrationNotFound)
    }
}
