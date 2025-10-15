use anyhow::{Context, Result, bail};
use parser::class::constant_pool::CpIndex;

fn cp_index(bytes: &[u8]) -> Result<CpIndex> {
    let byte1 = *bytes.get(1).context("premature end of code")?;
    let byte2 = *bytes.get(2).context("premature end of code")?;
    Ok(((byte1 as u16) << 8 | byte2 as u16).into())
}

fn offset(bytes: &[u8]) -> Result<i16> {
    let byte1 = *bytes.get(1).context("premature end of code")?;
    let byte2 = *bytes.get(2).context("premature end of code")?;
    Ok((byte1 as i16) << 8 | byte2 as i16)
}

fn short(bytes: &[u8]) -> Result<u16> {
    let byte1 = *bytes.get(1).context("premature end of code")?;
    let byte2 = *bytes.get(2).context("premature end of code")?;
    Ok((byte1 as u16) << 8 | byte2 as u16)
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
    Areturn,
    InvokeDynamic(CpIndex),
    New(CpIndex),
    Dup,
    InvokeSpecial(CpIndex),
    IfNonNull(i16),
    Ireturn,
    IfNe(i16),
    GetStatic(CpIndex),
    LdcW(CpIndex),
    PutField(CpIndex),
    Iload(u8),
    AconstNull,
    Aastore,
    Bipush(u8),
    Newarray(u8),
    Castore,
    Bastore,
    Iastore,
    Sipush(u16),
    Lreturn,
    Istore(u8),
    Isub,
    Iand,
    Ifeq(i16),
}

impl Instruction {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        Ok(match bytes.first().context("premature end of code")? {
            0x1 => Instruction::AconstNull,
            0x2 => Instruction::Iconst(-1),
            0x3 => Instruction::Iconst(0),
            0x4 => Instruction::Iconst(1),
            0x5 => Instruction::Iconst(2),
            0x6 => Instruction::Iconst(3),
            0x7 => Instruction::Iconst(4),
            0x8 => Instruction::Iconst(5),
            0x10 => Instruction::Bipush(*bytes.get(1).context("premature end of code")?),
            0x11 => Instruction::Sipush(short(bytes)?),
            0x12 => Instruction::Ldc((*bytes.get(1).context("premature end of code")?).into()),
            0x13 => Instruction::LdcW(cp_index(bytes)?),
            0x1a => Instruction::Iload(0),
            0x1b => Instruction::Iload(1),
            0x1c => Instruction::Iload(2),
            0x1d => Instruction::Iload(3),
            0x2a => Instruction::Aload(0),
            0x2b => Instruction::Aload(1),
            0x2c => Instruction::Aload(2),
            0x2d => Instruction::Aload(3),
            0x3b => Instruction::Istore(0),
            0x3c => Instruction::Istore(1),
            0x3d => Instruction::Istore(2),
            0x3e => Instruction::Istore(3),
            0x4b => Instruction::Astore(0),
            0x4c => Instruction::Astore(1),
            0x4d => Instruction::Astore(2),
            0x4f => Instruction::Iastore,
            0x4e => Instruction::Astore(3),
            0x53 => Instruction::Aastore,
            0x54 => Instruction::Bastore,
            0x55 => Instruction::Castore,
            0x59 => Instruction::Dup,
            0x64 => Instruction::Isub,
            0x7e => Instruction::Iand,
            0x99 => Instruction::Ifeq(offset(bytes)?),
            0x9a => Instruction::IfNe(offset(bytes)?),
            0xac => Instruction::Ireturn,
            0xad => Instruction::Lreturn,
            0xb0 => Instruction::Areturn,
            0xb1 => Instruction::Return,
            0xb2 => Instruction::GetStatic(cp_index(bytes)?),
            0xb3 => Instruction::PutStatic(cp_index(bytes)?),
            0xb4 => Instruction::GetField(cp_index(bytes)?),
            0xb5 => Instruction::PutField(cp_index(bytes)?),
            0xb6 => Instruction::InvokeVirtual(cp_index(bytes)?),
            0xb7 => Instruction::InvokeSpecial(cp_index(bytes)?),
            0xb8 => Instruction::InvokeStatic(cp_index(bytes)?),
            0xba => Instruction::InvokeDynamic(cp_index(bytes)?),
            0xbb => Instruction::New(cp_index(bytes)?),
            0xbc => Instruction::Newarray(*bytes.get(1).context("premature end of code")?),
            0xbd => Instruction::Anewarray(cp_index(bytes)?),
            0xc6 => Instruction::IfNull(offset(bytes)?),
            0xc7 => Instruction::IfNonNull(offset(bytes)?),
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
            Self::Areturn => 1,
            Self::InvokeDynamic(_) => 5,
            Self::New(_) => 3,
            Self::Dup => 1,
            Self::InvokeSpecial(_) => 3,
            Self::IfNonNull(_) => 3,
            Self::Ireturn => 1,
            Self::IfNe(_) => 3,
            Self::GetStatic(_) => 3,
            Self::LdcW(_) => 3,
            Self::PutField(_) => 3,
            Self::Iload(_) => 1,
            Self::AconstNull => 1,
            Self::Aastore => 1,
            Self::Bipush(_) => 2,
            Self::Newarray(_) => 2,
            Self::Castore => 1,
            Self::Bastore => 1,
            Self::Iastore => 1,
            Self::Sipush(_) => 3,
            Self::Lreturn => 1,
            Self::Istore(_) => 1,
            Self::Isub => 1,
            Self::Iand => 1,
            Self::Ifeq(_) => 3,
        }
    }
}
