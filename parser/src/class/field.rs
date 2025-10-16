use std::{
    collections::HashSet,
    io::{Read, Seek},
};

use anyhow::Result;

use crate::{
    class::{
        attribute::Attribute,
        constant_pool::{ConstantPool, CpIndex},
    },
    util::u2,
};

#[derive(Clone, Debug)]
pub struct Field {
    pub access_flags: HashSet<AccessFlag>,
    pub name_index: CpIndex,
    pub descriptor_index: CpIndex,
    pub attributes: Vec<Attribute>,
}

impl Field {
    pub fn new(r: &mut (impl Read + Seek), cp: &ConstantPool) -> Result<Self> {
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

    pub fn fields(
        r: &mut (impl Read + Seek),
        cp: &ConstantPool,
        count: usize,
    ) -> Result<Vec<Self>> {
        let mut fields = Vec::new();

        for _ in 0..count {
            fields.push(Field::new(r, cp)?);
        }

        Ok(fields)
    }

    pub fn is_static(&self) -> bool {
        self.access_flags.contains(&AccessFlag::Static)
    }

    pub fn get_constant_value_index(&self) -> Option<&CpIndex> {
        self.attributes.iter().find_map(|attr| match attr {
            Attribute::ConstantValue {
                constant_value_index,
                ..
            } => Some(constant_value_index),
            _ => None,
        })
    }

    pub fn name<'a>(&self, cp: &'a ConstantPool) -> Result<&'a str> {
        cp.utf8(&self.name_index)
    }

    pub fn raw_descriptor<'a>(&self, cp: &'a ConstantPool) -> Result<&'a str> {
        cp.utf8(&self.descriptor_index)
    }
}

const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0003;
const ACC_PROTECTED: u16 = 0x0004;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_VOLATILE: u16 = 0x0040;
const ACC_TRANSIENT: u16 = 0x0080;
const ACC_SYNTHETIC: u16 = 0x1000;
const ACC_ENUM: u16 = 0x4000;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum AccessFlag {
    Public,
    Private,
    Protected,
    Static,
    Final,
    Volatile,
    Transient,
    Synthetic,
    Enum,
}

impl AccessFlag {
    fn flags(r: &mut impl Read) -> Result<HashSet<AccessFlag>> {
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

        if raw_flags & ACC_VOLATILE > 0 {
            flags.insert(AccessFlag::Volatile);
        }

        if raw_flags & ACC_TRANSIENT > 0 {
            flags.insert(AccessFlag::Transient);
        }

        if raw_flags & ACC_SYNTHETIC > 0 {
            flags.insert(AccessFlag::Synthetic);
        }

        if raw_flags & ACC_ENUM > 0 {
            flags.insert(AccessFlag::Enum);
        }

        Ok(flags)
    }
}
