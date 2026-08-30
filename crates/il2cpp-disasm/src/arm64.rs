use anyhow::{Context, Result};
use capstone::Endian;
use capstone::arch::DetailsArchInsn;
use capstone::arch::arm64::{Arm64Insn, Arm64OperandType};
use capstone::prelude::*;

use crate::{ControlFlow, Disassembler, Instruction};

/// AArch64 disassembler backed by Capstone.
pub struct Arm64Disassembler {
    capstone: Capstone,
}

impl Arm64Disassembler {
    pub fn new() -> Result<Self> {
        let capstone = Capstone::new()
            .arm64()
            .mode(arch::arm64::ArchMode::Arm)
            .endian(Endian::Little)
            .detail(true)
            .build()
            .context("failed to initialize Capstone AArch64 backend")?;
        Ok(Self { capstone })
    }

    fn control_flow(&self, instruction: &capstone::Insn<'_>) -> Result<ControlFlow> {
        let id = instruction.id().0;
        let mnemonic = instruction.mnemonic().unwrap_or_default();
        let target = || -> Result<Option<u64>> {
            let detail = self
                .capstone
                .insn_detail(instruction)
                .context("failed to read Capstone AArch64 instruction details")?;
            let architecture = detail.arch_detail();
            let arm64 = architecture
                .arm64()
                .context("Capstone returned non-AArch64 instruction details")?;
            Ok(arm64
                .operands()
                .filter_map(|operand| match operand.op_type {
                    Arm64OperandType::Imm(immediate) => u64::try_from(immediate).ok(),
                    _ => None,
                })
                .last())
        };

        if id == Arm64Insn::ARM64_INS_BL as u32 {
            Ok(ControlFlow::Call { target: target()? })
        } else if id == Arm64Insn::ARM64_INS_BLR as u32 {
            Ok(ControlFlow::Call { target: None })
        } else if id == Arm64Insn::ARM64_INS_RET as u32 {
            Ok(ControlFlow::Return)
        } else if id == Arm64Insn::ARM64_INS_CBZ as u32
            || id == Arm64Insn::ARM64_INS_CBNZ as u32
            || id == Arm64Insn::ARM64_INS_TBZ as u32
            || id == Arm64Insn::ARM64_INS_TBNZ as u32
            || (id == Arm64Insn::ARM64_INS_B as u32 && mnemonic.starts_with("b."))
        {
            Ok(ControlFlow::ConditionalBranch { target: target()? })
        } else if id == Arm64Insn::ARM64_INS_B as u32 {
            Ok(ControlFlow::Branch { target: target()? })
        } else if id == Arm64Insn::ARM64_INS_BR as u32 {
            Ok(ControlFlow::Branch { target: None })
        } else {
            Ok(ControlFlow::None)
        }
    }
}

impl Default for Arm64Disassembler {
    fn default() -> Self {
        Self::new().expect("Capstone AArch64 backend should initialize")
    }
}

impl Disassembler for Arm64Disassembler {
    fn disassemble(&self, code: &[u8], address: u64) -> Result<Vec<Instruction>> {
        let instructions = self
            .capstone
            .disasm_all(code, address)
            .context("Capstone failed to disassemble AArch64 bytes")?;
        instructions
            .iter()
            .map(|instruction| {
                Ok(Instruction {
                    address: instruction.address(),
                    size: u8::try_from(instruction.bytes().len())
                        .context("decoded instruction is too large")?,
                    bytes: instruction.bytes().to_vec(),
                    mnemonic: instruction.mnemonic().unwrap_or_default().to_owned(),
                    operands: instruction.op_str().unwrap_or_default().to_owned(),
                    control_flow: self.control_flow(instruction)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_valid_aarch64_and_ignores_truncated_tail() {
        let disassembler = Arm64Disassembler::new().unwrap();
        let instructions = disassembler
            .disassemble(
                &[
                    0xfd, 0x7b, 0xbf, 0xa9, 0xfd, 0x03, 0x00, 0x91, 0xaa, 0xbb, 0xcc,
                ],
                0x1000,
            )
            .unwrap();

        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0].address, 0x1000);
        assert_eq!(instructions[0].size, 4);
        assert_eq!(instructions[0].mnemonic, "stp");
        assert_eq!(instructions[1].mnemonic, "mov");
        assert!(
            disassembler
                .disassemble(&[0, 1, 2], 0x2000)
                .unwrap()
                .is_empty()
        );
        assert!(
            disassembler
                .disassemble(&[0xff, 0xff, 0xff, 0xff], 0x2000)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn normalizes_direct_calls_conditional_branches_and_returns() {
        let disassembler = Arm64Disassembler::new().unwrap();
        let instructions = disassembler
            .disassemble(
                &[
                    0x02, 0x00, 0x00, 0x94, // bl 0x1008
                    0x40, 0x00, 0x00, 0x54, // b.eq 0x100c
                    0x40, 0x00, 0x00, 0xb4, // cbz x0, 0x1010
                    0x40, 0x00, 0x00, 0x36, // tbz w0, #0, 0x1014
                    0xc0, 0x03, 0x5f, 0xd6, // ret
                ],
                0x1000,
            )
            .unwrap();

        assert_eq!(
            instructions[0].control_flow,
            ControlFlow::Call {
                target: Some(0x1008)
            }
        );
        for (instruction, target) in instructions[1..4].iter().zip([0x100c, 0x1010, 0x1014]) {
            assert_eq!(
                instruction.control_flow,
                ControlFlow::ConditionalBranch {
                    target: Some(target)
                }
            );
        }
        assert_eq!(instructions[4].control_flow, ControlFlow::Return);
    }

    #[test]
    fn marks_indirect_calls_and_branches_without_guessing_targets() {
        let disassembler = Arm64Disassembler::new().unwrap();
        let instructions = disassembler
            .disassemble(
                &[
                    0x00, 0x01, 0x3f, 0xd6, // blr x8
                    0x00, 0x01, 0x1f, 0xd6, // br x8
                ],
                0x1000,
            )
            .unwrap();

        assert_eq!(
            instructions[0].control_flow,
            ControlFlow::Call { target: None }
        );
        assert_eq!(
            instructions[1].control_flow,
            ControlFlow::Branch { target: None }
        );
    }
}
