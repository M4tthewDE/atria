use std::{collections::HashSet, io::Read};

use anyhow::Result;

use crate::util::u2;

const ACC_PUBLIC: u16 = 0x0001;
const ACC_FINAL: u16 = 0x0010;
const ACC_SUPER: u16 = 0x0020;
const ACC_INTERFACE: u16 = 0x0200;
const ACC_ABSTRACT: u16 = 0x0400;
const ACC_SYNTHETIC: u16 = 0x1000;
const ACC_ANNOTATION: u16 = 0x2000;
const ACC_ENUM: u16 = 0x4000;
const ACC_MODULE: u16 = 0x8000;

#[derive(Hash, Eq, PartialEq, Debug)]
pub enum AccessFlag {
    Public,
    Final,
    Super,
    Interface,
    Abstract,
    Synthetic,
    Annotation,
    Enum,
    Module,
}

impl AccessFlag {
    pub fn flags(r: &mut impl Read) -> Result<HashSet<AccessFlag>> {
        let raw_flags = u2(r)?;

        let mut flags = HashSet::new();

        if raw_flags & ACC_PUBLIC > 0 {
            flags.insert(AccessFlag::Public);
        }

        if raw_flags & ACC_FINAL > 0 {
            flags.insert(AccessFlag::Final);
        }

        if raw_flags & ACC_SUPER > 0 {
            flags.insert(AccessFlag::Super);
        }

        if raw_flags & ACC_INTERFACE > 0 {
            flags.insert(AccessFlag::Interface);
        }

        if raw_flags & ACC_ABSTRACT > 0 {
            flags.insert(AccessFlag::Abstract);
        }

        if raw_flags & ACC_SYNTHETIC > 0 {
            flags.insert(AccessFlag::Synthetic);
        }

        if raw_flags & ACC_ANNOTATION > 0 {
            flags.insert(AccessFlag::Annotation);
        }

        if raw_flags & ACC_ENUM > 0 {
            flags.insert(AccessFlag::Enum);
        }

        if raw_flags & ACC_MODULE > 0 {
            flags.insert(AccessFlag::Module);
        }

        Ok(flags)
    }
}
