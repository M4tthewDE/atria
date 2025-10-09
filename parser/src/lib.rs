use std::io::{Read, Seek};

use anyhow::Result;
use tracing::debug;

use crate::class::ClassFile;

pub mod class;
mod util;

pub fn parse(r: &mut (impl Read + Seek)) -> Result<ClassFile> {
    debug!("parsing class");
    ClassFile::new(r)
}
