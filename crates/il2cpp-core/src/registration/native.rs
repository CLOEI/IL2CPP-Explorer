use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::binary::{Architecture, BinaryImage, Endianness};
use crate::metadata::Metadata;
use crate::model::MethodId;
use crate::{Error, Result};

use super::Registration;

const CODE_GEN_MODULE_SIZE: usize = 0x88;
const CODE_REGISTRATION_MODULE_COUNT_OFFSET: u64 = 0x78;
const CODE_REGISTRATION_MODULES_OFFSET: u64 = 0x80;
const METADATA_REGISTRATION_SIZE: usize = 0x80;
const METHOD_TOKEN_TYPE: u32 = 0x0600_0000;
const TOKEN_TYPE_MASK: u32 = 0xff00_0000;
const TOKEN_ROW_MASK: u32 = 0x00ff_ffff;

/// Validated native module entry from an IL2CPP code registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeGenModule {
    pub address: u64,
    pub name: String,
    pub method_pointer_count: u32,
    pub method_pointers: Option<u64>,
}

/// Registration roots and the validated module array they reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationInfo {
    pub registration: Registration,
    pub codegen_modules: u64,
    pub modules: Vec<CodeGenModule>,
}

/// One metadata method resolved to its native code pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodAddress {
    pub method: MethodId,
    pub module: String,
    pub pointer_index: u32,
    pub virtual_address: u64,
    pub relative_address: u64,
    pub file_offset: u64,
}

/// Address-keyed native method mappings built once for fast call resolution.
#[derive(Debug, Clone, Default)]
pub struct NativeMethodIndex {
    by_address: BTreeMap<u64, Vec<MethodId>>,
    by_method: Vec<Option<MethodAddress>>,
}

impl NativeMethodIndex {
    /// Resolves every metadata method and indexes all generated native addresses.
    pub fn build(
        binary: &dyn BinaryImage,
        metadata: &Metadata,
        registration: &RegistrationInfo,
    ) -> Result<Self> {
        let mut addresses = Vec::new();
        for method in &metadata.methods {
            if let Some(address) = registration.resolve_method(binary, metadata, method.id)? {
                addresses.push(address);
            }
        }
        Ok(Self::from_addresses(metadata.methods.len(), addresses))
    }

    /// Builds an index from already-resolved addresses.
    pub fn from_addresses(
        method_count: usize,
        addresses: impl IntoIterator<Item = MethodAddress>,
    ) -> Self {
        let mut index = Self {
            by_address: BTreeMap::new(),
            by_method: vec![None; method_count],
        };
        for address in addresses {
            let method = address.method;
            if method.0 >= index.by_method.len() {
                index.by_method.resize(method.0 + 1, None);
            }
            index
                .by_address
                .entry(address.virtual_address)
                .or_default()
                .push(method);
            index.by_method[method.0] = Some(address);
        }
        index
    }

    /// Returns one method starting exactly at an address.
    pub fn method_at_address(&self, address: u64) -> Option<MethodId> {
        self.methods_at_address(address).first().copied()
    }

    /// Returns every method sharing an exact native start address.
    pub fn methods_at_address(&self, address: u64) -> &[MethodId] {
        self.by_address
            .get(&address)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Returns one method's resolved address.
    pub fn address_of(&self, method: MethodId) -> Option<&MethodAddress> {
        self.by_method.get(method.0)?.as_ref()
    }

    /// Returns the next distinct known native method start.
    pub fn next_address_after(&self, address: u64) -> Option<u64> {
        self.by_address
            .range((
                std::ops::Bound::Excluded(address),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .map(|(address, _)| *address)
    }

    /// Returns the number of metadata methods with native mappings.
    pub fn mapped_method_count(&self) -> usize {
        self.by_method.iter().flatten().count()
    }
}

impl RegistrationInfo {
    /// Discovers and validates Unity 2022.3-style registration structures.
    pub fn discover(binary: &dyn BinaryImage, metadata: &Metadata) -> Result<Self> {
        if binary.architecture() != Architecture::Arm64 || binary.endianness() != Endianness::Little
        {
            return Err(Error::UnsupportedArchitecture);
        }

        let expected_counts = expected_method_counts(metadata)?;
        let modules_by_address = discover_modules(binary, &expected_counts)?;
        let (codegen_modules, modules) =
            discover_module_array(binary, metadata.images.len(), &modules_by_address)?;
        let code_registration = discover_code_registration(binary, codegen_modules, modules.len())?;
        let metadata_registration = discover_metadata_registration(binary, metadata);

        Ok(Self {
            registration: Registration {
                code_registration: Some(code_registration),
                metadata_registration,
            },
            codegen_modules,
            modules,
        })
    }

    /// Resolves a metadata method through its image's CodeGenModule.
    pub fn resolve_method(
        &self,
        binary: &dyn BinaryImage,
        metadata: &Metadata,
        method_id: MethodId,
    ) -> Result<Option<MethodAddress>> {
        let method = metadata
            .methods
            .get(method_id.0)
            .ok_or(Error::InvalidMetadataTable("method index"))?;
        let pointer_index = match method_pointer_index(method.token) {
            Some(index) => index,
            None => return Ok(None),
        };
        let declaring_type = metadata
            .types
            .get(method.declaring_type.0)
            .ok_or(Error::InvalidMetadataTable("method declaring type"))?;
        let image = metadata
            .images
            .get(declaring_type.image.0)
            .ok_or(Error::InvalidMetadataTable("method image"))?;
        let module = self
            .modules
            .iter()
            .find(|module| module.name == image.name)
            .ok_or(Error::RegistrationNotFound)?;
        if pointer_index >= module.method_pointer_count {
            return Err(Error::InvalidBinary);
        }
        let Some(method_pointers) = module.method_pointers else {
            return Ok(None);
        };
        let slot = method_pointers
            .checked_add(u64::from(pointer_index) * 8)
            .ok_or(Error::AddressTranslationFailed)?;
        let virtual_address = relocated_pointer(binary, slot)?;
        if virtual_address == 0 {
            return Ok(None);
        }
        if !is_executable(binary, virtual_address) {
            return Err(Error::InvalidBinary);
        }
        let relative_address = virtual_address
            .checked_sub(binary.image_base())
            .ok_or(Error::AddressTranslationFailed)?;
        let file_offset = binary
            .virtual_to_offset(virtual_address)
            .ok_or(Error::AddressTranslationFailed)?;

        Ok(Some(MethodAddress {
            method: method_id,
            module: module.name.clone(),
            pointer_index,
            virtual_address,
            relative_address,
            file_offset,
        }))
    }
}

fn expected_method_counts(metadata: &Metadata) -> Result<HashMap<&str, u32>> {
    let mut counts = metadata
        .images
        .iter()
        .map(|image| (image.name.as_str(), 0))
        .collect::<HashMap<_, _>>();
    for method in &metadata.methods {
        let declaring_type = metadata
            .types
            .get(method.declaring_type.0)
            .ok_or(Error::InvalidMetadataTable("method declaring type"))?;
        let image = metadata
            .images
            .get(declaring_type.image.0)
            .ok_or(Error::InvalidMetadataTable("method image"))?;
        let row = method.token & TOKEN_ROW_MASK;
        if method.token & TOKEN_TYPE_MASK != METHOD_TOKEN_TYPE || row == 0 {
            return Err(Error::InvalidMetadataTable("method token"));
        }
        counts
            .entry(image.name.as_str())
            .and_modify(|count| *count = (*count).max(row));
    }
    Ok(counts)
}

fn discover_modules(
    binary: &dyn BinaryImage,
    expected_counts: &HashMap<&str, u32>,
) -> Result<HashMap<u64, CodeGenModule>> {
    let mut modules = HashMap::new();
    let mut names = HashSet::new();
    let max_name_length = expected_counts
        .keys()
        .map(|name| name.len())
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    for relocation in binary.relative_relocations() {
        let Ok(name_address) = u64::try_from(relocation.addend) else {
            continue;
        };
        let Some(name) = read_c_string(binary, name_address, max_name_length) else {
            continue;
        };
        let Some(&expected_count) = expected_counts.get(name) else {
            continue;
        };
        let Some(module) = parse_module(binary, relocation.address, name, expected_count) else {
            continue;
        };
        if !names.insert(module.name.clone()) {
            return Err(Error::RegistrationNotFound);
        }
        modules.insert(module.address, module);
    }

    if modules.len() != expected_counts.len() {
        return Err(Error::RegistrationNotFound);
    }
    Ok(modules)
}

fn parse_module(
    binary: &dyn BinaryImage,
    address: u64,
    name: &str,
    expected_count: u32,
) -> Option<CodeGenModule> {
    if address % 8 != 0 || binary.read_virtual(address, CODE_GEN_MODULE_SIZE).is_err() {
        return None;
    }
    let method_pointer_count = read_u32(binary, address.checked_add(8)?).ok()?;
    if method_pointer_count != expected_count {
        return None;
    }
    let method_pointers = match relocated_pointer(binary, address.checked_add(0x10)?).ok()? {
        0 if method_pointer_count == 0 => None,
        0 => return None,
        pointer => {
            let table_size = usize::try_from(method_pointer_count).ok()?.checked_mul(8)?;
            if binary.read_virtual(pointer, table_size).is_err() {
                return None;
            }
            Some(pointer)
        }
    };

    Some(CodeGenModule {
        address,
        name: name.to_owned(),
        method_pointer_count,
        method_pointers,
    })
}

fn discover_module_array(
    binary: &dyn BinaryImage,
    expected_count: usize,
    modules: &HashMap<u64, CodeGenModule>,
) -> Result<(u64, Vec<CodeGenModule>)> {
    let module_addresses = modules.keys().copied().collect::<HashSet<_>>();
    let mut candidates = Vec::new();

    for relocation in binary.relative_relocations() {
        let Ok(first_module) = u64::try_from(relocation.addend) else {
            continue;
        };
        if !module_addresses.contains(&first_module)
            || relocation
                .address
                .checked_sub(8)
                .and_then(|previous| relocation_at(binary, previous))
                .and_then(|previous| u64::try_from(previous.addend).ok())
                .is_some_and(|address| module_addresses.contains(&address))
        {
            continue;
        }

        let mut ordered = Vec::with_capacity(expected_count);
        let mut seen = HashSet::new();
        for index in 0..expected_count {
            let Some(slot) = relocation.address.checked_add(index as u64 * 8) else {
                break;
            };
            let Ok(module_address) = relocated_pointer(binary, slot) else {
                break;
            };
            let Some(module) = modules.get(&module_address) else {
                break;
            };
            if !seen.insert(module_address) {
                break;
            }
            ordered.push(module.clone());
        }
        if ordered.len() == expected_count {
            candidates.push((relocation.address, ordered));
        }
    }

    if candidates.len() == 1 {
        Ok(candidates.remove(0))
    } else {
        Err(Error::RegistrationNotFound)
    }
}

fn discover_code_registration(
    binary: &dyn BinaryImage,
    module_array: u64,
    module_count: usize,
) -> Result<u64> {
    let mut candidates = binary
        .relative_relocations()
        .iter()
        .filter_map(|relocation| {
            (u64::try_from(relocation.addend).ok() == Some(module_array))
                .then(|| {
                    relocation
                        .address
                        .checked_sub(CODE_REGISTRATION_MODULES_OFFSET)
                })
                .flatten()
        })
        .filter(|address| {
            read_u32(binary, *address + CODE_REGISTRATION_MODULE_COUNT_OFFSET)
                .is_ok_and(|count| count == module_count as u32)
                && binary.read_virtual(*address, CODE_GEN_MODULE_SIZE).is_ok()
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.dedup();

    if candidates.len() == 1 {
        Ok(candidates[0])
    } else {
        Err(Error::RegistrationNotFound)
    }
}

fn discover_metadata_registration(binary: &dyn BinaryImage, metadata: &Metadata) -> Option<u64> {
    let type_count = u32::try_from(metadata.types.len()).ok()?;
    let mut candidates = binary
        .relative_relocations()
        .iter()
        .filter_map(|relocation| relocation.address.checked_sub(0x58))
        .filter(|base| {
            binary
                .read_virtual(*base, METADATA_REGISTRATION_SIZE)
                .is_ok()
                && read_u32(binary, *base + 0x50).is_ok_and(|count| count == type_count)
                && read_u32(binary, *base + 0x60).is_ok_and(|count| count == type_count)
                && relocated_pointer(binary, *base + 0x58).is_ok_and(|pointer| pointer != 0)
                && relocated_pointer(binary, *base + 0x68).is_ok_and(|pointer| pointer != 0)
                && read_u32(binary, *base + 0x30).is_ok_and(|count| count >= type_count)
                && relocated_pointer(binary, *base + 0x38).is_ok_and(|pointer| pointer != 0)
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.dedup();
    (candidates.len() == 1).then(|| candidates[0])
}

fn method_pointer_index(token: u32) -> Option<u32> {
    let row = token & TOKEN_ROW_MASK;
    if token & TOKEN_TYPE_MASK == METHOD_TOKEN_TYPE {
        row.checked_sub(1)
    } else {
        None
    }
}

fn relocated_pointer(binary: &dyn BinaryImage, address: u64) -> Result<u64> {
    if let Some(relocation) = relocation_at(binary, address) {
        return u64::try_from(relocation.addend).map_err(|_| Error::InvalidBinary);
    }
    read_u64(binary, address)
}

fn relocation_at(
    binary: &dyn BinaryImage,
    address: u64,
) -> Option<&crate::binary::RelativeRelocation> {
    binary
        .relative_relocations()
        .binary_search_by_key(&address, |relocation| relocation.address)
        .ok()
        .map(|index| &binary.relative_relocations()[index])
}

fn read_u32(binary: &dyn BinaryImage, address: u64) -> Result<u32> {
    let bytes: [u8; 4] = binary
        .read_virtual(address, 4)?
        .try_into()
        .map_err(|_| Error::InvalidBinary)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(binary: &dyn BinaryImage, address: u64) -> Result<u64> {
    let bytes: [u8; 8] = binary
        .read_virtual(address, 8)?
        .try_into()
        .map_err(|_| Error::InvalidBinary)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_c_string(binary: &dyn BinaryImage, address: u64, max_length: usize) -> Option<&str> {
    let segment = binary.segments().iter().find(|segment| {
        segment.kind == "LOAD"
            && address
                .checked_sub(segment.virtual_address)
                .is_some_and(|relative| relative < segment.file_size)
    })?;
    let relative = address.checked_sub(segment.virtual_address)?;
    let available = usize::try_from(segment.file_size.checked_sub(relative)?).ok()?;
    let bytes = binary
        .read_virtual(address, max_length.min(available))
        .ok()?;
    let length = bytes.iter().position(|byte| *byte == 0)?;
    let value = std::str::from_utf8(&bytes[..length]).ok()?;
    (!value.is_empty()).then_some(value)
}

fn is_executable(binary: &dyn BinaryImage, address: u64) -> bool {
    binary.segments().iter().any(|segment| {
        segment.kind == "LOAD"
            && segment.permissions.execute
            && address
                .checked_sub(segment.virtual_address)
                .is_some_and(|relative| relative < segment.virtual_size)
    })
}

#[cfg(test)]
mod tests {
    use super::{MethodAddress, NativeMethodIndex, method_pointer_index};
    use crate::model::MethodId;

    #[test]
    fn method_tokens_map_to_zero_based_module_slots() {
        assert_eq!(method_pointer_index(0x0600_0001), Some(0));
        assert_eq!(method_pointer_index(0x0600_0042), Some(0x41));
        assert_eq!(method_pointer_index(0x0200_0001), None);
        assert_eq!(method_pointer_index(0x0600_0000), None);
    }

    #[test]
    fn native_method_index_supports_exact_and_next_lookups() {
        let address = |method, virtual_address| MethodAddress {
            method: MethodId(method),
            module: "Test.dll".to_owned(),
            pointer_index: method as u32,
            virtual_address,
            relative_address: virtual_address - 0x1000,
            file_offset: virtual_address - 0x1000,
        };
        let index = NativeMethodIndex::from_addresses(
            4,
            [address(0, 0x1100), address(1, 0x1180), address(2, 0x1180)],
        );

        assert_eq!(index.method_at_address(0x1100), Some(MethodId(0)));
        assert_eq!(
            index.methods_at_address(0x1180),
            &[MethodId(1), MethodId(2)]
        );
        assert_eq!(
            index.address_of(MethodId(2)).unwrap().virtual_address,
            0x1180
        );
        assert_eq!(index.address_of(MethodId(3)), None);
        assert_eq!(index.next_address_after(0x1100), Some(0x1180));
        assert_eq!(index.next_address_after(0x1180), None);
        assert_eq!(index.mapped_method_count(), 3);
    }
}
