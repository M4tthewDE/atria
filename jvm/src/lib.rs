use std::{
    fmt::Display,
    io::{Cursor, Read, Seek},
    path::PathBuf,
};

use anyhow::{Context, Result};
use zip::ZipArchive;

use crate::jar::Jar;

mod jar;

#[derive(Default)]
pub struct Jvm {}

impl Jvm {
    pub fn run(&self, r: impl Read + Seek) -> Result<()> {
        let mut archive = ZipArchive::new(r)?;
        let mut jar = Jar::new(&mut archive);
        let manifest = jar.manifest()?;
        let main_class = jar.class(&manifest.main_class)?;
        parser::parse(&mut Cursor::new(main_class))?;

        Ok(())
    }
}

/// Identifies a class using package and name
struct ClassIdentifier {
    package: String,
    name: String,
}

impl ClassIdentifier {
    fn path(&self) -> Result<String> {
        let mut path = PathBuf::new();
        for package in self.package.split('.') {
            path.push(package);
        }

        path.push(format!("{}.class", self.name));
        path.to_str()
            .map(|p| p.to_owned())
            .clone()
            .context("unable to build path string")
    }
}

impl Display for ClassIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.package, self.name)
    }
}

impl ClassIdentifier {
    fn new(value: String) -> Result<Self> {
        let mut parts: Vec<&str> = value.split('.').collect();
        let name = parts
            .last()
            .context("invalid class identifier {value}")?
            .to_string();
        parts.truncate(parts.len() - 1);

        Ok(Self {
            package: parts.join("."),
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    use super::*;

    #[test]
    fn system() {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();

        let mut file = File::open("../spring-boot-demo/target/demo-0.0.1-SNAPSHOT.jar").unwrap();
        let jvm = Jvm::default();
        jvm.run(&mut file).unwrap();
    }
}
