use std::collections::HashMap;

use anyhow::{Result, bail};
use parser::class::{
    ClassFile,
    constant_pool::{CpIndex, CpInfo},
    field::Field,
};
use tracing::trace;

use crate::ClassIdentifier;

#[derive(Clone)]
pub struct Class {
    pub identifier: ClassIdentifier,
    fields: HashMap<String, FieldValue>,
    pub class_file: ClassFile,
    pub initialized: bool,
    pub being_initialized: bool,
}

impl Class {
    pub fn new(identifier: ClassIdentifier, class_file: ClassFile) -> Self {
        Self {
            identifier,
            class_file,
            fields: HashMap::default(),
            initialized: false,
            being_initialized: false,
        }
    }

    pub fn initialize_fields(&mut self) -> Result<()> {
        for field in &self.class_file.fields.clone() {
            if field.is_static_final() {
                self.initialize_static_final_field(field)?;
            }
        }

        Ok(())
    }

    fn initialize_static_final_field(&mut self, field: &Field) -> Result<()> {
        let name = self.class_file.constant_pool.utf8(&field.name_index)?;

        trace!("initializing field {name}");

        if let Some(constant_value_index) = field.get_constant_value_index() {
            let field_value = self.resolve_constant_value(constant_value_index)?;
            self.fields.insert(name.to_string(), field_value);
        }

        Ok(())
    }

    fn resolve_constant_value(&self, constant_value_index: &CpIndex) -> Result<FieldValue> {
        Ok(match self.class_file.cp_item(constant_value_index)? {
            CpInfo::String { string_index } => FieldValue::String(
                self.class_file
                    .constant_pool
                    .utf8(string_index)?
                    .to_string(),
            ),
            CpInfo::Integer(val) => FieldValue::Integer(*val),
            CpInfo::Long(val) => FieldValue::Long(*val),
            item => bail!("invalid constant pool item: {item:?}"),
        })
    }
}

#[derive(Clone)]
enum FieldValue {
    String(String),
    Integer(u32),
    Long(u64),
}
