use serde::{Deserialize, Serialize};

/// Public change classification. Location movement is distinct from body changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiffStatus {
    Unchanged,
    Added,
    Removed,
    Changed,
    Moved,
}

impl DiffStatus {
    pub const fn marker(self) -> char {
        match self {
            Self::Unchanged => '=',
            Self::Added => '+',
            Self::Removed => '-',
            Self::Changed => '~',
            Self::Moved => '>',
        }
    }
    pub const fn is_changed(self) -> bool {
        !matches!(self, Self::Unchanged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn symbols_are_terminal_independent() {
        assert_eq!(DiffStatus::Added.marker(), '+');
        assert_eq!(DiffStatus::Removed.marker(), '-');
        assert_eq!(DiffStatus::Changed.marker(), '~');
        assert_eq!(DiffStatus::Moved.marker(), '>');
        assert_eq!(DiffStatus::Unchanged.marker(), '=');
    }
}
