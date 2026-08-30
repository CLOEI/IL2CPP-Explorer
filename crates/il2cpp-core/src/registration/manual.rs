use crate::Result;
use crate::binary::BinaryImage;
use crate::metadata::Metadata;

use super::{Registration, RegistrationResolver};

/// Registration strategy using addresses supplied by a caller.
#[derive(Debug, Clone, Copy)]
pub struct ManualResolver {
    registration: Registration,
}

impl ManualResolver {
    /// Creates a resolver that returns the supplied addresses unchanged.
    pub fn new(registration: Registration) -> Self {
        Self { registration }
    }
}

impl RegistrationResolver for ManualResolver {
    fn resolve(&self, _binary: &dyn BinaryImage, _metadata: &Metadata) -> Result<Registration> {
        Ok(self.registration)
    }
}
