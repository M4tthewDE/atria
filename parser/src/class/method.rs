use anyhow::Result;
use std::{
    collections::HashSet,
    io::{Read, Seek},
};
use tracing::trace;

use crate::{
    class::{
        attribute::Attribute,
        constant_pool::{ConstantPool, CpIndex},
        descriptor::MethodDescriptor,
    },
    util::u2,
};

#[derive(Clone, Debug)]
pub struct Method {
    pub access_flags: HashSet<AccessFlag>,
    pub name_index: CpIndex,
    pub descriptor_index: CpIndex,
    pub attributes: Vec<Attribute>,
}

impl Method {
    fn new(r: &mut (impl Read + Seek), cp: &ConstantPool) -> Result<Self> {
        let access_flags = AccessFlag::flags(r)?;
        let name_index = u2(r)?.into();
        let descriptor_index = u2(r)?.into();
        let attributes_count = u2(r)?;
        let attributes = Attribute::attributes(r, cp, attributes_count.into())?;

        Ok(Self {
            access_flags,
            name_index,
            descriptor_index,
            attributes,
        })
    }

    pub fn methods(r: &mut (impl Read + Seek), cp: &ConstantPool, count: u16) -> Result<Vec<Self>> {
        let mut methods = Vec::new();

        trace!("parsing {count} methods");
        for _ in 0..count {
            methods.push(Method::new(r, cp)?);
        }

        Ok(methods)
    }

    pub fn code(&self) -> Option<&Vec<u8>> {
        self.attributes.iter().find_map(|attr| match attr {
            Attribute::Code { code, .. } => Some(code),
            _ => None,
        })
    }

    pub fn name<'a>(&self, cp: &'a ConstantPool) -> Result<&'a str> {
        cp.utf8(&self.name_index)
    }

    pub fn raw_descriptor<'a>(&self, cp: &'a ConstantPool) -> Result<&'a str> {
        cp.utf8(&self.descriptor_index)
    }

    pub fn descriptor(&self, cp: &ConstantPool) -> Result<MethodDescriptor> {
        let raw = cp.utf8(&self.descriptor_index)?;
        MethodDescriptor::new(raw)
    }

    pub fn is_varargs(&self) -> bool {
        self.access_flags.contains(&AccessFlag::Varargs)
    }

    pub fn is_native(&self) -> bool {
        self.access_flags.contains(&AccessFlag::Native)
    }
}

const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0002;
const ACC_PROTECTED: u16 = 0x0004;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_SYNCHRONIZED: u16 = 0x0020;
const ACC_BRIDGE: u16 = 0x0040;
const ACC_VARARGS: u16 = 0x0080;
const ACC_NATIVE: u16 = 0x0100;
const ACC_ABSTRACT: u16 = 0x0400;
const ACC_STRICT: u16 = 0x0800;
const ACC_SYNTHETIC: u16 = 0x1000;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum AccessFlag {
    Public,
    Private,
    Protected,
    Static,
    Final,
    Synchronized,
    Bridge,
    Varargs,
    Native,
    Abstract,
    Strict,
    Synthetic,
}

impl AccessFlag {
    pub fn flags(r: &mut impl Read) -> Result<HashSet<AccessFlag>> {
        let raw_flags = u2(r)?;

        let mut flags = HashSet::new();

        if raw_flags & ACC_PUBLIC > 0 {
            flags.insert(AccessFlag::Public);
        }

        if raw_flags & ACC_PRIVATE > 0 {
            flags.insert(AccessFlag::Private);
        }

        if raw_flags & ACC_PROTECTED > 0 {
            flags.insert(AccessFlag::Protected);
        }

        if raw_flags & ACC_STATIC > 0 {
            flags.insert(AccessFlag::Static);
        }

        if raw_flags & ACC_FINAL > 0 {
            flags.insert(AccessFlag::Final);
        }

        if raw_flags & ACC_SYNCHRONIZED > 0 {
            flags.insert(AccessFlag::Synchronized);
        }

        if raw_flags & ACC_BRIDGE > 0 {
            flags.insert(AccessFlag::Bridge);
        }

        if raw_flags & ACC_VARARGS > 0 {
            flags.insert(AccessFlag::Varargs);
        }

        if raw_flags & ACC_NATIVE > 0 {
            flags.insert(AccessFlag::Native);
        }

        if raw_flags & ACC_ABSTRACT > 0 {
            flags.insert(AccessFlag::Abstract);
        }

        if raw_flags & ACC_STRICT > 0 {
            flags.insert(AccessFlag::Strict);
        }

        if raw_flags & ACC_SYNTHETIC > 0 {
            flags.insert(AccessFlag::Synthetic);
        }

        Ok(flags)
    }
}
