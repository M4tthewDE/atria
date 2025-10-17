use std::collections::HashMap;

use crate::loader::ReadClass;
use anyhow::{Context, Result};
use rkyv::util::AlignedVec;

pub struct Jdk {
    classes: HashMap<String, Vec<u8>>,
}

static CLASSE_CACHE: &[u8] = include_bytes!("../../target/class_cache.bin");

impl Jdk {
    pub fn new() -> Result<Self> {
        let mut aligned: AlignedVec = AlignedVec::new();
        aligned.extend_from_slice(CLASSE_CACHE);
        let classes: HashMap<String, Vec<u8>> =
            rkyv::from_bytes::<HashMap<String, Vec<u8>>, rkyv::rancor::Error>(&aligned)?;
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
