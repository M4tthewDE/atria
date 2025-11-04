use anyhow::{Result, bail};
use parser::class::attribute::Attribute;

#[derive(Debug)]
pub struct Code {
    instructions: Vec<u8>,
    max_locals: u16,
    attributes: Vec<Attribute>,
}

impl Code {
    pub fn new(attribute: Attribute) -> Result<Self> {
        if let Attribute::Code {
            max_locals,
            code,
            attributes,
            ..
        } = attribute
        {
            Ok(Self {
                instructions: code,
                max_locals,
                attributes,
            })
        } else {
            bail!("cannot build Code from {attribute:?}");
        }
    }

    pub fn instructions(&self) -> &[u8] {
        &self.instructions
    }

    pub fn max_locals(&self) -> u16 {
        self.max_locals
    }

    pub fn line_number(&self, pc: u16) -> Option<u16> {
        let mut res = None;
        for attribute in &self.attributes {
            if let Attribute::LineNumberTable {
                line_number_table, ..
            } = attribute
            {
                for entry in line_number_table {
                    if entry.start_pc <= pc {
                        res = Some(entry.line_number);
                    }
                }
            }
        }

        res
    }
}
