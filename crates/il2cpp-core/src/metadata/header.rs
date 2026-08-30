use std::fmt;

use serde::{Deserialize, Serialize};

/// Sanity value at the start of an unprotected IL2CPP metadata file.
pub const METADATA_SANITY: u32 = 0xFAB1_1BAF;

/// Supported metadata format families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetadataVersion {
    V31,
    Unknown(u32),
}

impl MetadataVersion {
    /// Returns the integer stored in the metadata header.
    pub const fn raw(self) -> u32 {
        match self {
            Self::V31 => 31,
            Self::Unknown(version) => version,
        }
    }
}

impl fmt::Display for MetadataVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw().fmt(formatter)
    }
}

/// Byte range occupied by one metadata table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetadataTable {
    pub offset: usize,
    pub byte_count: usize,
}

impl MetadataTable {
    pub(crate) fn from_raw(offset: u32, byte_count: u32) -> Self {
        Self {
            offset: offset as usize,
            byte_count: byte_count as usize,
        }
    }

    /// Validates table bounds and returns its exclusive end offset.
    pub fn validate(&self, file_size: usize) -> crate::Result<usize> {
        let end = self
            .offset
            .checked_add(self.byte_count)
            .ok_or(crate::Error::InvalidMetadata)?;
        if self.offset > file_size || end > file_size {
            return Err(crate::Error::MetadataOutOfBounds {
                offset: self.offset,
                length: self.byte_count,
            });
        }
        Ok(end)
    }

    /// Validates table bounds and record alignment, then returns record count.
    pub fn entry_count(&self, record_size: usize, file_size: usize) -> crate::Result<usize> {
        self.validate(file_size)?;
        if record_size == 0 || self.byte_count % record_size != 0 {
            return Err(crate::Error::InvalidMetadata);
        }
        Ok(self.byte_count / record_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn table_validation_rejects_overflow_and_bad_record_size() {
        let overflowing = MetadataTable {
            offset: usize::MAX,
            byte_count: 2,
        };
        assert!(matches!(
            overflowing.validate(usize::MAX),
            Err(Error::InvalidMetadata)
        ));

        let misaligned = MetadataTable {
            offset: 4,
            byte_count: 7,
        };
        assert!(matches!(
            misaligned.entry_count(4, 16),
            Err(Error::InvalidMetadata)
        ));
    }
}

/// Complete version 31 IL2CPP metadata header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataHeader {
    pub sanity: u32,
    pub version: u32,
    pub string_literals: MetadataTable,
    pub string_literal_data: MetadataTable,
    pub strings: MetadataTable,
    pub events: MetadataTable,
    pub properties: MetadataTable,
    pub methods: MetadataTable,
    pub parameter_default_values: MetadataTable,
    pub field_default_values: MetadataTable,
    pub default_value_data: MetadataTable,
    pub field_marshaled_sizes: MetadataTable,
    pub parameters: MetadataTable,
    pub fields: MetadataTable,
    pub generic_parameters: MetadataTable,
    pub generic_parameter_constraints: MetadataTable,
    pub generic_containers: MetadataTable,
    pub nested_types: MetadataTable,
    pub interfaces: MetadataTable,
    pub vtable_methods: MetadataTable,
    pub interface_offsets: MetadataTable,
    pub type_definitions: MetadataTable,
    pub images: MetadataTable,
    pub assemblies: MetadataTable,
    pub field_refs: MetadataTable,
    pub referenced_assemblies: MetadataTable,
    pub attribute_data: MetadataTable,
    pub attribute_data_ranges: MetadataTable,
    pub unresolved_virtual_call_parameter_types: MetadataTable,
    pub unresolved_virtual_call_parameter_ranges: MetadataTable,
    pub windows_runtime_type_names: MetadataTable,
    pub windows_runtime_strings: MetadataTable,
    pub exported_type_definitions: MetadataTable,
}

impl MetadataHeader {
    /// Returns all named metadata tables in header order.
    pub fn tables(&self) -> [(&'static str, MetadataTable); 31] {
        [
            ("String Literals", self.string_literals),
            ("String Literal Data", self.string_literal_data),
            ("String Data", self.strings),
            ("Events", self.events),
            ("Properties", self.properties),
            ("Methods", self.methods),
            ("Parameter Default Values", self.parameter_default_values),
            ("Field Default Values", self.field_default_values),
            ("Default Value Data", self.default_value_data),
            ("Field Marshaled Sizes", self.field_marshaled_sizes),
            ("Parameters", self.parameters),
            ("Fields", self.fields),
            ("Generic Parameters", self.generic_parameters),
            (
                "Generic Parameter Constraints",
                self.generic_parameter_constraints,
            ),
            ("Generic Containers", self.generic_containers),
            ("Nested Types", self.nested_types),
            ("Interfaces", self.interfaces),
            ("VTable Methods", self.vtable_methods),
            ("Interface Offsets", self.interface_offsets),
            ("Type Definitions", self.type_definitions),
            ("Images", self.images),
            ("Assemblies", self.assemblies),
            ("Field References", self.field_refs),
            ("Referenced Assemblies", self.referenced_assemblies),
            ("Attribute Data", self.attribute_data),
            ("Attribute Data Ranges", self.attribute_data_ranges),
            (
                "Unresolved Virtual Call Parameter Types",
                self.unresolved_virtual_call_parameter_types,
            ),
            (
                "Unresolved Virtual Call Parameter Ranges",
                self.unresolved_virtual_call_parameter_ranges,
            ),
            (
                "Windows Runtime Type Names",
                self.windows_runtime_type_names,
            ),
            ("Windows Runtime Strings", self.windows_runtime_strings),
            ("Exported Type Definitions", self.exported_type_definitions),
        ]
    }
}
