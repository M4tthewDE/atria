use std::fmt::Debug;
use std::{collections::HashMap, fmt::Display, fs::File, path::PathBuf};

use anyhow::{Context, Result, bail};
use parser::class::{
    constant_pool::{CpIndex, CpInfo},
    field::Field,
    method::Method,
};
use tracing::{debug, instrument, trace};
use zip::ZipArchive;

use crate::{
    class::{Class, FieldValue, ReferenceValue},
    code::{Code, Instruction},
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
    stack::{FrameValue, Reference, Stack},
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
    main_class: ClassIdentifier,
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
            main_class,
        })
    }

    fn current_class_mut(&mut self) -> Result<&mut Class> {
        let current_class = self.stack.current_class()?;
        self.classes.get_mut(&current_class).context(format!(
            "current class {} is not initialized",
            current_class
        ))
    }

    fn current_class(&self) -> Result<Class> {
        let current_class = self.stack.current_class()?;
        self.classes
            .get(&current_class)
            .context(format!(
                "current class {} is not initialized",
                current_class
            ))
            .cloned()
    }

    fn class_mut(&mut self, identifier: &ClassIdentifier) -> Result<&mut Class> {
        self.classes
            .get_mut(identifier)
            .context(format!("class {identifier} is not initialized"))
    }

    fn class(&self, identifier: &ClassIdentifier) -> Result<Class> {
        self.classes
            .get(identifier)
            .context(format!("class {identifier} is not initialized"))
            .cloned()
    }

    pub fn run(&mut self) -> Result<()> {
        let main_class = &self.main_class.clone();
        self.initialize(main_class)?;
        bail!("TODO: run")
    }

    fn initialize(&mut self, identifier: &ClassIdentifier) -> Result<Class> {
        if let Some(c) = self.classes.get(identifier)
            && (c.initialized() || c.being_initialized())
        {
            return Ok(c.clone());
        }

        let class_file = self.class_loader.load(identifier)?;

        debug!("initializing {identifier:?}");

        let mut class = Class::new(identifier.clone(), class_file);
        class.initializing();
        class.initialize_fields()?;

        self.classes.insert(identifier.clone(), class.clone());

        if class.has_super_class() {
            let super_class_identifier = class.super_class()?;
            self.initialize(&super_class_identifier)?;
        }

        self.execute_clinit(&class)?;
        self.classes
            .get_mut(identifier)
            .context("class {identifier} not found")?
            .finished_initialization();
        debug!("initialized {identifier:?}");
        Ok(class)
    }

    fn execute_clinit(&mut self, class: &Class) -> Result<()> {
        if let Ok(clinit_method) = class.method("<clinit>", "()V") {
            debug!("executing <clinit> for {:?}", class.identifier());

            let code_bytes = clinit_method
                .code()
                .context("no code found for <clinit> method")?;
            let code = Code::new(code_bytes)?;
            debug!("{:?}", &code);
            self.stack.push(
                "<clinit>".to_string(),
                vec![],
                code,
                class.identifier().clone(),
            );
            self.execute()?;
            debug!("executed <clinit> for {:?}", class.identifier());
        }

        Ok(())
    }

    #[instrument(skip(self), fields(class = %self.stack.current_class()?))]
    fn execute(&mut self) -> Result<()> {
        loop {
            let instruction = self.stack.current_instruction()?;
            debug!("executing {instruction:?}");
            match instruction {
                Instruction::Ldc(index) => {
                    self.ldc(&index)?;
                }
                Instruction::InvokeVirtual(index) => self.invoke_virtual(&index)?,
                Instruction::InvokeStatic(index) => self.invoke_static(&index)?,
                Instruction::Iconst(val) => self.stack.push_operand(FrameValue::Int(val.into()))?,
                Instruction::Anewarray(index) => self.a_new_array(&index)?,
                Instruction::PutStatic(index) => self.put_static(&index)?,
                Instruction::Return => {
                    self.stack.pop()?;
                    break;
                }
            }
        }

        Ok(())
    }

    fn ldc(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class_mut()?;
        match current_class.cp_item(index)? {
            CpInfo::Class { name_index } => {
                let name = current_class.utf8(name_index)?;
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
        } = current_class.cp_item(index)?
        {
            let class_identifier = current_class.class_identifier(class_index)?;
            let (method_name, descriptor) = current_class.name_and_type(name_and_type_index)?;
            let method = self.resolve_method(&class_identifier, method_name, descriptor)?;
            let class = self.class(&class_identifier)?;

            if method.is_synchronized() {
                bail!("TODO: invokevirtual synchronized method");
            }

            if !method.is_native() {
                let descriptor = class.method_descriptor(&method)?;
                let operands = self.stack.pop_operands(descriptor.parameters.len() + 1)?;
                let method_name = class.method_name(&method)?.to_string();
                let code = Code::new(method.code().context("method {method_name} has no code")?)?;
                self.stack
                    .push(method_name, operands, code, class.identifier().clone());
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
        } = current_class.cp_item(index)?
        {
            (class_index, name_and_type_index)
        } else {
            bail!("no method reference at index {index:?}")
        };

        let class_identifier = current_class.class_identifier(class_index)?;
        let (method_name, descriptor) = current_class.name_and_type(name_and_type_index)?;

        let method = self.resolve_method(&class_identifier, method_name, descriptor)?;
        let class = self.class(&class_identifier)?;

        if !method.is_static() {
            bail!("method has to be static");
        }

        if method.is_abstract() {
            bail!("method cannot be static");
        }

        if method.is_synchronized() {
            bail!("TODO: invokestatic synchronized method");
        }

        let descriptor = class.method_descriptor(&method)?;

        if method.is_native() {
            let operands = self.stack.pop_operands(descriptor.parameters.len())?;
            self.run_native_method(method_name, operands)
        } else {
            bail!("TODO: invokestatic")
        }
    }

    fn a_new_array(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        let array_class = current_class.class_identifier(index)?;
        self.initialize(&array_class)?;
        let length = self.stack.pop_int()?;
        let array = FrameValue::ReferenceArray(array_class, vec![Reference::Null; length as usize]);
        self.stack.push_operand(array)
    }

    fn put_static(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        let (class_index, name_and_type_index) = if let CpInfo::FieldRef {
            class_index,
            name_and_type_index,
        } = current_class.cp_item(index)?
        {
            (class_index, name_and_type_index)
        } else {
            bail!("no field reference at index {index:?}")
        };

        let class = current_class.class_identifier(class_index)?;
        let (name, descriptor) = current_class.name_and_type(name_and_type_index)?;

        self.resolve_field(&class, name, descriptor)?;
        let value = self.stack.pop_operand()?;
        let class = self.class_mut(&class)?;
        class.set_field(name, value.into())
    }

    fn run_native_method(&self, name: &str, _operands: Vec<FrameValue>) -> Result<()> {
        trace!("running native method {name}");
        trace!("finished native method {name}");
        Ok(())
    }

    fn resolve_method(
        &mut self,
        class: &ClassIdentifier,
        name: &str,
        descriptor: &str,
    ) -> Result<Method> {
        let class = self.initialize(class)?;

        if let Ok(m) = class.method(name, descriptor) {
            if class.is_method_signature_polymorphic(m)? {
                bail!("TODO: method is signature polymorphic");
            }

            Ok(m.clone())
        } else {
            let super_class = class
                .super_class()
                .context("method not found, maybe check interfaces?")?;
            self.resolve_method(&super_class, name, descriptor)
        }
    }

    fn resolve_field(
        &mut self,
        class: &ClassIdentifier,
        name: &str,
        descriptor: &str,
    ) -> Result<Field> {
        let class = self.initialize(class)?;

        if let Ok(f) = class.field(name, descriptor) {
            Ok(f.clone())
        } else {
            let super_class = class
                .super_class()
                .context("field not found, maybe check interfaces?")?;
            self.resolve_field(&super_class, name, descriptor)
        }
    }
}

impl From<FrameValue> for FieldValue {
    fn from(value: FrameValue) -> Self {
        match value {
            FrameValue::ClassReference(class_identifier) => {
                Self::Reference(ReferenceValue::Class(class_identifier))
            }
            FrameValue::ReferenceArray(class_identifier, references) => {
                Self::Reference(ReferenceValue::Array(
                    class_identifier,
                    references.iter().map(|r| r.clone().into()).collect(),
                ))
            }
            FrameValue::Int(val) => Self::Integer(val),
        }
    }
}

impl From<Reference> for FieldValue {
    fn from(value: Reference) -> Self {
        match value {
            Reference::Null => Self::Reference(ReferenceValue::Null),
        }
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
        for package in self.package.split('.') {
            write!(f, "{}.", package.chars().next().unwrap_or_default())?;
        }

        write!(f, "{}", self.name)?;

        Ok(())
    }
}

impl Debug for ClassIdentifier {
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
            .with(fmt::layer().with_file(true).with_line_number(true))
            .with(EnvFilter::from_default_env())
            .init();

        let file = File::open("../spring-boot-demo/target/demo-0.0.1-SNAPSHOT.jar").unwrap();
        let mut jvm = Jvm::from_jar(file)?;
        jvm.run()
    }
}
