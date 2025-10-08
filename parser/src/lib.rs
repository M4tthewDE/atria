use std::{
    fmt::Display,
    fs::File,
    io::{Cursor, Read},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use tracing::debug;
use zip::{ZipArchive, result::ZipError};

use crate::class::ClassFile;

pub mod class;
mod util;

/// Finds and parses class files.
#[derive(Default)]
pub struct Parser {}

impl Parser {
    pub fn parse(&self, identifier: &ClassIdentifier) -> Result<ClassFile> {
        debug!("parsing {identifier}");

        let bytes = Self::load_bytes(identifier)?;
        debug!("loaded {} bytes", bytes.len());

        ClassFile::new(&mut Cursor::new(bytes))
    }

    fn load_bytes(identifier: &ClassIdentifier) -> Result<Vec<u8>> {
        let java_home = std::env::var("JAVA_HOME").context("JAVA_HOME is not set")?;
        let java_home_path = PathBuf::from(java_home);

        debug!("using JAVA_HOME {java_home_path:?}");

        let jmods_path = java_home_path.join("jmods");
        Self::find_contents_in_jmods(&jmods_path, identifier)
    }

    fn find_contents_in_jmods(
        jmods_path: &PathBuf,
        identifier: &ClassIdentifier,
    ) -> Result<Vec<u8>> {
        debug!("loading jmods from {jmods_path:?}");

        for jmod_dir_entry in jmods_path.read_dir()? {
            let mut archive = ZipArchive::new(File::open(jmod_dir_entry?.path())?)?;

            let path_in_archive = format!(
                "classes/{}",
                identifier
                    .path()
                    .to_str()
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

/// Identifies a class using package and name
pub struct ClassIdentifier {
    package: String,
    name: String,
}

impl ClassIdentifier {
    fn path(&self) -> PathBuf {
        let mut path = PathBuf::new();
        for package in self.package.split('.') {
            path.push(package);
        }

        path.push(format!("{}.class", self.name));
        path
    }
}

impl Display for ClassIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.package, self.name)
    }
}

impl ClassIdentifier {
    pub fn new(package: String, name: String) -> Self {
        Self { package, name }
    }
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    use super::*;

    #[test]
    fn system() {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();
        let parser = Parser::default();
        let class_identifier = ClassIdentifier::new("java.lang".to_owned(), "System".to_owned());

        let _class = parser.parse(&class_identifier).unwrap();
    }
}
