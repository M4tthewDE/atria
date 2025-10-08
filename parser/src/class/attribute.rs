use std::io::Read;

use anyhow::{Result, bail};
use tracing::debug;

use crate::{
    class::constant_pool::{ConstantPool, CpIndex},
    util::{u1, u2, u4, vec},
};

const CONSTANT_VALUE_ATTR_NAME: &str = "ConstantValue";
const RUNTIME_VISIBLE_ANNOTATIONS_ATTR_NAME: &str = "RuntimeVisibleAnnotations";
const CODE_ATTR_NAME: &str = "Code";

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
    Code {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        max_stack: u16,
        max_locals: u16,
        code: Vec<u8>,
        exception_table: Vec<ExceptionHandler>,
        attributes: Vec<Attribute>,
    },
}

impl Attribute {
    pub fn new(r: &mut impl Read, cp: &ConstantPool) -> Result<Self> {
        let attribute_name_index = u2(r)?.into();
        let attribute_length = u4(r)?;

        let name = cp.utf8(&attribute_name_index)?;

        debug!("parsing {name} attribute");

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
            CODE_ATTR_NAME => {
                let max_stack = u2(r)?;
                let max_locals = u2(r)?;
                let code_length = u4(r)?;
                let code = vec(r, code_length as usize)?;

                let exception_table_length = u2(r)?;
                let mut exception_table = Vec::new();
                for _ in 0..exception_table_length {
                    exception_table.push(ExceptionHandler::new(r)?);
                }

                let attributes_count = u2(r)?;
                let attributes = Attribute::attributes(r, cp, attributes_count.into())?;

                Self::Code {
                    attribute_name_index,
                    attribute_length,
                    max_stack,
                    max_locals,
                    code,
                    exception_table,
                    attributes,
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

pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

impl ExceptionHandler {
    fn new(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            start_pc: u2(r)?,
            end_pc: u2(r)?,
            handler_pc: u2(r)?,
            catch_type: u2(r)?,
        })
    }
}
