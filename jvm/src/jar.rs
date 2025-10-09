use anyhow::{Result, bail};
use std::io::{Read, Seek};

use zip::ZipArchive;

use crate::ClassIdentifier;

pub struct Jar<'a, R: Read + Seek> {
    archive: &'a mut ZipArchive<R>,
}

impl<'a, R: Read + Seek> Jar<'a, R> {
    pub fn new(archive: &'a mut ZipArchive<R>) -> Self {
        Self { archive }
    }

    pub(crate) fn manifest(&mut self) -> Result<Manifest> {
        Manifest::new(self.archive)
    }

    pub(crate) fn class(&mut self, identifier: &ClassIdentifier) -> Result<Vec<u8>> {
        let mut r = self.archive.by_name(&identifier.path()?)?;
        let mut contents = Vec::new();
        r.read_to_end(&mut contents)?;
        Ok(contents)
    }
}

pub(crate) struct Manifest {
    pub main_class: ClassIdentifier,
}

impl Manifest {
    fn new(archive: &mut ZipArchive<impl Read + Seek>) -> Result<Self> {
        let mut r = archive.by_name("META-INF/MANIFEST.MF")?;
        let mut contents = String::new();
        r.read_to_string(&mut contents)?;

        for line in contents.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts[0] == "Main-Class:" {
                return Ok(Self {
                    main_class: ClassIdentifier::new(parts[1].to_string())?,
                });
            }
        }

        bail!("unable to parse MANIFEST.MF")
    }
}
