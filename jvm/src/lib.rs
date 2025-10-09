use std::{fmt::Display, fs::File, path::PathBuf};

use anyhow::{Context, Result, bail};
use zip::ZipArchive;

use crate::{jar::Jar, loader::BootstrapClassLoader};

mod jar;
mod loader;

pub struct Jvm {
    class_loader: BootstrapClassLoader,
    main_class: ClassIdentifier,
}

impl Jvm {
    pub fn from_jar(file: File) -> Result<Self> {
        let archive = ZipArchive::new(file)?;
        let mut jar = Jar::new(archive);
        let main_class = jar.manifest()?.main_class;
        let class_loader = BootstrapClassLoader::new(jar);

        Ok(Self {
            class_loader,
            main_class,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.load(&self.main_class.clone())?;
        bail!("TODO: run")
    }

    fn load(&mut self, identifier: &ClassIdentifier) -> Result<()> {
        let _class = self.class_loader.load(identifier)?;

        bail!("TODO: jvm load")
    }
}

/// Identifies a class using package and name
#[derive(Clone)]
struct ClassIdentifier {
    package: String,
    name: String,
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

    fn from_path(path: &str) -> Result<Self> {
        let mut parts: Vec<&str> = path.split('/').collect();
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

    fn with_slashes(&self) -> Result<String> {
        let mut path = PathBuf::new();
        for package in self.package.split('.') {
            path.push(package);
        }

        path.push(&self.name);
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

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    use super::*;

    #[test]
    fn system() -> Result<()> {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();

        let file = File::open("../spring-boot-demo/target/demo-0.0.1-SNAPSHOT.jar").unwrap();
        let mut jvm = Jvm::from_jar(file)?;
        jvm.run()
    }
}
