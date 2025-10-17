use std::collections::HashMap;

use crate::loader::ReadClass;
use anyhow::{Context, Result};

pub struct Jdk {
    classes: HashMap<String, Vec<u8>>,
}

impl Jdk {
    pub fn new() -> Result<Self> {
        let classes = jdk::classes()?;
        Ok(Self { classes })
    }
}

impl ReadClass for Jdk {
    fn read_class(&mut self, identifier: &crate::ClassIdentifier) -> Result<Vec<u8>> {
        self.classes
            .get(&identifier.path()?)
            .context("class not found")
            .cloned()
    }
}
