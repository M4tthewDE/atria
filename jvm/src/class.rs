use std::collections::HashMap;

use anyhow::{Result, bail};
use parser::class::{
    ClassFile,
    constant_pool::{CpIndex, CpInfo},
    descriptor::MethodDescriptor,
    field::Field,
    method::Method,
};
use tracing::trace;

use crate::ClassIdentifier;

#[derive(Clone)]
pub struct Class {
    identifier: ClassIdentifier,
    fields: HashMap<String, FieldValue>,
    class_file: ClassFile,
    initialized: bool,
    being_initialized: bool,
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

    pub fn identifier(&self) -> &ClassIdentifier {
        &self.identifier
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn finished_initialization(&mut self) {
        self.initialized = true
    }

    pub fn being_initialized(&self) -> bool {
        self.being_initialized
    }

    pub fn initializing(&mut self) {
        self.being_initialized = true;
    }

    pub fn has_super_class(&self) -> bool {
        self.class_file.super_class != 0
    }

    pub fn super_class(&self) -> Result<ClassIdentifier> {
        ClassIdentifier::from_path(
            self.class_file
                .constant_pool
                .class_name(&self.class_file.super_class)?,
        )
    }

    pub fn method(&self, name: &str, descriptor: &str) -> Result<&Method> {
        self.class_file.method(name, descriptor)
    }

    pub fn cp_item(&self, index: &CpIndex) -> Result<&CpInfo> {
        self.class_file.cp_item(index)
    }

    pub fn utf8(&self, index: &CpIndex) -> Result<&str> {
        self.class_file.constant_pool.utf8(index)
    }

    pub fn class_identifier(&self, index: &CpIndex) -> Result<ClassIdentifier> {
        ClassIdentifier::from_path(self.class_file.constant_pool.class_name(index)?)
    }

    pub fn name_and_type(&self, index: &CpIndex) -> Result<(&str, &str)> {
        self.class_file.constant_pool.name_and_type(index)
    }

    pub fn is_method_signature_polymorphic(&self, method: &Method) -> Result<bool> {
        self.class_file.is_method_signature_polymorphic(method)
    }

    pub fn method_descriptor(&self, method: &Method) -> Result<MethodDescriptor> {
        MethodDescriptor::new(self.utf8(&method.descriptor_index)?)
    }

    pub fn method_name(&self, method: &Method) -> Result<&str> {
        self.utf8(&method.name_index)
    }
}

#[derive(Clone)]
enum FieldValue {
    String(String),
    Integer(u32),
    Long(u64),
}
