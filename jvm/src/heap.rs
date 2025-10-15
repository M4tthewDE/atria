use anyhow::{Result, bail};
use std::collections::HashMap;

use anyhow::Context;
use tracing::debug;

use crate::{ClassIdentifier, ReferenceValue, class::FieldValue};

#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub struct HeapId(u64);

impl From<u64> for HeapId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub struct Object {
    class_identifier: ClassIdentifier,
    fields: HashMap<String, FieldValue>,
}

impl Object {
    fn new(class_identifier: ClassIdentifier, fields: HashMap<String, FieldValue>) -> Self {
        Self {
            class_identifier,
            fields,
        }
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
    PrimitiveArray(Vec<PrimitiveArrayValue>),
}

impl HeapItem {
    pub fn is_array(&self) -> bool {
        match self {
            Self::Object(_) => false,
            Self::ReferenceArray { .. } | Self::PrimitiveArray(_) => true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PrimitiveArrayValue {
    Byte(u8),
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
        fields: HashMap<String, FieldValue>,
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

    pub fn allocate_primitive_array(&mut self, values: Vec<PrimitiveArrayValue>) -> HeapId {
        let heap_item = HeapItem::PrimitiveArray(values);
        let id: HeapId = self.current_id.into();
        self.items.insert(id.clone(), heap_item.clone());
        self.current_id += 1;

        debug!("allocated {heap_item:?} with id {id:?}");
        id
    }

    pub fn set_field(&mut self, object_id: &HeapId, name: &str, value: FieldValue) -> Result<()> {
        let item = self
            .items
            .get_mut(object_id)
            .context(format!("unknown object with {object_id:?}"))?;
        if let HeapItem::Object(object) = item {
            object
                .fields
                .insert(name.to_string(), value)
                .context(format!("field '{name}' not found on object {object:?}"))?;
            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn get_field(&mut self, id: &HeapId, name: &str) -> Result<FieldValue> {
        let item = self
            .items
            .get(id)
            .context(format!("unknown object with {id:?}"))?;

        match item {
            HeapItem::Object(object) => object
                .fields
                .get(name)
                .context("no field with name '{name}' found")
                .cloned(),
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
            HeapItem::ReferenceArray { values, .. } => values.insert(index, value),
            _ => bail!("object at {id:?} is not a reference array, is {arr:?}"),
        }

        Ok(())
    }

    pub fn get_primitive_array(&self, id: &HeapId) -> Result<&Vec<PrimitiveArrayValue>> {
        let item = self
            .items
            .get(id)
            .context(format!("unknown object with {id:?}"))?;

        match item {
            HeapItem::PrimitiveArray(primitive_array_values) => Ok(primitive_array_values),
            _ => bail!("object at {id:?} is not a primitive array, is {item:?}"),
        }
    }
}
