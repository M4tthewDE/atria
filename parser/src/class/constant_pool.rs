use std::io::Read;

use anyhow::{Result, bail};

use crate::util::{u1, u2, utf8};

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
        let mut infos = Vec::with_capacity(count.into());
        infos.push(CpInfo::Reserved);

        for _ in 0..count {
            infos.push(CpInfo::new(r)?);
        }

        Ok(Self { infos })
    }
}

const UTF8_TAG: u8 = 1;
const CLASS_TAG: u8 = 7;
const STRING_TAG: u8 = 8;
const FIELD_REF_TAG: u8 = 9;
const METHOD_REF_TAG: u8 = 10;
const INTERFACE_METHOD_REF_TAG: u8 = 11;
const NAME_AND_TYPE_TAG: u8 = 12;
const INVOKE_DYNAMIC_TAG: u8 = 18;

pub enum CpInfo {
    Reserved,
    Utf8(String),
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
    InvokeDynamic {
        bootstrap_method_attr_index: CpIndex,
        name_and_type_index: CpIndex,
    },
}

impl CpInfo {
    fn new(r: &mut impl Read) -> Result<Self> {
        let tag = u1(r)?;

        match tag {
            UTF8_TAG => Self::utf8(r),
            CLASS_TAG => Self::class(r),
            STRING_TAG => Self::string(r),
            FIELD_REF_TAG => Self::field_ref(r),
            METHOD_REF_TAG => Self::method_ref(r),
            INTERFACE_METHOD_REF_TAG => Self::interface_method_ref(r),
            NAME_AND_TYPE_TAG => Self::name_and_type(r),
            INVOKE_DYNAMIC_TAG => Self::invoke_dynamic(r),
            _ => bail!("invalid constant pool info tag {tag}"),
        }
    }

    fn utf8(r: &mut impl Read) -> Result<Self> {
        let length = u2(r)?;
        Ok(Self::Utf8(utf8(r, length.into())?))
    }

    fn class(r: &mut impl Read) -> Result<Self> {
        Ok(Self::Class {
            name_index: u2(r)?.into(),
        })
    }

    fn string(r: &mut impl Read) -> Result<Self> {
        Ok(Self::String {
            string_index: u2(r)?.into(),
        })
    }

    fn field_ref(r: &mut impl Read) -> Result<Self> {
        Ok(Self::FieldRef {
            class_index: u2(r)?.into(),
            name_and_type_index: u2(r)?.into(),
        })
    }

    fn method_ref(r: &mut impl Read) -> Result<Self> {
        Ok(Self::MethodRef {
            class_index: u2(r)?.into(),
            name_and_type_index: u2(r)?.into(),
        })
    }

    fn interface_method_ref(r: &mut impl Read) -> Result<Self> {
        Ok(Self::InterfaceMethodRef {
            class_index: u2(r)?.into(),
            name_and_type_index: u2(r)?.into(),
        })
    }

    fn name_and_type(r: &mut impl Read) -> Result<Self> {
        Ok(Self::NameAndType {
            name_index: u2(r)?.into(),
            descriptor_index: u2(r)?.into(),
        })
    }

    fn invoke_dynamic(r: &mut impl Read) -> Result<Self> {
        Ok(Self::InvokeDynamic {
            bootstrap_method_attr_index: u2(r)?.into(),
            name_and_type_index: u2(r)?.into(),
        })
    }
}
