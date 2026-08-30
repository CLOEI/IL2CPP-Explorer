use std::cell::RefCell;
use std::collections::HashSet;

use crate::binary::BinaryImage;
use crate::metadata::Metadata;
use crate::model::{Field, GenericParameterId, Method, ParameterId, TypeId, TypeIndex, TypeRef};
use crate::registration::RuntimeMetadata;
use crate::{Error, Result};

const MAX_TYPE_DEPTH: usize = 128;
const MAX_GENERIC_ARGUMENTS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedParameter {
    pub id: ParameterId,
    pub name: String,
    pub ty: TypeRef,
    pub position: usize,
    pub attributes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    pub return_type: TypeRef,
    pub return_attributes: u16,
    pub parameters: Vec<ResolvedParameter>,
    pub generic_parameters: Vec<GenericParameterId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSignature {
    pub ty: TypeRef,
    pub attributes: u16,
    pub offset: Option<i32>,
}

/// Resolves runtime `Il2CppType` records into stable [`TypeRef`] values.
pub struct TypeResolver<'a> {
    metadata: &'a Metadata,
    binary: Option<&'a dyn BinaryImage>,
    runtime: Option<&'a RuntimeMetadata>,
    resolved: Option<&'a [TypeRef]>,
    resolved_attributes: Option<&'a [u16]>,
    cache: RefCell<Vec<Option<TypeRef>>>,
}

impl<'a> TypeResolver<'a> {
    /// Creates a metadata-only resolver. Type indexes remain explicit unknowns.
    pub fn metadata_only(metadata: &'a Metadata) -> Self {
        Self {
            metadata,
            binary: None,
            runtime: None,
            resolved: None,
            resolved_attributes: None,
            cache: RefCell::new(Vec::new()),
        }
    }

    /// Creates a resolver backed by validated runtime metadata registration.
    pub fn with_runtime(
        metadata: &'a Metadata,
        binary: &'a dyn BinaryImage,
        runtime: &'a RuntimeMetadata,
    ) -> Self {
        Self {
            metadata,
            binary: Some(binary),
            runtime: Some(runtime),
            resolved: None,
            resolved_attributes: None,
            cache: RefCell::new(vec![None; runtime.type_count()]),
        }
    }

    /// Creates a resolver over an already-normalized type table.
    pub fn from_resolved(
        metadata: &'a Metadata,
        resolved: &'a [TypeRef],
        attributes: Option<&'a [u16]>,
    ) -> Result<Self> {
        if attributes.is_some_and(|attributes| attributes.len() != resolved.len()) {
            return Err(Error::InvalidMetadataTable("resolved type attributes"));
        }
        Ok(Self {
            metadata,
            binary: None,
            runtime: None,
            resolved: Some(resolved),
            resolved_attributes: attributes,
            cache: RefCell::new(Vec::new()),
        })
    }

    pub fn metadata(&self) -> &'a Metadata {
        self.metadata
    }

    pub fn has_runtime_types(&self) -> bool {
        self.runtime.is_some() || self.resolved.is_some()
    }

    pub fn resolve(&self, index: TypeIndex) -> Result<TypeRef> {
        if let Some(resolved) = self.resolved {
            return resolved
                .get(index.0)
                .cloned()
                .ok_or(Error::InvalidTypeIndex(index.0));
        }
        let Some(runtime) = self.runtime else {
            return Ok(TypeRef::Unknown {
                type_index: Some(index.0),
                raw_type: None,
            });
        };
        let Some(address) = runtime.type_address(index.0) else {
            return Err(Error::InvalidTypeIndex(index.0));
        };
        if let Some(resolved) = self.cache.borrow()[index.0].clone() {
            return Ok(resolved);
        }
        let resolved = self.resolve_address(address, &mut HashSet::new(), 0)?;
        self.cache.borrow_mut()[index.0] = Some(resolved.clone());
        Ok(resolved)
    }

    pub fn attributes(&self, index: TypeIndex) -> Result<u16> {
        if let Some(attributes) = self.resolved_attributes {
            return attributes
                .get(index.0)
                .copied()
                .ok_or(Error::InvalidTypeIndex(index.0));
        }
        if self.resolved.is_some() {
            return Ok(0);
        }
        let Some(runtime) = self.runtime else {
            return Ok(0);
        };
        let address = runtime
            .type_address(index.0)
            .ok_or(Error::InvalidTypeIndex(index.0))?;
        Ok((self.read_u32(checked_address_add(address, 8)?)? & 0xffff) as u16)
    }

    pub fn field_signature(&self, field: &Field) -> Result<FieldSignature> {
        Ok(FieldSignature {
            ty: self.resolve(field.field_type)?,
            attributes: self.attributes(field.field_type)?,
            offset: self
                .runtime
                .and_then(|runtime| runtime.field_offset(field.id)),
        })
    }

    pub fn method_signature(&self, method: &Method) -> Result<MethodSignature> {
        let parameters = method
            .parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let parameter = &self.metadata.parameters[parameter.0];
                Ok(ResolvedParameter {
                    id: parameter.id,
                    name: parameter.name.clone(),
                    ty: self.resolve(parameter.parameter_type)?,
                    position,
                    attributes: self.attributes(parameter.parameter_type)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let generic_parameters = method
            .generic_container_index
            .and_then(|index| self.metadata.generic_containers.get(index))
            .map(|container| container.parameters.clone())
            .unwrap_or_default();
        Ok(MethodSignature {
            return_type: self.resolve(method.return_type)?,
            return_attributes: self.attributes(method.return_type)?,
            parameters,
            generic_parameters,
        })
    }

    fn resolve_address(
        &self,
        address: u64,
        active: &mut HashSet<u64>,
        depth: usize,
    ) -> Result<TypeRef> {
        if depth >= MAX_TYPE_DEPTH || !active.insert(address) {
            return Err(Error::RecursiveTypeReference(address));
        }
        let data = self.read_pointer_or_value(address)?;
        let bits = self.read_u32(checked_address_add(address, 8)?)?;
        let raw_type = ((bits >> 16) & 0xff) as u8;
        let mut resolved = match raw_type {
            0x01 => TypeRef::Void,
            0x02 => TypeRef::Boolean,
            0x03 => TypeRef::Char,
            0x04 => TypeRef::I8,
            0x05 => TypeRef::U8,
            0x06 => TypeRef::I16,
            0x07 => TypeRef::U16,
            0x08 => TypeRef::I32,
            0x09 => TypeRef::U32,
            0x0a => TypeRef::I64,
            0x0b => TypeRef::U64,
            0x0c => TypeRef::F32,
            0x0d => TypeRef::F64,
            0x0e => TypeRef::String,
            0x0f => TypeRef::Pointer(Box::new(self.resolve_address(data, active, depth + 1)?)),
            0x10 => TypeRef::ByRef(Box::new(self.resolve_address(data, active, depth + 1)?)),
            0x11 | 0x12 | 0x55 => self.type_definition(data, raw_type)?,
            0x13 | 0x1e => self.generic_parameter(data, raw_type == 0x1e)?,
            0x14 => self.array_type(data, active, depth + 1)?,
            0x15 => self.generic_instance(data, active, depth + 1)?,
            0x16 => TypeRef::TypedReference,
            0x18 => TypeRef::IntPtr,
            0x19 => TypeRef::UIntPtr,
            0x1b => TypeRef::FunctionPointer,
            0x1c => TypeRef::Object,
            0x1d => TypeRef::Array {
                element: Box::new(self.resolve_address(data, active, depth + 1)?),
                rank: 1,
                zero_based: true,
            },
            _ => TypeRef::Unknown {
                type_index: None,
                raw_type: Some(raw_type),
            },
        };
        active.remove(&address);

        // Metadata v31 stores by-ref separately from the type code.
        if bits & (1 << 29) != 0 && !matches!(resolved, TypeRef::ByRef(_)) {
            resolved = TypeRef::ByRef(Box::new(resolved));
        }
        Ok(resolved)
    }

    fn type_definition(&self, data: u64, raw_type: u8) -> Result<TypeRef> {
        let index = usize::try_from(data).map_err(|_| Error::InvalidBinary)?;
        if index >= self.metadata.types.len() {
            return Ok(TypeRef::Unknown {
                type_index: None,
                raw_type: Some(raw_type),
            });
        }
        Ok(TypeRef::Type(TypeId(index)))
    }

    fn generic_parameter(&self, data: u64, method: bool) -> Result<TypeRef> {
        let index = usize::try_from(data).map_err(|_| Error::InvalidBinary)?;
        let parameter = self
            .metadata
            .generic_parameters
            .get(index)
            .ok_or(Error::InvalidBinary)?;
        Ok(TypeRef::GenericParameter {
            id: parameter.id,
            name: parameter.name.clone(),
            method,
        })
    }

    fn array_type(&self, address: u64, active: &mut HashSet<u64>, depth: usize) -> Result<TypeRef> {
        let element = self.read_pointer(address)?;
        let rank = usize::from(self.read_u8(checked_address_add(address, 8)?)?);
        if element == 0 || rank == 0 || rank > 32 {
            return Err(Error::InvalidBinary);
        }
        Ok(TypeRef::Array {
            element: Box::new(self.resolve_address(element, active, depth)?),
            rank,
            zero_based: false,
        })
    }

    fn generic_instance(
        &self,
        address: u64,
        active: &mut HashSet<u64>,
        depth: usize,
    ) -> Result<TypeRef> {
        let base = self.read_pointer(address)?;
        let instance = self.read_pointer(checked_address_add(address, 8)?)?;
        if base == 0 || instance == 0 {
            return Err(Error::InvalidBinary);
        }
        let argument_count =
            usize::try_from(self.read_u32(instance)?).map_err(|_| Error::InvalidBinary)?;
        if argument_count > MAX_GENERIC_ARGUMENTS {
            return Err(Error::InvalidBinary);
        }
        let arguments_pointer = self.read_pointer(checked_address_add(instance, 8)?)?;
        let mut arguments = Vec::with_capacity(argument_count);
        for index in 0..argument_count {
            let argument = self.read_pointer(
                arguments_pointer
                    .checked_add(index as u64 * 8)
                    .ok_or(Error::InvalidBinary)?,
            )?;
            arguments.push(self.resolve_address(argument, active, depth)?);
        }
        Ok(TypeRef::GenericInstance {
            base: Box::new(self.resolve_address(base, active, depth)?),
            arguments,
        })
    }

    fn binary(&self) -> Result<&dyn BinaryImage> {
        self.binary.ok_or(Error::RegistrationNotFound)
    }

    fn read_u8(&self, address: u64) -> Result<u8> {
        self.binary()?
            .read_virtual(address, 1)?
            .first()
            .copied()
            .ok_or(Error::InvalidBinary)
    }

    fn read_u32(&self, address: u64) -> Result<u32> {
        let bytes: [u8; 4] = self
            .binary()?
            .read_virtual(address, 4)?
            .try_into()
            .map_err(|_| Error::InvalidBinary)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&self, address: u64) -> Result<u64> {
        let bytes: [u8; 8] = self
            .binary()?
            .read_virtual(address, 8)?
            .try_into()
            .map_err(|_| Error::InvalidBinary)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_pointer_or_value(&self, address: u64) -> Result<u64> {
        let binary = self.binary()?;
        if let Ok(index) = binary
            .relative_relocations()
            .binary_search_by_key(&address, |relocation| relocation.address)
        {
            return u64::try_from(binary.relative_relocations()[index].addend)
                .map_err(|_| Error::InvalidBinary);
        }
        self.read_u64(address)
    }

    fn read_pointer(&self, address: u64) -> Result<u64> {
        self.read_pointer_or_value(address)
    }
}

fn checked_address_add(address: u64, offset: u64) -> Result<u64> {
    address.checked_add(offset).ok_or(Error::InvalidBinary)
}

#[cfg(test)]
mod tests {
    use crate::binary::{
        Architecture, BinaryFormat, BinaryKind, Endianness, Permissions, RelativeRelocation,
        SectionInfo, SegmentInfo,
    };
    use crate::metadata::{METADATA_SANITY, Metadata};
    use crate::registration::RuntimeMetadata;

    use super::*;

    #[test]
    fn resolves_primitive_and_composite_runtime_types() {
        let mut image = TestImage::new(0x400);
        image.write_type(0x1000, 0, 0x08, 0);
        image.write_type(0x1010, 0, 0x09, 0);
        image.write_type(0x1020, 0x1000, 0x0f, 0);
        image.write_type(0x1030, 0x1000, 0x1d, 0);
        image.write_type(0x1040, 0, 0x08, 1 << 29);
        image.write_type(0x1050, 0x1100, 0x14, 0);
        image.write_u64(0x1100, 0x1000);
        image.write_u8(0x1108, 2);
        image.write_type(0x1060, 0x1200, 0x15, 0);
        image.write_u64(0x1200, 0x1000);
        image.write_u64(0x1208, 0x1220);
        image.write_u32(0x1220, 1);
        image.write_u64(0x1228, 0x1240);
        image.write_u64(0x1240, 0x1010);
        image.write_type(0x1070, 0, 0x7f, 0);
        let metadata = empty_metadata();
        let runtime = RuntimeMetadata::for_tests(
            vec![
                0x1000, 0x1010, 0x1020, 0x1030, 0x1040, 0x1050, 0x1060, 0x1070,
            ],
            0,
        );
        let resolver = TypeResolver::with_runtime(&metadata, &image, &runtime);

        assert_eq!(resolver.resolve(TypeIndex(0)).unwrap(), TypeRef::I32);
        assert_eq!(resolver.resolve(TypeIndex(1)).unwrap(), TypeRef::U32);
        assert_eq!(
            resolver.resolve(TypeIndex(2)).unwrap(),
            TypeRef::Pointer(Box::new(TypeRef::I32))
        );
        assert_eq!(
            resolver.resolve(TypeIndex(3)).unwrap(),
            TypeRef::Array {
                element: Box::new(TypeRef::I32),
                rank: 1,
                zero_based: true,
            }
        );
        assert_eq!(
            resolver.resolve(TypeIndex(4)).unwrap(),
            TypeRef::ByRef(Box::new(TypeRef::I32))
        );
        assert_eq!(
            resolver.resolve(TypeIndex(5)).unwrap(),
            TypeRef::Array {
                element: Box::new(TypeRef::I32),
                rank: 2,
                zero_based: false,
            }
        );
        assert_eq!(
            resolver.resolve(TypeIndex(6)).unwrap(),
            TypeRef::GenericInstance {
                base: Box::new(TypeRef::I32),
                arguments: vec![TypeRef::U32],
            }
        );
        assert_eq!(
            resolver.resolve(TypeIndex(7)).unwrap(),
            TypeRef::Unknown {
                type_index: None,
                raw_type: Some(0x7f),
            }
        );
        assert!(matches!(
            resolver.resolve(TypeIndex(99)),
            Err(Error::InvalidTypeIndex(99))
        ));
    }

    #[test]
    fn rejects_recursive_malformed_runtime_type() {
        let mut image = TestImage::new(0x100);
        image.write_type(0x1000, 0x1000, 0x0f, 0);
        let metadata = empty_metadata();
        let runtime = RuntimeMetadata::for_tests(vec![0x1000], 0);
        let resolver = TypeResolver::with_runtime(&metadata, &image, &runtime);

        assert!(matches!(
            resolver.resolve(TypeIndex(0)),
            Err(Error::RecursiveTypeReference(0x1000))
        ));
    }

    fn empty_metadata() -> Metadata {
        let mut bytes = vec![0; 256];
        bytes[0..4].copy_from_slice(&METADATA_SANITY.to_le_bytes());
        bytes[4..8].copy_from_slice(&31_u32.to_le_bytes());
        Metadata::from_bytes(&bytes).unwrap()
    }

    struct TestImage {
        data: Vec<u8>,
        segments: Vec<SegmentInfo>,
    }

    impl TestImage {
        fn new(size: usize) -> Self {
            Self {
                data: vec![0; size],
                segments: vec![SegmentInfo {
                    kind: "LOAD".to_owned(),
                    file_offset: 0,
                    file_size: size as u64,
                    virtual_address: 0x1000,
                    virtual_size: size as u64,
                    alignment: 0x1000,
                    permissions: Permissions {
                        read: true,
                        write: true,
                        execute: false,
                    },
                }],
            }
        }

        fn write_type(&mut self, address: u64, data: u64, type_code: u8, extra_bits: u32) {
            self.write_u64(address, data);
            self.write_u32(address + 8, u32::from(type_code) << 16 | extra_bits);
        }

        fn write_u8(&mut self, address: u64, value: u8) {
            let offset = (address - 0x1000) as usize;
            self.data[offset] = value;
        }

        fn write_u32(&mut self, address: u64, value: u32) {
            let offset = (address - 0x1000) as usize;
            self.data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }

        fn write_u64(&mut self, address: u64, value: u64) {
            let offset = (address - 0x1000) as usize;
            self.data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
    }

    impl BinaryImage for TestImage {
        fn format(&self) -> BinaryFormat {
            BinaryFormat::Elf64
        }
        fn architecture(&self) -> Architecture {
            Architecture::Arm64
        }
        fn endianness(&self) -> Endianness {
            Endianness::Little
        }
        fn kind(&self) -> BinaryKind {
            BinaryKind::SharedObject
        }
        fn file_size(&self) -> u64 {
            self.data.len() as u64
        }
        fn entry_point(&self) -> u64 {
            0
        }
        fn section_count(&self) -> usize {
            0
        }
        fn sections(&self) -> &[SectionInfo] {
            &[]
        }
        fn segments(&self) -> &[SegmentInfo] {
            &self.segments
        }
        fn relative_relocations(&self) -> &[RelativeRelocation] {
            &[]
        }
        fn is_stripped(&self) -> bool {
            true
        }
        fn image_base(&self) -> u64 {
            0x1000
        }
        fn virtual_to_offset(&self, address: u64) -> Option<u64> {
            address.checked_sub(0x1000)
        }
        fn offset_to_virtual(&self, offset: u64) -> Option<u64> {
            0x1000_u64.checked_add(offset)
        }
        fn read_virtual(&self, address: u64, size: usize) -> Result<&[u8]> {
            let start = usize::try_from(
                address
                    .checked_sub(0x1000)
                    .ok_or(Error::AddressTranslationFailed)?,
            )
            .map_err(|_| Error::AddressTranslationFailed)?;
            let end = start
                .checked_add(size)
                .ok_or(Error::AddressTranslationFailed)?;
            self.data
                .get(start..end)
                .ok_or(Error::AddressTranslationFailed)
        }
    }
}
