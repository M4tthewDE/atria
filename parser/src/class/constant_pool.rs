use std::io::Read;

use anyhow::{Result, bail};
use tracing::{debug, trace};

use crate::util::{u1, u2, u4, utf8};

/// A valid index into the constant pool.
#[derive(Debug)]
pub struct CpIndex(pub u16);

impl From<u16> for CpIndex {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

/// A table of structures representing various string constants,
/// class and interface names, field names, and other constant structures
pub struct ConstantPool {
    pub infos: Vec<CpInfo>,
}

impl ConstantPool {
    pub fn new(r: &mut impl Read, count: u16) -> Result<Self> {
        // first item is reserved
        let count = count - 1;

        let mut infos = Vec::with_capacity(count.into());
        infos.push(CpInfo::Reserved);

        for i in 0..count {
            let cp_info = CpInfo::new(r)?;
            trace!("{}: {cp_info:?}", i + 1);
            infos.push(cp_info);
        }

        debug!("parsed {} constant pool items", count);

        Ok(Self { infos })
    }
}

const UTF8_TAG: u8 = 1;
const INTEGER_TAG: u8 = 3;
const CLASS_TAG: u8 = 7;
const STRING_TAG: u8 = 8;
const FIELD_REF_TAG: u8 = 9;
const METHOD_REF_TAG: u8 = 10;
const INTERFACE_METHOD_REF_TAG: u8 = 11;
const NAME_AND_TYPE_TAG: u8 = 12;
const METHOD_HANDLE_TAG: u8 = 15;
const METHOD_TYPE_TAG: u8 = 16;
const INVOKE_DYNAMIC_TAG: u8 = 18;

#[derive(Debug)]
pub enum CpInfo {
    Reserved,
    Utf8(String),
    Integer(u32),
    Class {
        name_index: CpIndex,
    },
    String {
        string_index: CpIndex,
    },
    FieldRef {
        class_index: CpIndex,
        name_and_type_index: CpIndex,
    },
    MethodRef {
        class_index: CpIndex,
        name_and_type_index: CpIndex,
    },
    InterfaceMethodRef {
        class_index: CpIndex,
        name_and_type_index: CpIndex,
    },
    NameAndType {
        name_index: CpIndex,
        descriptor_index: CpIndex,
    },
    MethodHandle {
        reference_kind: ReferenceKind,
        reference_index: CpIndex,
    },
    MethodType {
        descriptor_index: CpIndex,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: CpIndex,
        name_and_type_index: CpIndex,
    },
}

impl CpInfo {
    fn new(r: &mut impl Read) -> Result<Self> {
        let tag = u1(r)?;

        match tag {
            UTF8_TAG => {
                let length = u2(r)?;
                Ok(Self::Utf8(utf8(r, length.into())?))
            }
            INTEGER_TAG => Ok(Self::Integer(u4(r)?)),
            CLASS_TAG => Ok(Self::Class {
                name_index: u2(r)?.into(),
            }),
            STRING_TAG => Ok(Self::String {
                string_index: u2(r)?.into(),
            }),
            FIELD_REF_TAG => Ok(Self::FieldRef {
                class_index: u2(r)?.into(),
                name_and_type_index: u2(r)?.into(),
            }),
            METHOD_REF_TAG => Ok(Self::MethodRef {
                class_index: u2(r)?.into(),
                name_and_type_index: u2(r)?.into(),
            }),
            INTERFACE_METHOD_REF_TAG => Ok(Self::InterfaceMethodRef {
                class_index: u2(r)?.into(),
                name_and_type_index: u2(r)?.into(),
            }),
            NAME_AND_TYPE_TAG => Ok(Self::NameAndType {
                name_index: u2(r)?.into(),
                descriptor_index: u2(r)?.into(),
            }),
            METHOD_HANDLE_TAG => Ok(Self::MethodHandle {
                reference_kind: ReferenceKind::new(u1(r)?)?,
                reference_index: u2(r)?.into(),
            }),
            METHOD_TYPE_TAG => Ok(Self::MethodType {
                descriptor_index: u2(r)?.into(),
            }),
            INVOKE_DYNAMIC_TAG => Ok(Self::InvokeDynamic {
                bootstrap_method_attr_index: u2(r)?.into(),
                name_and_type_index: u2(r)?.into(),
            }),
            _ => bail!("invalid constant pool info tag {tag}"),
        }
    }
}

#[derive(Debug)]
pub enum ReferenceKind {
    GetField,
    GetStatic,
    PutField,
    PutStatic,
    InvokeVirtual,
    InvokeStatic,
    InvokeSpecial,
    NewInvokeSpecial,
    InvokeInterface,
}

impl ReferenceKind {
    pub fn new(value: u8) -> Result<Self> {
        Ok(match value {
            1 => Self::GetField,
            2 => Self::GetStatic,
            3 => Self::PutField,
            4 => Self::PutStatic,
            5 => Self::InvokeVirtual,
            6 => Self::InvokeStatic,
            7 => Self::InvokeSpecial,
            8 => Self::NewInvokeSpecial,
            9 => Self::InvokeInterface,
            _ => bail!("invalid reference kind {value}"),
        })
    }
}
