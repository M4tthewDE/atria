use core::f32;
use std::io::Read;

use anyhow::{Context, Result, bail};
use tracing::trace;

use crate::util::{f4, u1, u2, u4, u8, utf8, vec};

/// A valid index into the constant pool.
#[derive(Debug, Clone)]
pub struct CpIndex(pub u16);

impl From<u16> for CpIndex {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<u8> for CpIndex {
    fn from(value: u8) -> Self {
        Self(value as u16)
    }
}

impl PartialEq<i32> for CpIndex {
    fn eq(&self, other: &i32) -> bool {
        *other == self.0.into()
    }
}

/// A table of structures representing various string constants,
/// class and interface names, field names, and other constant structures
#[derive(Clone)]
pub struct ConstantPool {
    pub infos: Vec<CpInfo>,
}

impl ConstantPool {
    pub fn new(r: &mut impl Read, count: u16) -> Result<Self> {
        // first item is reserved
        let count = count - 1;

        let mut infos = Vec::with_capacity(count.into());
        infos.push(CpInfo::Reserved);

        let mut i = 0;
        loop {
            if i == count {
                break;
            }

            let cp_info = CpInfo::new(r)?;

            trace!("{}: {cp_info:?}", i + 1);
            infos.push(cp_info.clone());
            if matches!(cp_info, CpInfo::Long(_)) {
                infos.push(CpInfo::Reserved);
                i += 2;
            } else {
                i += 1;
            }
        }

        trace!("parsed {} constant pool items", count);

        Ok(Self { infos })
    }

    pub fn utf8(&self, index: &CpIndex) -> Result<&str> {
        if let CpInfo::Utf8(content) = self
            .infos
            .get(index.0 as usize)
            .context(format!("constant pool item at index {} not found", index.0))?
        {
            Ok(content)
        } else {
            bail!("no utf8 constant pool item found at index {index:?}")
        }
    }

    pub fn class_name(&self, index: &CpIndex) -> Result<&str> {
        if let CpInfo::Class { name_index } = self
            .infos
            .get(index.0 as usize)
            .context(format!("constant pool item at index {} not found", index.0))?
        {
            self.utf8(name_index)
        } else {
            bail!("no utf8 constant pool item found at index {index:?}")
        }
    }

    pub fn name_and_type(&self, index: &CpIndex) -> Result<(&str, &str)> {
        if let CpInfo::NameAndType {
            name_index,
            descriptor_index,
        } = self
            .infos
            .get(index.0 as usize)
            .context(format!("constant pool item at index {} not found", index.0))?
        {
            Ok((self.utf8(name_index)?, self.utf8(descriptor_index)?))
        } else {
            bail!("no name_and_type constant pool item found at index {index:?}")
        }
    }
}

const UTF8_TAG: u8 = 1;
const INTEGER_TAG: u8 = 3;
const FLOAT_TAG: u8 = 4;
const LONG_TAG: u8 = 5;
const CLASS_TAG: u8 = 7;
const STRING_TAG: u8 = 8;
const FIELD_REF_TAG: u8 = 9;
const METHOD_REF_TAG: u8 = 10;
const INTERFACE_METHOD_REF_TAG: u8 = 11;
const NAME_AND_TYPE_TAG: u8 = 12;
const METHOD_HANDLE_TAG: u8 = 15;
const METHOD_TYPE_TAG: u8 = 16;
const INVOKE_DYNAMIC_TAG: u8 = 18;

#[derive(Debug, Clone)]
pub enum CpInfo {
    Reserved,
    Utf8(String),
    Integer(u32),
    Float(f32),
    Long(u64),
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
            FLOAT_TAG => Ok(Self::Float(f4(r)?)),
            LONG_TAG => Ok(Self::Long(u8(r)?)),
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

#[derive(Debug, Clone)]
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
