//! Deterministic, managed-identity-first IL2CPP build comparison.

mod diff;
mod identity;
mod native;
mod report;
mod status;

pub use diff::{DiffEngine, DiffOptions};
pub use identity::{MethodIdentity, MethodMatchKey, TypeIdentity, TypeIdentityRef};
pub use native::{
    FunctionFingerprint, NativeDiff, NativeInstruction, NormalizedInstruction, NormalizedOperand,
};
pub use report::{
    AssemblyDiff, DiffSummary, FieldDiff, MethodDiff, ProjectDiff, PropertyDiff, TypeDiff,
};
pub use status::DiffStatus;
