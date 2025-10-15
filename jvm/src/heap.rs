use anyhow::Result;
use std::collections::HashMap;

use anyhow::Context;
use tracing::debug;

use crate::{ClassIdentifier, class::FieldValue};

#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub struct ObjectId(u64);

impl From<u64> for ObjectId {
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

#[derive(Default)]
pub struct Heap {
    current_id: u64,
    objects: HashMap<ObjectId, Object>,
}

impl Heap {
    pub fn allocate(
        &mut self,
        class_identifier: ClassIdentifier,
        fields: HashMap<String, FieldValue>,
    ) -> ObjectId {
        let object = Object::new(class_identifier, fields);
        let id: ObjectId = self.current_id.into();
        self.objects.insert(id.clone(), object.clone());
        self.current_id += 1;

        debug!("allocated {object:?} with id {id:?}");

        id
    }

    pub fn set_field(&mut self, object_id: &ObjectId, name: &str, value: FieldValue) -> Result<()> {
        let object = self
            .objects
            .get_mut(object_id)
            .context(format!("unknown object with {object_id:?}"))?;
        object
            .fields
            .insert(name.to_string(), value)
            .context(format!("field '{name}' not found on object {object:?}"))?;
        Ok(())
    }
}
