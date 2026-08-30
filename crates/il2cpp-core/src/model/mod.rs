//! Normalized IL2CPP domain models, independent of raw metadata layouts.

mod assembly;
mod field;
mod image;
mod method;
mod parameter;
mod property;
mod ty;

pub use assembly::{Assembly, AssemblyId};
pub use field::{Field, FieldId};
pub use image::{Image, ImageId};
pub use method::{Method, MethodId};
pub use parameter::{Parameter, ParameterId};
pub use property::{Property, PropertyId};
pub use ty::{TypeDefinition, TypeId, TypeIndex};
