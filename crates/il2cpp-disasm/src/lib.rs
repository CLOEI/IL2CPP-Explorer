//! Backend-independent native instruction and function inspection APIs.

mod arm64;
mod inspector;

use serde::{Deserialize, Serialize};

pub use arm64::Arm64Disassembler;
pub use inspector::{
    FunctionInspection, FunctionInspector, collect_direct_calls, disassemble_executable_window,
};

/// Normalized control-flow information proven by one decoded instruction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlFlow {
    #[default]
    None,
    Branch {
        target: Option<u64>,
    },
    ConditionalBranch {
        target: Option<u64>,
    },
    Call {
        target: Option<u64>,
    },
    Return,
}

/// One backend-independent native instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub address: u64,
    pub size: u8,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operands: String,
    pub control_flow: ControlFlow,
}

/// Backend-independent native instruction decoder.
pub trait Disassembler {
    fn disassemble(&self, code: &[u8], address: u64) -> anyhow::Result<Vec<Instruction>>;
}

/// Evidence used to select a native function boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionRangeSource {
    NextMethod,
    Symbol,
    ExplicitLength,
    Unknown,
}

/// Known or bounded native function range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionRange {
    pub start: u64,
    pub end: Option<u64>,
    pub source: FunctionRangeSource,
}

/// One direct AArch64 `BL` call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectCall {
    pub caller: il2cpp_core::model::MethodId,
    pub call_address: u64,
    pub target_address: u64,
    pub callees: Vec<il2cpp_core::model::MethodId>,
}

/// CFG groundwork without inferred graph edges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicBlock {
    pub start: u64,
    pub instructions: Vec<Instruction>,
}
