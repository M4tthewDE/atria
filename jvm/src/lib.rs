use std::{collections::HashMap, fmt::Display, fs::File, path::PathBuf};

use anyhow::{Context, Result, bail};
use tracing::debug;
use zip::ZipArchive;

use crate::{
    class::Class,
    code::Code,
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
};

mod class;
mod code;
mod jar;
mod jdk;
mod loader;

pub struct Jvm {
    class_loader: BootstrapClassLoader,
    main_class: ClassIdentifier,

    classes: HashMap<ClassIdentifier, Class>,
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
            classes: HashMap::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let identifier = self.main_class.clone();

        self.initialize(&identifier)?;

        bail!("TODO: run")
    }

    fn initialize(&mut self, identifier: &ClassIdentifier) -> Result<()> {
        if self.classes.contains_key(identifier) {
            return Ok(());
        }

        let class_file = self.class_loader.load(identifier)?;

        debug!("initializing {identifier}");
        let mut class = Class::new(identifier.clone(), class_file);
        class.initialize_fields()?;

        if class.class_file.super_class != 0 {
            let name = class
                .class_file
                .constant_pool
                .class_name(&class.class_file.super_class)?;
            let identifier = ClassIdentifier::from_path(name)?;
            self.initialize(&identifier)?;
        }

        self.execute_clinit(&mut class)?;
        self.classes.insert(identifier.clone(), class);
        debug!("initialized {identifier}");
        Ok(())
    }

    fn execute_clinit(&mut self, class: &mut Class) -> Result<()> {
        if let Some(clinit_method) = class.class_file.clinit() {
            debug!("executing <clinit> for {}", class.identifier);

            let code_bytes = clinit_method
                .code()
                .context("no code found for <clinit> method")?;
            let code = Code::new(code_bytes)?;
            debug!("{:?}", &code);
            self.execute(&code)?;
        }

        Ok(())
    }

    fn execute(&self, code: &Code) -> Result<()> {
        for instruction in &code.instructions {
            match instruction {
                _ => bail!("instruction {instruction:?} is not supported"),
            }
        }

        Ok(())
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
