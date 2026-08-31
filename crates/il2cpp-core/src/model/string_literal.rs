use serde::{Deserialize, Serialize};

/// Stable index into managed IL2CPP string-literal records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StringLiteralId(pub usize);

/// Decoded managed-code literal, distinct from metadata identifier strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringLiteral {
    pub id: StringLiteralId,
    pub value: String,
    pub metadata_index: usize,
    pub data_index: u32,
    pub byte_length: u32,
    pub metadata_file_offset: Option<u64>,
    pub valid_utf8: bool,
}

impl StringLiteral {
    pub fn escaped(&self) -> String {
        self.value.escape_default().to_string()
    }
    pub fn is_url(&self) -> bool {
        ["http://", "https://", "ws://", "wss://"]
            .iter()
            .any(|prefix| self.value.starts_with(prefix))
    }
}
