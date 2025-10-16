use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use parser::class::{
    ClassFile,
    access_flags::AccessFlag,
    constant_pool::{CpIndex, CpInfo},
    descriptor::{BaseType, FieldDescriptor, FieldType, MethodDescriptor},
    field::Field,
    method::Method,
};
use tracing::trace;

use crate::{ClassIdentifier, ReferenceValue, heap::HeapId};

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

    // TODO: rename to default
    pub fn set_class_field(&mut self, name: String, descriptor: &str) -> Result<()> {
        self.fields
            .insert(name, FieldDescriptor::new(descriptor)?.into());
        Ok(())
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
        ClassIdentifier::new(
            self.class_file
                .constant_pool
                .class_name(&self.class_file.super_class)?,
        )
    }

    pub fn super_interfaces(&self) -> Result<Vec<ClassIdentifier>> {
        Ok(self
            .class_file
            .interfaces
            .iter()
            .map(|i| self.class_identifier(i))
            .collect::<Result<Vec<ClassIdentifier>>>()?)
    }

    pub fn method(&self, name: &str, descriptor: &str) -> Result<&Method> {
        self.class_file.method(name, descriptor)
    }

    pub fn field(&self, name: &str, descriptor: &str) -> Result<&Field> {
        self.class_file.field(name, descriptor)
    }

    pub fn cp_item(&self, index: &CpIndex) -> Result<&CpInfo> {
        self.class_file.cp_item(index)
    }

    pub fn utf8(&self, index: &CpIndex) -> Result<&str> {
        self.class_file.constant_pool.utf8(index)
    }

    pub fn class_identifier(&self, index: &CpIndex) -> Result<ClassIdentifier> {
        ClassIdentifier::new(self.class_file.constant_pool.class_name(index)?)
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

    pub fn set_static_field(&mut self, name: &str, value: FieldValue) -> Result<()> {
        trace!("setting field {name} to value {value:?}");
        self.fields.insert(name.to_string(), value);
        Ok(())
    }

    pub fn get_static_field_value(&self, name: &str) -> Result<FieldValue> {
        self.fields
            .get(name)
            .context(format!("field {name} not found in {:?}", self.identifier))
            .cloned()
    }

    pub fn fields(&self) -> &Vec<Field> {
        &self.class_file.fields
    }

    pub fn contains_method(&self, method: &Method) -> bool {
        self.class_file.methods.contains(method)
    }

    pub fn is_interface(&self) -> bool {
        self.class_file
            .access_flags
            .contains(&AccessFlag::Interface)
    }
}

#[derive(Clone, Debug)]
pub enum FieldValue {
    Reference(ReferenceValue),
    Integer(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl FieldValue {
    pub fn heap_id(&self) -> Result<&HeapId> {
        match self {
            FieldValue::Reference(reference_value) => reference_value.heap_id(),
            _ => bail!("no heap id found"),
        }
    }
}

impl From<FieldDescriptor> for FieldValue {
    fn from(value: FieldDescriptor) -> Self {
        match value.field_type {
            FieldType::BaseType(base_type) => match base_type {
                BaseType::Byte => Self::Integer(0),
                BaseType::Char => Self::Integer(0),
                BaseType::Double => Self::Double(0.0),
                BaseType::Float => Self::Float(0.0),
                BaseType::Int => Self::Integer(0),
                BaseType::Long => Self::Long(0),
                BaseType::Short => Self::Integer(0),
                BaseType::Boolean => Self::Integer(0),
            },
            FieldType::ObjectType { .. } => Self::Reference(ReferenceValue::Null),
            FieldType::ComponentType(..) => Self::Reference(ReferenceValue::Null),
        }
    }
}
