use std::collections::HashMap;

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
}
