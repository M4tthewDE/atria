use anyhow::{Result, bail};
use std::{fs::File, io::Read};

use zip::ZipArchive;

use crate::{ClassIdentifier, loader::ReadClass};

pub struct Jar {
    archive: ZipArchive<File>,
}

impl ReadClass for Jar {
    fn read_class(&mut self, identifier: &ClassIdentifier) -> Result<Vec<u8>> {
        let mut r = self.archive.by_name(&identifier.path()?)?;
        let mut contents = Vec::new();
        r.read_to_end(&mut contents)?;
        Ok(contents)
    }
}

impl Jar {
    pub fn new(archive: ZipArchive<File>) -> Self {
        Self { archive }
    }

    pub(crate) fn manifest(&mut self) -> Result<Manifest> {
        Manifest::new(&mut self.archive)
    }
}

pub(crate) struct Manifest {
    pub main_class: ClassIdentifier,
}

impl Manifest {
    fn new(archive: &mut ZipArchive<File>) -> Result<Self> {
        let mut r = archive.by_name("META-INF/MANIFEST.MF")?;
        let mut contents = String::new();
        r.read_to_string(&mut contents)?;

        for line in contents.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts[0] == "Main-Class:" {
                return Ok(Self {
                    main_class: ClassIdentifier::new(parts[1])?,
                });
            }
        }

        bail!("unable to parse MANIFEST.MF")
    }
}
