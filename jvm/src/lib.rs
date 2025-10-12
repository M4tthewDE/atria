use std::{collections::HashMap, fmt::Display, fs::File, path::PathBuf};

use anyhow::{Context, Result, bail};
use parser::class::{
    ClassFile,
    constant_pool::{CpIndex, CpInfo},
    method::Method,
};
use tracing::debug;
use zip::ZipArchive;

use crate::{
    class::Class,
    code::{Code, Instruction},
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
    stack::{FrameValue, Stack},
};

mod class;
mod code;
mod jar;
mod jdk;
mod loader;
mod stack;

pub struct Jvm {
    class_loader: BootstrapClassLoader,

    classes: HashMap<ClassIdentifier, Class>,
    stack: Stack,
    current_code: Option<Code>,
    current_class: ClassIdentifier,
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
            classes: HashMap::new(),
            stack: Stack::default(),
            current_code: None,
            current_class: main_class,
        })
    }

    fn current_class_mut(&mut self) -> Result<&mut Class> {
        self.classes.get_mut(&self.current_class).context(format!(
            "current class {} is not initialized",
            self.current_class
        ))
    }

    fn current_class(&self) -> Result<Class> {
        self.classes
            .get(&self.current_class)
            .context(format!(
                "current class {} is not initialized",
                self.current_class
            ))
            .cloned()
    }

    pub fn run(&mut self) -> Result<()> {
        let identifier = self.current_class.clone();

        self.initialize(&identifier)?;

        bail!("TODO: run")
    }

    fn initialize(&mut self, identifier: &ClassIdentifier) -> Result<Class> {
        if let Some(c) = self.classes.get(identifier)
            && (c.initialized || c.being_initialized)
        {
            return Ok(c.clone());
        }

        let class_file = self.class_loader.load(identifier)?;

        debug!("initializing {identifier}");
        let mut class = Class::new(identifier.clone(), class_file);
        class.being_initialized = true;
        class.initialize_fields()?;

        self.classes.insert(identifier.clone(), class.clone());

        if class.class_file.super_class != 0 {
            let name = class
                .class_file
                .constant_pool
                .class_name(&class.class_file.super_class)?;
            let identifier = ClassIdentifier::from_path(name)?;
            self.initialize(&identifier)?;
        }

        self.execute_clinit(&class)?;
        self.classes
            .get_mut(identifier)
            .context("class {identifier} not found")?
            .initialized = true;
        debug!("initialized {identifier}");
        Ok(class)
    }

    fn execute_clinit(&mut self, class: &Class) -> Result<()> {
        if let Ok(clinit_method) = class.class_file.method("<clinit>", "()V") {
            debug!("executing <clinit> for {}", class.identifier);

            self.stack.push("<clinit>".to_string(), vec![]);

            let code_bytes = clinit_method
                .code()
                .context("no code found for <clinit> method")?;
            let code = Code::new(code_bytes)?;
            debug!("{:?}", &code);
            self.current_code = Some(code);
            self.current_class = class.identifier.clone();
            self.execute()?;
        }

        Ok(())
    }

    fn execute(&mut self) -> Result<()> {
        let instructions = self
            .current_code
            .clone()
            .context("no code to run")?
            .instructions;
        for instruction in instructions {
            match instruction {
                Instruction::Ldc(index) => {
                    self.ldc(&index)?;
                }
                Instruction::InvokeVirtual(index) => self.invoke_virtual(&index)?,
                Instruction::InvokeStatic(index) => self.invoke_static(&index)?,
                _ => bail!("instruction {instruction:?} is not supported"),
            }
        }

        Ok(())
    }

    fn ldc(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class_mut()?;
        match current_class.class_file.cp_item(index)? {
            CpInfo::Class { name_index } => {
                let name = current_class.class_file.constant_pool.utf8(name_index)?;
                let identifier = ClassIdentifier::from_path(name)?;
                self.class_loader.load(&identifier)?;

                self.stack
                    .push_operand(FrameValue::ClassReference(identifier))
            }
            info => bail!("item {info:?} at index {index:?} is not loadable"),
        }
    }

    fn invoke_virtual(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        if let CpInfo::MethodRef {
            class_index,
            name_and_type_index,
        } = current_class.class_file.cp_item(index)?
        {
            let class_name = current_class
                .class_file
                .constant_pool
                .class_name(class_index)?;

            let class_identifier = ClassIdentifier::from_path(class_name)?;
            let class = self.initialize(&class_identifier)?;

            let (method_name, descriptor) = current_class
                .class_file
                .constant_pool
                .name_and_type(name_and_type_index)?;
            let method = self.resolve_method(&class.class_file, method_name, descriptor)?;

            if method.is_synchronized() {
                bail!("TODO: invokevirtual synchronized method");
            }

            if !method.is_native() {
                let operands = self.stack.pop_operands(
                    method
                        .descriptor(&class.class_file.constant_pool)?
                        .parameters
                        .len()
                        + 1,
                )?;
                let method_name = method.name(&class.class_file.constant_pool)?.to_string();
                self.stack.push(method_name, operands);
                let code = Code::new(method.code().context("method {method_name} has no code")?)?;
                self.current_code = Some(code);
                self.current_class = class.identifier.clone();
                self.execute()
            } else {
                bail!("TODO: invokevirtual native method");
            }
        } else {
            bail!("no method reference at index {index:?}")
        }
    }

    fn invoke_static(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        let (class_index, name_and_type_index) = if let CpInfo::MethodRef {
            class_index,
            name_and_type_index,
        } = current_class.class_file.cp_item(index)?
        {
            (class_index, name_and_type_index)
        } else {
            bail!("no method reference at index {index:?}")
        };

        let class_name = current_class
            .class_file
            .constant_pool
            .class_name(class_index)?;
        let class_identifier = ClassIdentifier::from_path(class_name)?;
        let class = self.initialize(&class_identifier)?;
        let (method_name, descriptor) = current_class
            .class_file
            .constant_pool
            .name_and_type(name_and_type_index)?;

        let method = self.resolve_method(&class.class_file, method_name, descriptor)?;

        if !method.is_static() {
            bail!("method has to be static");
        }

        if method.is_abstract() {
            bail!("method cannot be static");
        }

        if method.is_synchronized() {
            bail!("TODO: invokestatic synchronized method");
        }

        let descriptor = method.descriptor(&class.class_file.constant_pool)?;

        if method.is_native() {
            let operands = self.stack.pop_operands(descriptor.parameters.len())?;
            self.run_native_method(method_name, operands)
        } else {
            bail!("TODO: invokestatic")
        }
    }

    fn run_native_method(&self, name: &str, _operands: Vec<FrameValue>) -> Result<()> {
        debug!("running native method {name}");
        Ok(())
    }

    fn resolve_method(
        &mut self,
        class_file: &ClassFile,
        name: &str,
        descriptor: &str,
    ) -> Result<Method> {
        if let Ok(m) = class_file.method(name, descriptor) {
            if class_file.is_method_signature_polymorphic(m)? {
                bail!("TODO: method is signature polymorphic");
            }

            Ok(m.clone())
        } else {
            let super_class = ClassIdentifier::from_path(class_file.super_class()?)?;
            let class_file = self.class_loader.load(&super_class)?;
            self.resolve_method(&class_file, name, descriptor)
        }
    }
}

/// Identifies a class using package and name
#[derive(Clone, Eq, Hash, PartialEq, Debug)]
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
