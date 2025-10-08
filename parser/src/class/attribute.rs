use std::io::Read;

use anyhow::{Result, bail};

use crate::{
    class::constant_pool::{ConstantPool, CpIndex},
    util::{u1, u2, u4},
};

const CONSTANT_VALUE_ATTR_NAME: &str = "ConstantValue";
const RUNTIME_VISIBLE_ANNOTATIONS_ATTR_NAME: &str = "RuntimeVisibleAnnotations";

pub enum Attribute {
    ConstantValue {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        constant_value_index: CpIndex,
    },
    RuntimeVisibleAnnoations {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        annotations: Vec<Annotation>,
    },
}

impl Attribute {
    pub fn new(r: &mut impl Read, cp: &ConstantPool) -> Result<Self> {
        let attribute_name_index = u2(r)?.into();
        let attribute_length = u4(r)?;

        let name = cp.utf8(&attribute_name_index)?;

        Ok(match name {
            CONSTANT_VALUE_ATTR_NAME => Self::ConstantValue {
                attribute_name_index,
                attribute_length,
                constant_value_index: u2(r)?.into(),
            },
            RUNTIME_VISIBLE_ANNOTATIONS_ATTR_NAME => {
                let num_annotations = u2(r)?;

                Self::RuntimeVisibleAnnoations {
                    attribute_name_index,
                    attribute_length,
                    annotations: Annotation::annotations(r, num_annotations.into())?,
                }
            }
            _ => bail!("unknown attribute {}", name),
        })
    }

    pub fn attributes(r: &mut impl Read, cp: &ConstantPool, count: usize) -> Result<Vec<Self>> {
        let mut attributes = Vec::new();

        for _ in 0..count {
            attributes.push(Attribute::new(r, cp)?);
        }

        Ok(attributes)
    }
}

pub struct Annotation {
    pub type_index: CpIndex,
    pub element_value_pairs: Vec<ElementValuePair>,
}

impl Annotation {
    fn new(r: &mut impl Read) -> Result<Self> {
        let type_index = u2(r)?.into();
        let num_element_value_pairs = u2(r)?;

        let mut element_value_pairs = Vec::new();
        for _ in 0..num_element_value_pairs {
            element_value_pairs.push(ElementValuePair::new(r)?);
        }

        Ok(Self {
            type_index,
            element_value_pairs,
        })
    }

    fn annotations(r: &mut impl Read, count: usize) -> Result<Vec<Self>> {
        let mut annotations = Vec::new();
        for _ in 0..count {
            annotations.push(Annotation::new(r)?);
        }

        Ok(annotations)
    }
}

pub struct ElementValuePair {
    pub element_name_index: CpIndex,
    pub value: ElementValue,
}

impl ElementValuePair {
    fn new(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            element_name_index: u2(r)?.into(),
            value: ElementValue::new(r)?,
        })
    }
}

pub struct ElementValue {}

impl ElementValue {
    fn new(r: &mut impl Read) -> Result<Self> {
        let tag = u1(r)?;
        bail!("invalid element value tag: {tag}")
    }
}
