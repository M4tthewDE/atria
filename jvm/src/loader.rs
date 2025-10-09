use std::io::Cursor;

use crate::ClassIdentifier;
use anyhow::{Result, bail};

pub trait ReadClass {
    fn read_class(&mut self, identifier: &ClassIdentifier) -> Result<Vec<u8>>;
}

pub struct ClassLoader {
    sources: Vec<Box<dyn ReadClass>>,
}

impl ClassLoader {
    pub fn new(source: impl ReadClass + 'static) -> Self {
        Self {
            sources: vec![Box::new(source)],
        }
    }

    pub fn load(&mut self, identifier: &ClassIdentifier) -> Result<()> {
        for source in &mut self.sources {
            let class_bytes = source.read_class(identifier)?;
            let _class_file = parser::parse(&mut Cursor::new(class_bytes))?;
            bail!("TODO: load")
        }

        bail!("class {identifier} not found")
    }
}
