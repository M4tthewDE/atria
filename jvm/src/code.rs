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
                0x2a => Instruction::Aload(0),
                0x2b => Instruction::Aload(1),
                0x2c => Instruction::Aload(2),
                0x2d => Instruction::Aload(3),
                0x4b => Instruction::Astore(0),
                0x4c => Instruction::Astore(1),
                0x4d => Instruction::Astore(2),
                0x4e => Instruction::Astore(3),
                0xa7 => {
                    let offset = (bytes[i + 1] as i16) << 8 | bytes[i + 2] as i16;
                    Instruction::Goto(offset)
                }
                0xb0 => Instruction::Areturn,
                0xb1 => Instruction::Return,
                0xb3 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    Instruction::PutStatic(index.into())
                }
                0xb4 => {
                    let index = (bytes[i + 1] as u16) << 8 | bytes[i + 2] as u16;
                    Instruction::GetField(index.into())
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
                0xc6 => {
                    let offset = (bytes[i + 1] as i16) << 8 | bytes[i + 2] as i16;
                    Instruction::IfNull(offset)
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
    Aload(u8),
    GetField(CpIndex),
    Astore(u8),
    IfNull(i16),
    Goto(i16),
    Areturn,
}

impl Instruction {
    fn length(&self) -> usize {
        match self {
            Self::Iconst(_) => 1,
            Self::Ldc(_) => 2,
            Self::Return => 1,
            Self::PutStatic(_) => 3,
            Self::InvokeVirtual(_) => 3,
            Self::InvokeStatic(_) => 3,
            Self::Anewarray(_) => 3,
            Self::Aload(_) => 1,
            Self::GetField(_) => 3,
            Self::Astore(_) => 1,
            Self::IfNull(_) => 3,
            Self::Goto(_) => 3,
            Self::Areturn => 1,
        }
    }
}
