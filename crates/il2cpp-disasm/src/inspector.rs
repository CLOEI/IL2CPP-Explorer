use anyhow::{Context, Result, bail};
use il2cpp_core::binary::BinaryImage;
use il2cpp_core::model::MethodId;
use il2cpp_core::registration::{MethodAddress, NativeMethodIndex};
use serde::{Deserialize, Serialize};

use crate::{
    ControlFlow, DirectCall, Disassembler, FunctionRange, FunctionRangeSource, Instruction,
};

pub const DEFAULT_DISASSEMBLY_BYTES: usize = 256;
const MAX_NEXT_METHOD_DISTANCE: u64 = 64 * 1024;

/// Native instructions and proven direct calls for one IL2CPP method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionInspection {
    pub method: MethodId,
    pub address: MethodAddress,
    pub range: FunctionRange,
    pub window_end: u64,
    pub instructions: Vec<Instruction>,
    pub direct_calls: Vec<DirectCall>,
}

/// Resolves safe native windows and decodes them through a backend abstraction.
pub struct FunctionInspector<'a> {
    binary: &'a dyn BinaryImage,
    methods: &'a NativeMethodIndex,
    disassembler: &'a dyn Disassembler,
    max_bytes: usize,
}

impl<'a> FunctionInspector<'a> {
    pub fn new(
        binary: &'a dyn BinaryImage,
        methods: &'a NativeMethodIndex,
        disassembler: &'a dyn Disassembler,
    ) -> Self {
        Self {
            binary,
            methods,
            disassembler,
            max_bytes: DEFAULT_DISASSEMBLY_BYTES,
        }
    }

    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    pub fn inspect(&self, method: MethodId) -> Result<FunctionInspection> {
        if self.max_bytes < 4 {
            bail!("AArch64 disassembly window must contain at least four bytes");
        }
        let address = self
            .methods
            .address_of(method)
            .with_context(|| format!("method {} has no generated native address", method.0))?
            .clone();
        let start = address.virtual_address;
        if start % 4 != 0 {
            bail!("method address is not four-byte aligned for AArch64");
        }
        let segment = self
            .binary
            .executable_segment(start)
            .context("method address is outside file-backed executable memory")?;
        let available = segment
            .file_size
            .min(segment.virtual_size)
            .checked_sub(start - segment.virtual_address)
            .context("invalid executable segment range")?;
        let limit = u64::try_from(self.max_bytes).context("disassembly window is too large")?;
        let next = self.methods.next_address_after(start).filter(|next| {
            next.checked_sub(segment.virtual_address)
                .is_some_and(|relative| {
                    relative < segment.file_size && relative < segment.virtual_size
                })
        });
        let (range, window_size) = select_window(start, next, available, limit)?;
        if window_size < 4 {
            bail!("fewer than four executable bytes remain at method address");
        }
        let instructions = disassemble_executable_window(
            self.binary,
            self.disassembler,
            start,
            usize::try_from(window_size).context("disassembly window is too large")?,
        )?;
        let direct_calls = collect_direct_calls(method, &instructions, self.methods);
        let window_end = start
            .checked_add(window_size)
            .context("disassembly window overflows address space")?;

        Ok(FunctionInspection {
            method,
            address,
            range,
            window_end,
            instructions,
            direct_calls,
        })
    }
}

/// Reads and disassembles one strictly checked executable window.
pub fn disassemble_executable_window(
    binary: &dyn BinaryImage,
    disassembler: &dyn Disassembler,
    address: u64,
    size: usize,
) -> Result<Vec<Instruction>> {
    let code = binary
        .read_executable(address, size)
        .context("failed to read executable disassembly window")?;
    disassembler.disassemble(code, address)
}

/// Collects only direct `BL` targets normalized by the disassembly backend.
pub fn collect_direct_calls(
    caller: MethodId,
    instructions: &[Instruction],
    methods: &NativeMethodIndex,
) -> Vec<DirectCall> {
    instructions
        .iter()
        .filter_map(|instruction| match instruction.control_flow {
            ControlFlow::Call {
                target: Some(target_address),
            } => Some(DirectCall {
                caller,
                call_address: instruction.address,
                target_address,
                callees: methods.methods_at_address(target_address).to_vec(),
            }),
            _ => None,
        })
        .collect()
}

fn select_window(
    start: u64,
    next: Option<u64>,
    available: u64,
    limit: u64,
) -> Result<(FunctionRange, u64)> {
    let distance = next.and_then(|next| next.checked_sub(start));
    if let (Some(end), Some(distance)) = (next, distance)
        && distance > 0
        && distance <= MAX_NEXT_METHOD_DISTANCE
        && distance <= available
    {
        return Ok((
            FunctionRange {
                start,
                end: Some(end),
                source: FunctionRangeSource::NextMethod,
            },
            distance.min(limit),
        ));
    }

    let window = available.min(limit);
    if window == 0 {
        bail!("no file-backed executable bytes remain at method address");
    }
    Ok((
        FunctionRange {
            start,
            end: None,
            source: FunctionRangeSource::Unknown,
        },
        window,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use il2cpp_core::binary::{
        Architecture, BinaryFormat, BinaryKind, Endianness, Permissions, RelativeRelocation,
        SectionInfo, SegmentInfo,
    };
    use il2cpp_core::{Error, Result as CoreResult};

    fn instruction(address: u64, control_flow: ControlFlow) -> Instruction {
        Instruction {
            address,
            size: 4,
            bytes: vec![0; 4],
            mnemonic: String::new(),
            operands: String::new(),
            control_flow,
        }
    }

    fn method_address(method: usize, address: u64) -> MethodAddress {
        MethodAddress {
            method: MethodId(method),
            module: "Test.dll".to_owned(),
            pointer_index: method as u32,
            virtual_address: address,
            relative_address: address,
            file_offset: address,
        }
    }

    #[test]
    fn next_method_bounds_reasonable_function_window() {
        let (range, size) = select_window(0x1000, Some(0x1080), 0x1000, 256).unwrap();
        assert_eq!(range.end, Some(0x1080));
        assert_eq!(range.source, FunctionRangeSource::NextMethod);
        assert_eq!(size, 0x80);
    }

    #[test]
    fn maximum_limit_bounds_unknown_function_window() {
        let (range, size) = select_window(0x1000, Some(0x11001), 0x20000, 256).unwrap();
        assert_eq!(range.end, None);
        assert_eq!(range.source, FunctionRangeSource::Unknown);
        assert_eq!(size, 256);

        let (_, segment_limited_size) = select_window(0x1000, None, 96, 256).unwrap();
        assert_eq!(segment_limited_size, 96);
    }

    #[test]
    fn known_boundary_is_independent_from_smaller_read_limit() {
        let (range, size) = select_window(0x1000, Some(0x1400), 0x2000, 128).unwrap();
        assert_eq!(range.end, Some(0x1400));
        assert_eq!(range.source, FunctionRangeSource::NextMethod);
        assert_eq!(size, 128);
    }

    #[test]
    fn resolves_and_preserves_direct_call_targets() {
        let methods = NativeMethodIndex::from_addresses(2, [method_address(1, 0x2000)]);
        let instructions = [
            instruction(
                0x1000,
                ControlFlow::Call {
                    target: Some(0x2000),
                },
            ),
            instruction(
                0x1004,
                ControlFlow::Call {
                    target: Some(0x3000),
                },
            ),
            instruction(0x1008, ControlFlow::Call { target: None }),
        ];
        let calls = collect_direct_calls(MethodId(0), &instructions, &methods);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].callees, vec![MethodId(1)]);
        assert!(calls[1].callees.is_empty());
        assert_eq!(calls[1].target_address, 0x3000);
    }

    #[test]
    fn inspector_resolves_method_to_checked_next_method_bytes() {
        let image = TestImage::new();
        let methods = NativeMethodIndex::from_addresses(
            2,
            [method_address(0, 0x1004), method_address(1, 0x100c)],
        );
        let inspection = FunctionInspector::new(&image, &methods, &TestDisassembler)
            .inspect(MethodId(0))
            .unwrap();

        assert_eq!(inspection.range.end, Some(0x100c));
        assert_eq!(inspection.range.source, FunctionRangeSource::NextMethod);
        assert_eq!(inspection.window_end, 0x100c);
        assert_eq!(inspection.instructions.len(), 2);
        assert_eq!(inspection.instructions[0].bytes, vec![4, 5, 6, 7]);
        assert_eq!(inspection.instructions[1].bytes, vec![8, 9, 10, 11]);
    }

    struct TestDisassembler;

    impl Disassembler for TestDisassembler {
        fn disassemble(&self, code: &[u8], address: u64) -> Result<Vec<Instruction>> {
            Ok(code
                .chunks_exact(4)
                .enumerate()
                .map(|(index, bytes)| Instruction {
                    address: address + index as u64 * 4,
                    size: 4,
                    bytes: bytes.to_vec(),
                    mnemonic: "test".to_owned(),
                    operands: String::new(),
                    control_flow: ControlFlow::None,
                })
                .collect())
        }
    }

    struct TestImage {
        data: Vec<u8>,
        segments: Vec<SegmentInfo>,
    }

    impl TestImage {
        fn new() -> Self {
            Self {
                data: (0_u8..32).collect(),
                segments: vec![SegmentInfo {
                    kind: "LOAD".to_owned(),
                    file_offset: 0,
                    file_size: 32,
                    virtual_address: 0x1000,
                    virtual_size: 32,
                    alignment: 0x1000,
                    permissions: Permissions {
                        read: true,
                        write: false,
                        execute: true,
                    },
                }],
            }
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
        fn read_virtual(&self, address: u64, size: usize) -> CoreResult<&[u8]> {
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
