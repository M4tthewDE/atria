use anyhow::{Result, bail};
use parser::class::constant_pool::CpIndex;

#[derive(Debug, Clone)]
pub struct Code {
    pub instructions: Vec<Instruction>,
}

impl Code {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let mut instructions = Vec::new();
        let mut i = 0;
        loop {
            match bytes[i] {
                0x3 => {
                    instructions.push(Instruction::Iconst(0));
                    i += 1
                }
                0x12 => {
                    instructions.push(Instruction::Ldc(bytes[i + 1].into()));
                    i += 2
                }
                0xb1 => {
                    instructions.push(Instruction::Return);
                    i += 1
                }
                0xb3 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    instructions.push(Instruction::PutStatic(index.into()));
                    i += 3
                }
                0xb6 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    instructions.push(Instruction::InvokeVirtual(index.into()));
                    i += 3
                }
                0xb8 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    instructions.push(Instruction::InvokeStatic(index.into()));
                    i += 3
                }
                0xbd => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    instructions.push(Instruction::Anewarray(index.into()));
                    i += 3
                }
                op_code => bail!("unknown instruction: 0x{op_code:x}"),
            }

            if i == bytes.len() {
                return Ok(Self { instructions });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Iconst(u8),
    Ldc(CpIndex),
    Return,
    PutStatic(CpIndex),
    InvokeVirtual(CpIndex),
    InvokeStatic(CpIndex),
    Anewarray(CpIndex),
}
