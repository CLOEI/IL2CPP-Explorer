use serde::{Deserialize, Serialize};

/// Native address represented in relative and virtual forms when known.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address {
    pub relative: Option<u64>,
    pub virtual_address: Option<u64>,
}
