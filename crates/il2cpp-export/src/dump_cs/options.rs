#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DumpCsOptions {
    pub include_addresses: bool,
    pub include_file_offsets: bool,
    pub include_tokens: bool,
    pub include_indices: bool,
    pub include_field_offsets: bool,
    pub fully_qualified_types: bool,
}

impl Default for DumpCsOptions {
    fn default() -> Self {
        Self {
            include_addresses: true,
            include_file_offsets: false,
            include_tokens: true,
            include_indices: false,
            include_field_offsets: true,
            fully_qualified_types: false,
        }
    }
}
