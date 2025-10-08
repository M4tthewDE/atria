use std::{collections::HashSet, io::Read};

use anyhow::{Result, bail};
use tracing::debug;

use crate::{
    class::{
        access_flags::AccessFlag,
        constant_pool::{ConstantPool, CpIndex},
        field::Field,
        method::Method,
    },
    util::{u2, u4},
};

mod access_flags;
mod attribute;
mod constant_pool;
mod field;
mod method;

/// Representation of a class, interface or module
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
    pub access_flags: HashSet<AccessFlag>,
    pub this_class: CpIndex,
    pub super_class: CpIndex,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
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

        let access_flags = AccessFlag::flags(r)?;
        debug!("access flags: {access_flags:?}");

        let this_class = u2(r)?.into();
        let super_class = u2(r)?.into();

        let interfaces_count = u2(r)?;
        if interfaces_count != 0 {
            bail!("todo: parse interfaces");
        }

        let fields_count = u2(r)?;
        let fields = Field::fields(r, &constant_pool, fields_count.into())?;

        let methods_count = u2(r)?;
        let methods = Method::methods(r, &constant_pool, methods_count)?;

        Ok(Self {
            minor_version,
            major_version,
            constant_pool,
            access_flags,
            this_class,
            super_class,
            fields,
            methods,
        })
    }
}
