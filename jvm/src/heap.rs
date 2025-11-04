use anyhow::{Result, bail};
use common::ClassIdentifier;
use common::HeapId;
use common::ReferenceValue;
use std::collections::HashMap;

use anyhow::Context;
use tracing::debug;

use crate::class::FieldValue;

#[derive(Debug, Clone)]
pub struct InstanceField {
    offset: i64,
    value: FieldValue,
}

impl InstanceField {
    pub fn new(offset: i64, value: FieldValue) -> Self {
        Self { offset, value }
    }

    pub fn offset(&self) -> i64 {
        self.offset
    }
}

#[derive(Debug, Clone)]
pub struct Object {
    class_identifier: ClassIdentifier,
    fields: HashMap<String, InstanceField>,
}

impl Object {
    fn new(class_identifier: ClassIdentifier, fields: HashMap<String, InstanceField>) -> Self {
        Self {
            class_identifier,
            fields,
        }
    }

    pub fn class(&self) -> &ClassIdentifier {
        &self.class_identifier
    }
}

#[derive(Debug, Clone)]
pub enum HeapItem {
    Object(Object),
    ReferenceArray {
        object_id: HeapId,
        class: ClassIdentifier,
        values: Vec<ReferenceValue>,
    },
    PrimitiveArray(PrimitiveArrayType, Vec<PrimitiveArrayValue>),
}

impl HeapItem {
    pub fn is_array(&self) -> bool {
        match self {
            Self::Object(_) => false,
            Self::ReferenceArray { .. } | Self::PrimitiveArray(_, _) => true,
        }
    }

    pub fn class_identifier(&self) -> Result<ClassIdentifier> {
        Ok(match self {
            HeapItem::Object(object) => object.class_identifier.clone(),
            HeapItem::ReferenceArray { class, .. } => class.clone(),
            HeapItem::PrimitiveArray(array_type, _) => array_type.class_identifier(),
        })
    }

    pub fn object(&self) -> Result<&Object> {
        if let Self::Object(object) = self {
            Ok(object)
        } else {
            bail!("heap item is not a object, is {self:?}")
        }
    }
}

#[derive(Debug, Clone)]
pub enum PrimitiveArrayType {
    Boolean,
    Char,
    Float,
    Double,
    Byte,
    Short,
    Int,
    Long,
}

impl PrimitiveArrayType {
    pub fn new(atype: u8) -> Result<Self> {
        Ok(match atype {
            4 => PrimitiveArrayType::Boolean,
            5 => PrimitiveArrayType::Char,
            6 => PrimitiveArrayType::Float,
            7 => PrimitiveArrayType::Double,
            8 => PrimitiveArrayType::Byte,
            9 => PrimitiveArrayType::Short,
            10 => PrimitiveArrayType::Int,
            11 => PrimitiveArrayType::Long,
            _ => bail!("invalid array type: {atype}"),
        })
    }

    pub fn default(&self) -> PrimitiveArrayValue {
        match self {
            Self::Boolean => PrimitiveArrayValue::Boolean(false),
            Self::Char => PrimitiveArrayValue::Char(0),
            Self::Float => PrimitiveArrayValue::Float(0.0),
            Self::Double => PrimitiveArrayValue::Double(0.0),
            Self::Byte => PrimitiveArrayValue::Byte(0),
            Self::Short => PrimitiveArrayValue::Short(0),
            Self::Int => PrimitiveArrayValue::Int(0),
            Self::Long => PrimitiveArrayValue::Long(0),
        }
    }

    pub fn class_identifier(&self) -> ClassIdentifier {
        match self {
            PrimitiveArrayType::Boolean => {
                ClassIdentifier::new("java.lang".to_owned(), "Boolean".to_owned())
            }
            PrimitiveArrayType::Char => {
                ClassIdentifier::new("java.lang".to_owned(), "Character".to_owned())
            }
            PrimitiveArrayType::Float => {
                ClassIdentifier::new("java.lang".to_owned(), "Float".to_owned())
            }
            PrimitiveArrayType::Double => {
                ClassIdentifier::new("java.lang".to_owned(), "Double".to_owned())
            }
            PrimitiveArrayType::Byte => {
                ClassIdentifier::new("java.lang".to_owned(), "Byte".to_owned())
            }
            PrimitiveArrayType::Short => {
                ClassIdentifier::new("java.lang".to_owned(), "Short".to_owned())
            }
            PrimitiveArrayType::Int => {
                ClassIdentifier::new("java.lang".to_owned(), "Integer".to_owned())
            }
            PrimitiveArrayType::Long => {
                ClassIdentifier::new("java.lang".to_owned(), "Long".to_owned())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PrimitiveArrayValue {
    Boolean(bool),
    Char(u16),
    Float(f32),
    Double(f64),
    Byte(u8),
    Short(u16),
    Int(i32),
    Long(i64),
}

impl PrimitiveArrayValue {
    pub fn byte(&self) -> Result<u8> {
        if let Self::Byte(val) = self {
            Ok(*val)
        } else {
            bail!("value is not a byte, is {self:?}")
        }
    }
}

#[derive(Default)]
pub struct Heap {
    current_id: u64,
    items: HashMap<HeapId, HeapItem>,
}

impl Heap {
    pub fn allocate(
        &mut self,
        class_identifier: ClassIdentifier,
        fields: HashMap<String, InstanceField>,
    ) -> HeapId {
        let object = Object::new(class_identifier, fields);
        let heap_item = HeapItem::Object(object);
        let id: HeapId = self.current_id.into();
        self.items.insert(id.clone(), heap_item.clone());
        self.current_id += 1;

        debug!("allocated {heap_item:?} with id {id:?}");

        id
    }

    pub fn allocate_array(&mut self, class: ClassIdentifier, length: usize) -> HeapId {
        let heap_item = HeapItem::ReferenceArray {
            object_id: self.current_id.into(),
            class,
            values: vec![ReferenceValue::Null; length],
        };
        let id: HeapId = self.current_id.into();
        self.items.insert(id.clone(), heap_item.clone());
        self.current_id += 1;

        debug!("allocated {heap_item:?} with id {id:?}");
        id
    }

    pub fn allocate_primitive_array(
        &mut self,
        array_type: PrimitiveArrayType,
        values: Vec<PrimitiveArrayValue>,
    ) -> HeapId {
        let heap_item = HeapItem::PrimitiveArray(array_type, values);
        let id: HeapId = self.current_id.into();
        self.items.insert(id.clone(), heap_item.clone());
        self.current_id += 1;

        debug!("allocated {heap_item:?} with id {id:?}");
        id
    }

    pub fn allocate_default_primitive_array(
        &mut self,
        array_type: PrimitiveArrayType,
        count: usize,
    ) -> HeapId {
        let items = vec![array_type.default(); count];
        self.allocate_primitive_array(array_type, items)
    }

    pub fn set_field(&mut self, object_id: &HeapId, name: &str, value: FieldValue) -> Result<()> {
        let item = self
            .items
            .get_mut(object_id)
            .context(format!("unknown object with {object_id:?}"))?;
        if let HeapItem::Object(object) = item {
            let field = object
                .fields
                .get_mut(name)
                .context(format!("field '{name}' not found on object"))?;
            field.value = value;
            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn get_field(&self, id: &HeapId, name: &str) -> Result<FieldValue> {
        let item = self
            .items
            .get(id)
            .context(format!("unknown object with {id:?}"))?;

        match item {
            HeapItem::Object(object) => object
                .fields
                .get(name)
                .context("no field with name '{name}' found")
                .map(|f| f.value.clone()),
            _ => bail!("item at {id:?} is no object, but {item:?}"),
        }
    }

    pub fn get(&self, id: &HeapId) -> Result<&HeapItem> {
        self.items.get(id).context("no heap item at id {id}")
    }

    pub fn store_into_reference_array(
        &mut self,
        id: &HeapId,
        index: usize,
        value: ReferenceValue,
    ) -> Result<()> {
        let arr = self
            .items
            .get_mut(id)
            .context(format!("unknown object with {id:?}"))?;

        match arr {
            HeapItem::ReferenceArray { values, .. } => values[index] = value,
            _ => bail!("object at {id:?} is not a reference array, is {arr:?}"),
        }

        Ok(())
    }

    pub fn store_into_primitive_array(
        &mut self,
        id: &HeapId,
        index: usize,
        value: PrimitiveArrayValue,
    ) -> Result<()> {
        let arr = self
            .items
            .get_mut(id)
            .context(format!("unknown object with {id:?}"))?;

        match arr {
            HeapItem::PrimitiveArray(_, values) => values[index] = value,
            _ => bail!("object at {id:?} is not a reference array, is {arr:?}"),
        }

        Ok(())
    }

    pub fn get_reference_array(&self, id: &HeapId) -> Result<&Vec<ReferenceValue>> {
        let item = self
            .items
            .get(id)
            .context(format!("unknown object with {id:?}"))?;

        match item {
            HeapItem::ReferenceArray { values, .. } => Ok(values),
            _ => bail!("object at {id:?} is not a array, is {item:?}"),
        }
    }

    pub fn get_primitive_array(
        &self,
        id: &HeapId,
    ) -> Result<(&PrimitiveArrayType, &Vec<PrimitiveArrayValue>)> {
        let item = self
            .items
            .get(id)
            .context(format!("unknown object with {id:?}"))?;

        match item {
            HeapItem::PrimitiveArray(typ, values) => Ok((typ, values)),
            _ => bail!("object at {id:?} is not a array, is {item:?}"),
        }
    }

    pub fn get_array_length(&self, id: &HeapId) -> Result<usize> {
        let item = self
            .items
            .get(id)
            .context(format!("unknown object with {id:?}"))?;

        match item {
            HeapItem::PrimitiveArray(_, values) => Ok(values.len()),
            HeapItem::ReferenceArray { values, .. } => Ok(values.len()),
            _ => bail!("object at {id:?} is not a array, is {item:?}"),
        }
    }
}
