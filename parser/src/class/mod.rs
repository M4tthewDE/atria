use std::io::Read;

use anyhow::{Result, bail};

use crate::{
    class::constant_pool::ConstantPool,
    util::{u2, u4},
};

mod constant_pool;

/// Representation of a class, interface or module
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
}

impl ClassFile {
    pub fn new(r: &mut impl Read) -> Result<Self> {
        let magic = u4(r)?;

        if magic != 0xCAFEBABE {
            bail!("invalid magic number 0x{magic:x}");
        }

        let minor_version = u2(r)?;
        let major_version = u2(r)?;

        if major_version != 61 && minor_version != 0 {
            bail!(
                "unsupported class file version {}.{}",
                major_version,
                minor_version
            );
        }

        let constant_pool_count = u2(r)?;
        let constant_pool = ConstantPool::new(r, constant_pool_count)?;

        Ok(Self {
            minor_version,
            major_version,
            constant_pool,
        })
    }
}
