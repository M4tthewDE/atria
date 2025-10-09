use std::io::{Read, Seek};

use anyhow::Result;

use crate::class::ClassFile;

pub mod class;
mod util;

pub fn parse(r: &mut (impl Read + Seek)) -> Result<ClassFile> {
    ClassFile::new(r)
}
