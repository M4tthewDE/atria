use anyhow::{Result, bail};
use parser::class::{ClassFile, field::AccessFlag};

use crate::ClassIdentifier;

#[derive(Clone)]
pub struct Class {
    identifier: ClassIdentifier,
    pub class_file: ClassFile,
}

impl Class {
    pub fn new(identifier: ClassIdentifier, class_file: ClassFile) -> Self {
        Self {
            identifier,
            class_file,
        }
    }

    pub fn initialize_fields(&mut self) -> Result<()> {
        for field in &self.class_file.fields {
            if field.access_flags.contains(&AccessFlag::Static)
                && field.access_flags.contains(&AccessFlag::Final)
            {
                bail!("TODO: initialize field");
            }
        }

        Ok(())
    }
}
