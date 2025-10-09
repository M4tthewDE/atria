use std::{fs::File, io::Read, path::PathBuf};

use crate::loader::ReadClass;
use anyhow::{Context, Result, bail};
use tracing::debug;
use zip::{ZipArchive, result::ZipError};

pub struct Jdk {
    java_home_path: PathBuf,
}

impl Jdk {
    pub fn new() -> Result<Self> {
        let java_home = std::env::var("JAVA_HOME").context("JAVA_HOME is not set")?;
        let java_home_path = PathBuf::from(java_home);
        Ok(Self { java_home_path })
    }
}

impl ReadClass for Jdk {
    fn read_class(&mut self, identifier: &crate::ClassIdentifier) -> Result<Vec<u8>> {
        let jmods_path = self.java_home_path.join("jmods");
        debug!("loading jmods from {jmods_path:?}");

        for jmod_dir_entry in jmods_path.read_dir()? {
            let jmod_path = jmod_dir_entry?.path();

            let mut archive = ZipArchive::new(File::open(&jmod_path)?)?;

            let path_in_archive = format!(
                "classes/{}",
                identifier
                    .path()
                    .context("unable to build archive path string")?
            );

            match archive.by_name(&path_in_archive) {
                Ok(mut reader) => {
                    let mut contents = Vec::new();
                    reader.read_to_end(&mut contents)?;
                    return Ok(contents);
                }

                Err(err) => match err {
                    ZipError::FileNotFound => continue,
                    err => return Err(err.into()),
                },
            }
        }

        bail!("class {identifier} not found");
    }
}
