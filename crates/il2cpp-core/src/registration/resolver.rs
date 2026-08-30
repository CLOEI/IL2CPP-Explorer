use crate::Result;
use crate::binary::BinaryImage;
use crate::metadata::Metadata;

use super::Registration;

/// Strategy for locating IL2CPP registration structures in a binary.
pub trait RegistrationResolver {
    fn resolve(&self, binary: &dyn BinaryImage, metadata: &Metadata) -> Result<Registration>;
}
