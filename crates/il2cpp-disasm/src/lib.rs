//! Disassembly interfaces for native IL2CPP method bodies.
//!
//! No disassembly engine is selected yet. A future backend can implement
//! [`Disassembler`] without adding that dependency to `il2cpp-core`.

/// One decoded native instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub address: u64,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operands: String,
}

/// Backend-independent native instruction decoder.
pub trait Disassembler {
    fn disassemble(&self, bytes: &[u8], start_address: u64) -> anyhow::Result<Vec<Instruction>>;
}
