//! Normalized IL2CPP domain models, independent of raw metadata layouts.

mod assembly;
mod field;
mod generic;
mod image;
mod method;
mod parameter;
mod property;
mod string_literal;
mod ty;
mod type_ref;

pub use assembly::{Assembly, AssemblyId};
pub use field::{Field, FieldId};
pub use generic::{
    GenericContainer, GenericContainerId, GenericOwner, GenericParameter, GenericParameterId,
};
pub use image::{Image, ImageId};
pub use method::{Method, MethodId};
pub use parameter::{Parameter, ParameterId};
pub use property::{Property, PropertyId};
pub use string_literal::{StringLiteral, StringLiteralId};
pub use ty::{TypeDefinition, TypeId, TypeIndex, TypeKind};
pub use type_ref::TypeRef;
