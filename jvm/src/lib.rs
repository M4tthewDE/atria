use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, fmt::Display, fs::File, path::PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use parser::class::descriptor::{BaseType, FieldDescriptor, FieldType};
use zip::ZipArchive;

use crate::heap::{Heap, HeapId};
use crate::monitor::Monitors;
use crate::thread::JvmThread;
use crate::{
    class::{Class, FieldValue},
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
    stack::FrameValue,
};

pub mod class;
mod code;
pub mod heap;
pub mod instruction;
pub mod jar;
pub mod jdk;
pub mod loader;
mod monitor;
pub mod stack;
pub mod thread;

pub struct Jvm {
    class_loader: Arc<Mutex<BootstrapClassLoader>>,
    classes: Arc<Mutex<HashMap<ClassIdentifier, Class>>>,
    main_class: ClassIdentifier,
    heap: Arc<Mutex<Heap>>,
    monitors: Arc<Mutex<Monitors>>,
}

impl Jvm {
    pub fn from_jar(file: File) -> Result<Self> {
        let archive = ZipArchive::new(file)?;
        let mut jar = Jar::new(archive);
        let main_class = jar.manifest()?.main_class;
        let sources: Vec<Box<dyn ReadClass>> = vec![Box::new(jar), Box::new(Jdk::new()?)];
        let class_loader = Arc::new(Mutex::new(BootstrapClassLoader::new(sources)));

        Ok(Self {
            class_loader,
            classes: Arc::new(Mutex::new(HashMap::new())),
            main_class,
            heap: Arc::new(Mutex::new(Heap::default())),
            monitors: Arc::new(Mutex::new(Monitors::default())),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let main_thread = JvmThread::new(
            "main".to_string(),
            self.class_loader.clone(),
            self.classes.clone(),
            self.heap.clone(),
            self.monitors.clone(),
        );

        let main_handle = JvmThread::run_with_class(main_thread, self.main_class.clone());
        main_handle
            .join()
            .map_err(|err| anyhow!("thread error: {err:?}"))??;
        bail!("TODO: After main thread exits")
    }
}

impl From<FrameValue> for FieldValue {
    fn from(value: FrameValue) -> Self {
        match value {
            FrameValue::Reference(reference_value) => Self::Reference(reference_value),
            FrameValue::Int(val) => Self::Integer(val),
            FrameValue::Long(val) => Self::Long(val),
            FrameValue::Float(val) => Self::Float(val),
            FrameValue::Double(val) => Self::Double(val),
            FrameValue::Reserved => panic!("impossible"),
        }
    }
}

impl From<FieldValue> for FrameValue {
    fn from(value: FieldValue) -> Self {
        match value {
            FieldValue::Reference(reference_value) => Self::Reference(reference_value),
            FieldValue::Integer(val) => Self::Int(val),
            FieldValue::Long(val) => Self::Long(val),
            FieldValue::Float(val) => Self::Float(val),
            FieldValue::Double(val) => Self::Double(val),
        }
    }
}

/// Identifies a class using package and name
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct ClassIdentifier {
    is_array: bool,
    package: String,
    name: String,
}

impl ClassIdentifier {
    fn new(value: &str) -> Result<Self> {
        let value = value.replace("/", ".");
        if let Ok(descriptor) = FieldDescriptor::new(&value) {
            if let FieldType::ComponentType(field_type) = descriptor.field_type {
                match *field_type {
                    FieldType::ObjectType { class_name } => Self::new(&class_name),
                    FieldType::BaseType(base_type) => match base_type {
                        BaseType::Byte => Self::new("java.lang.Byte"),
                        BaseType::Char => Self::new("java.lang.Character"),
                        BaseType::Double => Self::new("java.lang.Double"),
                        BaseType::Float => Self::new("java.lang.Float"),
                        BaseType::Int => Self::new("java.lang.Integer"),
                        BaseType::Long => Self::new("java.lang.Long"),
                        BaseType::Short => Self::new("java.lang.Short"),
                        BaseType::Boolean => Self::new("java.lang.Boolean"),
                    },
                    _ => bail!("invalid array class: {value}"),
                }
            } else {
                bail!("invalid array class: {value}")
            }
        } else {
            let mut parts: Vec<&str> = value.split('.').collect();
            let name = parts
                .last()
                .context("invalid class identifier {value}")?
                .to_string();
            parts.truncate(parts.len() - 1);

            Ok(Self {
                is_array: false,
                package: parts.join("."),
                name,
            })
        }
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
        write!(f, "{}", self.name)
    }
}

impl Debug for ClassIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.package, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum ReferenceValue {
    HeapItem(HeapId),
    Class(ClassIdentifier),
    Null,
}

impl ReferenceValue {
    pub fn heap_id(&self) -> Result<&HeapId> {
        match self {
            ReferenceValue::HeapItem(heap_id) => Ok(heap_id),
            _ => bail!("no heap id found"),
        }
    }

    pub fn class_identifier(&self) -> Result<&ClassIdentifier> {
        match self {
            ReferenceValue::Class(class_identifier) => Ok(class_identifier),
            _ => bail!("no class identifier found"),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_class(&self) -> bool {
        matches!(self, Self::Class(_))
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
