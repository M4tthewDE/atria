use anyhow::{Result, bail};
use parser::class::constant_pool::CpIndex;

fn cp_index(bytes: &[u8]) -> CpIndex {
    ((bytes[1] as u16) << 8 | bytes[2] as u16).into()
}

fn offset(bytes: &[u8]) -> i16 {
    (bytes[1] as i16) << 8 | bytes[2] as i16
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
    InvokeDynamic(CpIndex),
    New(CpIndex),
    Dup,
    InvokeSpecial(CpIndex),
}

impl Instruction {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        Ok(match bytes[0] {
            0x2 => Instruction::Iconst(-1),
            0x3 => Instruction::Iconst(0),
            0x4 => Instruction::Iconst(1),
            0x5 => Instruction::Iconst(2),
            0x6 => Instruction::Iconst(3),
            0x7 => Instruction::Iconst(4),
            0x8 => Instruction::Iconst(5),
            0x12 => Instruction::Ldc(bytes[1].into()),
            0x2a => Instruction::Aload(0),
            0x2b => Instruction::Aload(1),
            0x2c => Instruction::Aload(2),
            0x2d => Instruction::Aload(3),
            0x4b => Instruction::Astore(0),
            0x4c => Instruction::Astore(1),
            0x4d => Instruction::Astore(2),
            0x4e => Instruction::Astore(3),
            0x59 => Instruction::Dup,
            0xa7 => Instruction::Goto(offset(bytes)),
            0xb0 => Instruction::Areturn,
            0xb1 => Instruction::Return,
            0xb3 => Instruction::PutStatic(cp_index(bytes)),
            0xb4 => Instruction::GetField(cp_index(bytes)),
            0xb6 => Instruction::InvokeVirtual(cp_index(bytes)),
            0xb7 => Instruction::InvokeSpecial(cp_index(bytes)),
            0xb8 => Instruction::InvokeStatic(cp_index(bytes)),
            0xba => Instruction::InvokeDynamic(cp_index(bytes)),
            0xbb => Instruction::New(cp_index(bytes)),
            0xbd => Instruction::Anewarray(cp_index(bytes)),
            0xc6 => Instruction::IfNull(offset(bytes)),
            op_code => bail!("unknown instruction: 0x{op_code:x}"),
        })
    }

    pub fn length(&self) -> usize {
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
            Self::InvokeDynamic(_) => 5,
            Self::New(_) => 3,
            Self::Dup => 1,
            Self::InvokeSpecial(_) => 3,
        }
    }
}
