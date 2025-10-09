use std::{collections::HashMap, fmt::Display, fs::File, path::PathBuf};

use anyhow::{Context, Result, bail};
use parser::class::ClassFile;
use tracing::trace;
use zip::ZipArchive;

use crate::{
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
};

mod jar;
mod jdk;
mod loader;

pub struct Jvm {
    class_loader: BootstrapClassLoader,
    main_class: ClassIdentifier,

    class_files: HashMap<ClassIdentifier, ClassFile>,
}

impl Jvm {
    pub fn from_jar(file: File) -> Result<Self> {
        let archive = ZipArchive::new(file)?;
        let mut jar = Jar::new(archive);
        let main_class = jar.manifest()?.main_class;
        let sources: Vec<Box<dyn ReadClass>> = vec![Box::new(jar), Box::new(Jdk::new()?)];
        let class_loader = BootstrapClassLoader::new(sources);

        Ok(Self {
            class_loader,
            main_class,
            class_files: HashMap::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let class_file = self.load(&self.main_class.clone())?;
        self.initialize(&class_file)?;

        bail!("TODO: run")
    }

    fn load(&mut self, identifier: &ClassIdentifier) -> Result<ClassFile> {
        Ok(match self.class_files.get(identifier) {
            Some(cf) => {
                trace!("class has already been loaded, skipping");
                cf.clone()
            }
            None => {
                let class_file = self.class_loader.load(identifier)?;
                self.class_files
                    .insert(identifier.clone(), class_file.clone());
                class_file
            }
        })
    }

    fn initialize(&self, _class_file: &ClassFile) -> Result<()> {
        bail!("TODO: jvm initialize")
    }
}

/// Identifies a class using package and name
#[derive(Clone, Eq, Hash, PartialEq)]
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
