use std::collections::HashMap;

use il2cpp_core::analysis::Il2CppProject;
use il2cpp_core::model::MethodId;
use il2cpp_disasm::{Arm64Disassembler, ControlFlow, FunctionInspector, Instruction};
use serde::{Deserialize, Serialize};

use crate::MethodIdentity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedInstruction {
    pub mnemonic: String,
    pub operands: Vec<NormalizedOperand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum NormalizedOperand {
    Register(String),
    Immediate(i64),
    Memory(String),
    MethodTarget(String),
    RelativeTarget,
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionFingerprint {
    pub instruction_count: usize,
    pub hash: u64,
}

/// Original decoded text retained only for changed-function UI/JSON presentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeInstruction {
    pub address: u64,
    pub mnemonic: String,
    pub operands: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeDiff {
    pub old_instruction_count: Option<usize>,
    pub new_instruction_count: Option<usize>,
    pub old_fingerprint: Option<FunctionFingerprint>,
    pub new_fingerprint: Option<FunctionFingerprint>,
    pub equivalent: Option<bool>,
    pub similarity: Option<f32>,
    pub old_instructions: Option<Vec<NativeInstruction>>,
    pub new_instructions: Option<Vec<NativeInstruction>>,
}

pub(crate) fn fingerprint(
    project: &Il2CppProject,
    method: MethodId,
    identities: &[MethodIdentity],
) -> Option<(
    FunctionFingerprint,
    Vec<NormalizedInstruction>,
    Vec<NativeInstruction>,
)> {
    let methods = project.native_methods()?;
    let disassembler = Arm64Disassembler::new().ok()?;
    let inspection = FunctionInspector::new(project.binary(), methods, &disassembler)
        .with_max_bytes(2048)
        .inspect(method)
        .ok()?;
    let targets = inspection
        .instructions
        .iter()
        .enumerate()
        .map(|(index, instruction)| (instruction.address, index))
        .collect::<HashMap<_, _>>();
    let normalized = inspection
        .instructions
        .iter()
        .map(|instruction| normalize_instruction(instruction, &targets, methods, identities))
        .collect::<Vec<_>>();
    let fingerprint = FunctionFingerprint {
        instruction_count: normalized.len(),
        hash: stable_hash(&normalized),
    };
    let original = inspection
        .instructions
        .into_iter()
        .map(|item| NativeInstruction {
            address: item.address,
            mnemonic: item.mnemonic,
            operands: item.operands,
        })
        .collect();
    Some((fingerprint, normalized, original))
}

fn normalize_instruction(
    instruction: &Instruction,
    targets: &HashMap<u64, usize>,
    methods: &il2cpp_core::registration::NativeMethodIndex,
    identities: &[MethodIdentity],
) -> NormalizedInstruction {
    let control = match instruction.control_flow {
        ControlFlow::Call {
            target: Some(address),
        } => methods
            .method_at_address(address)
            .and_then(|id| identities.get(id.0))
            .map(|identity| NormalizedOperand::MethodTarget(identity.to_string()))
            .unwrap_or(NormalizedOperand::RelativeTarget),
        ControlFlow::Branch {
            target: Some(address),
        }
        | ControlFlow::ConditionalBranch {
            target: Some(address),
        } => targets
            .get(&address)
            .map_or(NormalizedOperand::RelativeTarget, |block| {
                NormalizedOperand::Unknown(format!("BLOCK_{block}"))
            }),
        _ => NormalizedOperand::Unknown(String::new()),
    };
    let has_target = matches!(
        instruction.control_flow,
        ControlFlow::Call { target: Some(_) }
            | ControlFlow::Branch { target: Some(_) }
            | ControlFlow::ConditionalBranch { target: Some(_) }
    );
    let mut operands = match instruction.control_flow {
        ControlFlow::Call { target: Some(_) } | ControlFlow::Branch { target: Some(_) } => {
            vec![control]
        }
        ControlFlow::ConditionalBranch { target: Some(_) } => {
            let prefix = instruction
                .operands
                .rsplit_once(',')
                .map_or("", |(prefix, _)| prefix);
            let mut values = parse_operands(prefix);
            values.push(control);
            values
        }
        _ => parse_operands(&instruction.operands),
    };
    if operands.is_empty() && !instruction.operands.is_empty() && !has_target {
        operands.push(NormalizedOperand::Unknown(instruction.operands.clone()));
    }
    NormalizedInstruction {
        mnemonic: instruction.mnemonic.to_ascii_lowercase(),
        operands,
    }
}

fn parse_operands(value: &str) -> Vec<NormalizedOperand> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.starts_with('x') || part.starts_with('w') || part == "sp" || part == "lr" {
                NormalizedOperand::Register(part.to_owned())
            } else if part.starts_with('[') {
                NormalizedOperand::Memory(part.to_owned())
            } else if let Some(number) = part.strip_prefix('#').and_then(parse_number) {
                NormalizedOperand::Immediate(number)
            } else {
                NormalizedOperand::Unknown(part.to_owned())
            }
        })
        .collect()
}

fn parse_number(value: &str) -> Option<i64> {
    value
        .strip_prefix("0x")
        .and_then(|v| i64::from_str_radix(v, 16).ok())
        .or_else(|| value.parse().ok())
}

fn stable_hash(items: &[NormalizedInstruction]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for item in items {
        hash_bytes(&mut hash, item.mnemonic.as_bytes());
        for operand in &item.operands {
            hash_bytes(&mut hash, format!("{operand:?}").as_bytes());
        }
    }
    hash
}
fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

pub(crate) fn native_diff(
    old: Option<(
        FunctionFingerprint,
        Vec<NormalizedInstruction>,
        Vec<NativeInstruction>,
    )>,
    new: Option<(
        FunctionFingerprint,
        Vec<NormalizedInstruction>,
        Vec<NativeInstruction>,
    )>,
    similarity: bool,
) -> Option<NativeDiff> {
    let (old_fingerprint, old_body, old_instructions) = old?;
    let (new_fingerprint, new_body, new_instructions) = new?;
    let equivalent = old_fingerprint == new_fingerprint;
    let score = similarity.then(|| sequence_similarity(&old_body, &new_body));
    Some(NativeDiff {
        old_instruction_count: Some(old_fingerprint.instruction_count),
        new_instruction_count: Some(new_fingerprint.instruction_count),
        old_fingerprint: Some(old_fingerprint),
        new_fingerprint: Some(new_fingerprint),
        equivalent: Some(equivalent),
        similarity: score,
        old_instructions: (!equivalent).then_some(old_instructions),
        new_instructions: (!equivalent).then_some(new_instructions),
    })
}

/// O(n), positional normalized-instruction overlap. Keeps 150k-method diff practical.
fn sequence_similarity(old: &[NormalizedInstruction], new: &[NormalizedInstruction]) -> f32 {
    let total = old.len().max(new.len());
    if total == 0 {
        return 1.0;
    }
    old.iter()
        .zip(new)
        .filter(|(left, right)| left == right)
        .count() as f32
        / total as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use il2cpp_core::registration::{MethodAddress, NativeMethodIndex};
    #[test]
    fn similarity_preserves_sequence_cost() {
        let item = |name: &str| NormalizedInstruction {
            mnemonic: name.to_owned(),
            operands: vec![],
        };
        assert_eq!(
            sequence_similarity(&[item("a"), item("b")], &[item("a"), item("c")]),
            0.5
        );
    }

    #[test]
    fn normalizes_internal_branch_without_absolute_address() {
        let instruction = Instruction {
            address: 0x1000,
            size: 4,
            bytes: vec![],
            mnemonic: "cbz".to_owned(),
            operands: "x0, 0x1008".to_owned(),
            control_flow: ControlFlow::ConditionalBranch {
                target: Some(0x1008),
            },
        };
        let target = Instruction {
            address: 0x1008,
            size: 4,
            bytes: vec![],
            mnemonic: "ret".to_owned(),
            operands: String::new(),
            control_flow: ControlFlow::Return,
        };
        let targets = HashMap::from([(0x1000, 0), (0x1008, 1)]);
        let methods = NativeMethodIndex::from_addresses(0, Vec::<MethodAddress>::new());
        let value = normalize_instruction(&instruction, &targets, &methods, &[]);
        assert_eq!(
            value.operands,
            vec![
                NormalizedOperand::Register("x0".to_owned()),
                NormalizedOperand::Unknown("BLOCK_1".to_owned())
            ]
        );
        assert_eq!(target.mnemonic, "ret");
    }
}
