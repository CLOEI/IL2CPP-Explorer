//! Data types for describing changes between normalized IL2CPP builds.
//!
//! Matching and comparison algorithms are intentionally not implemented yet.

/// Kind of change detected between two builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
    Moved,
}

/// A change and the normalized item it concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change<T> {
    pub kind: ChangeKind,
    pub item: T,
}
