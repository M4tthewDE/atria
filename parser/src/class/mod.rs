use std::{
    collections::HashSet,
    io::{Read, Seek},
};

use anyhow::{Context, Result, bail};
use tracing::trace;

use crate::{
    class::{
        access_flags::AccessFlag,
        attribute::Attribute,
        constant_pool::{ConstantPool, CpIndex, CpInfo},
        field::Field,
        method::Method,
    },
    util::{u2, u4},
};

pub mod access_flags;
pub mod attribute;
pub mod constant_pool;
pub mod field;
mod method;

/// Representation of a class, interface or module
#[derive(Clone)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
    pub access_flags: HashSet<AccessFlag>,
    pub this_class: CpIndex,
    pub super_class: CpIndex,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub attributes: Vec<Attribute>,
}

impl ClassFile {
    pub fn new(r: &mut (impl Read + Seek)) -> Result<Self> {
        let magic = u4(r)?;

        if magic != 0xCAFEBABE {
            bail!("invalid magic number 0x{magic:x}");
        }

        let minor_version = u2(r)?;
        let major_version = u2(r)?;

        let constant_pool_count = u2(r)?;
        let constant_pool = ConstantPool::new(r, constant_pool_count)?;

        let access_flags = AccessFlag::flags(r)?;
        trace!("access flags: {access_flags:?}");

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

        let attributes_count = u2(r)?;
        let attributes = Attribute::attributes(r, &constant_pool, attributes_count.into())?;

        Ok(Self {
            minor_version,
            major_version,
            constant_pool,
            access_flags,
            this_class,
            super_class,
            fields,
            methods,
            attributes,
        })
    }

    pub fn cp_item(&self, index: &CpIndex) -> Result<&CpInfo> {
        self.constant_pool
            .infos
            .get(index.0 as usize)
            .context(format!("no constant pool item at index {index:?}"))
    }
}
