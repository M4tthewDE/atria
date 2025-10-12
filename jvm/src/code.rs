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
            let instruction = match bytes[i] {
                0x2 => Instruction::Iconst(-1),
                0x3 => Instruction::Iconst(0),
                0x4 => Instruction::Iconst(1),
                0x5 => Instruction::Iconst(2),
                0x6 => Instruction::Iconst(3),
                0x7 => Instruction::Iconst(4),
                0x8 => Instruction::Iconst(5),
                0x12 => Instruction::Ldc(bytes[i + 1].into()),
                0xb1 => Instruction::Return,
                0xb3 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    Instruction::PutStatic(index.into())
                }
                0xb6 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    Instruction::InvokeVirtual(index.into())
                }
                0xb8 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    Instruction::InvokeStatic(index.into())
                }
                0xbd => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    Instruction::Anewarray(index.into())
                }
                op_code => bail!("unknown instruction: 0x{op_code:x}"),
            };

            i += instruction.length();
            instructions.push(instruction);

            if i == bytes.len() {
                return Ok(Self { instructions });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Iconst(i8),
    Ldc(CpIndex),
    Return,
    PutStatic(CpIndex),
    InvokeVirtual(CpIndex),
    InvokeStatic(CpIndex),
    Anewarray(CpIndex),
}

impl Instruction {
    fn length(&self) -> usize {
        match self {
            Instruction::Iconst(_) => 1,
            Instruction::Ldc(_) => 2,
            Instruction::Return => 1,
            Instruction::PutStatic(_) => 3,
            Instruction::InvokeVirtual(_) => 3,
            Instruction::InvokeStatic(_) => 3,
            Instruction::Anewarray(_) => 3,
        }
    }
}
