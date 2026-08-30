use crate::Result;
use crate::binary::BinaryImage;
use crate::metadata::Metadata;

use super::{Registration, RegistrationInfo, RegistrationResolver};

/// Registration strategy using executable patterns and metadata counts.
#[derive(Debug, Default)]
pub struct HeuristicResolver;

impl RegistrationResolver for HeuristicResolver {
    fn resolve(&self, binary: &dyn BinaryImage, metadata: &Metadata) -> Result<Registration> {
        Ok(RegistrationInfo::discover(binary, metadata)?.registration)
    }
}
