use std::fmt::Debug;
use std::{collections::HashMap, fmt::Display, fs::File, path::PathBuf};

use anyhow::{Context, Result, bail};
use parser::class::descriptor::{FieldDescriptor, FieldType, MethodDescriptor, ReturnDescriptor};
use parser::class::{
    constant_pool::{CpIndex, CpInfo},
    field::Field,
    method::Method,
};
use tracing::{debug, instrument};
use zip::ZipArchive;

use crate::heap::{Heap, ObjectId};
use crate::{
    class::{Class, FieldValue},
    instruction::Instruction,
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
    stack::{FrameValue, Stack},
};

mod class;
mod heap;
mod instruction;
mod jar;
mod jdk;
mod loader;
mod stack;

pub struct Jvm {
    class_loader: BootstrapClassLoader,

    classes: HashMap<ClassIdentifier, Class>,
    stack: Stack,
    main_class: ClassIdentifier,
    heap: Heap,
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
            heap: Heap::default(),
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
            .context(format!("class {identifier:?} is not initialized"))
    }

    fn class(&self, identifier: &ClassIdentifier) -> Result<Class> {
        self.classes
            .get(identifier)
            .context(format!("class {identifier:?} is not initialized"))
            .cloned()
    }

    pub fn run(&mut self) -> Result<()> {
        self.initialize(&ClassIdentifier::new("java.lang.Class".to_string())?)?;
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

        debug!("initializing {identifier:?}");
        let class_file = self.class_loader.load(identifier)?;

        let mut class = Class::new(identifier.clone(), class_file);
        class.initializing();
        class.initialize_static_fields()?;

        let class_identifier = ClassIdentifier::new("java.lang.Class".to_string())?;
        if identifier != &class_identifier {
            let class_class = self.class(&class_identifier)?;
            for field in class_class.fields() {
                if field.is_static() {
                    continue;
                }

                let name = class_class.utf8(&field.name_index)?;
                let descriptor = class_class.utf8(&field.descriptor_index)?;
                class.set_class_field(name.to_string(), descriptor)?;
            }
        }

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
            let descriptor = class.method_descriptor(clinit_method)?;
            let code = clinit_method
                .code()
                .context("no code found for <clinit> method")?
                .to_vec();
            self.stack.push(
                "<clinit>".to_string(),
                descriptor,
                vec![],
                code,
                class.identifier().clone(),
            );
            self.execute()?;
            debug!("executed <clinit> for {:?}", class.identifier());
        }

        Ok(())
    }

    #[instrument(name = "", skip(self), fields(c = %self.stack.current_class()?))]
    fn execute(&mut self) -> Result<()> {
        debug!(
            "running {} {:?}",
            self.stack.method_name()?,
            self.stack.method_descriptor()?
        );
        loop {
            let instruction = self.stack.current_instruction()?;
            debug!("executing {instruction:?}");
            match instruction {
                Instruction::Ldc(ref index) | Instruction::LdcW(ref index) => {
                    self.ldc(index)?;
                }
                Instruction::InvokeVirtual(ref index) => self.invoke_virtual(index)?,
                Instruction::InvokeStatic(ref index) => self.invoke_static(index)?,
                Instruction::Iconst(val) => self.stack.push_operand(FrameValue::Int(val.into()))?,
                Instruction::Anewarray(ref index) => self.a_new_array(index)?,
                Instruction::PutStatic(ref index) => self.put_static(index)?,
                Instruction::Return => {
                    self.stack.pop()?;
                    break;
                }
                Instruction::Aload(index) => self.aload(index)?,
                Instruction::GetField(ref index) => self.get_field(index)?,
                Instruction::Astore(index) => self.astore(index)?,
                Instruction::IfNull(offset) => self.if_null(offset)?,
                Instruction::New(ref index) => self.new_instruction(index)?,
                Instruction::Dup => self.dup()?,
                Instruction::InvokeSpecial(ref index) => self.invoke_special(index)?,
                Instruction::Areturn => {
                    let object_ref = self.stack.pop_operand()?;
                    self.stack.pop()?;
                    self.stack.push_operand(object_ref)?;
                    break;
                }
                Instruction::InvokeDynamic(ref index) => self.invoke_dynamic(index)?,
                Instruction::IfNonNull(offset) => self.if_non_null(offset)?,
                Instruction::Ireturn => {
                    self.ireturn()?;
                    break;
                }
                Instruction::IfNe(offset) => self.if_ne(offset)?,
                Instruction::GetStatic(ref index) => self.get_static(index)?,
                Instruction::PutField(ref index) => self.put_field(index)?,
                _ => bail!("instruction {instruction:?} is not implemented"),
            }

            match instruction {
                Instruction::IfNull(_) | Instruction::IfNe(_) | Instruction::IfNonNull(_) => {}
                _ => self.stack.offset_pc(instruction.length() as i16)?,
            }
        }

        Ok(())
    }

    fn put_field(&mut self, index: &CpIndex) -> Result<()> {
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
        let object_ref = self.stack.pop_operand()?;

        if object_ref.is_array() || !object_ref.is_reference() {
            bail!("object ref has to be reference but not array, is {object_ref:?}")
        }

        if let FrameValue::Reference(ReferenceValue::Object(object_id)) = object_ref {
            self.heap.set_field(&object_id, name, value.into())
        } else {
            bail!("object ref has to be reference but not array, is {object_ref:?}")
        }
    }

    fn get_static(&mut self, index: &CpIndex) -> Result<()> {
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

        let (name, _) = current_class.name_and_type(name_and_type_index)?;
        let class_identifier = current_class.class_identifier(class_index)?;
        let class = self.class(&class_identifier)?;
        let field_value = class.get_field_value(name)?;
        self.stack.push_operand(field_value.into())
    }

    fn if_ne(&mut self, offset: i16) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        if let FrameValue::Int(val) = operand {
            if val == 0 {
                self.stack.offset_pc(offset)
            } else {
                self.stack.offset_pc(3)
            }
        } else {
            bail!("ifne operand has to be int, is {operand:?}")
        }
    }

    fn ireturn(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;

        if matches!(operand, FrameValue::Int(_)) {
            self.stack.pop()?;
            self.stack.push_operand(operand)
        } else {
            bail!("ireturn can only return int, is {operand:?}")
        }
    }

    fn ldc(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;

        let value = match current_class.cp_item(index)? {
            CpInfo::Class { name_index } => {
                let name = current_class.utf8(name_index)?;
                let identifier = ClassIdentifier::from_path(name)?;
                self.resolve_class(&identifier)?;

                FrameValue::Reference(ReferenceValue::Class(identifier))
            }
            CpInfo::String { string_index } => {
                let value = current_class.utf8(string_index)?;
                let object_id = self.new_string(value.to_string())?;
                FrameValue::Reference(ReferenceValue::Object(object_id))
            }
            info => bail!("item {info:?} at index {index:?} is not loadable"),
        };

        self.stack.push_operand(value)
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

            let descriptor = class.method_descriptor(&method)?;
            let operands = self.stack.pop_operands(descriptor.parameters.len() + 1)?;
            let method_name = class.method_name(&method)?.to_string();

            if !method.is_native() {
                let code = method
                    .code()
                    .context("method {method_name} has no code")?
                    .to_vec();
                self.stack.push(
                    method_name,
                    descriptor,
                    operands,
                    code,
                    class.identifier().clone(),
                );
                self.execute()
            } else if let Some(return_value) =
                self.run_native_method(&class, &method_name, operands)?
            {
                self.stack.push_operand(return_value)
            } else {
                Ok(())
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

        let operands = self.stack.pop_operands(descriptor.parameters.len())?;
        if method.is_native() {
            if let Some(return_value) = self.run_native_method(&class, method_name, operands)? {
                self.stack.push_operand(return_value)
            } else {
                Ok(())
            }
        } else {
            let code = method
                .code()
                .context("method {method_name} has no code")?
                .to_vec();
            self.stack.push(
                method_name.to_string(),
                descriptor,
                operands,
                code,
                class_identifier,
            );
            self.execute()
        }
    }

    fn a_new_array(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        let array_class = current_class.class_identifier(index)?;
        self.initialize(&array_class)?;
        let length = self.stack.pop_int()?;
        let array = FrameValue::Reference(ReferenceValue::Array(
            ArrayType::Reference(array_class),
            vec![ArrayValue::Reference(ReferenceValue::Null); length as usize],
        ));
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

    fn aload(&mut self, index: u8) -> Result<()> {
        let local_variable = self.stack.local_variable(index.into())?;

        if !local_variable.is_reference() {
            bail!("local variable has to be a reference, is {local_variable:?}")
        }

        self.stack.push_operand(local_variable)
    }

    fn resolve_class(&mut self, identifier: &ClassIdentifier) -> Result<Class> {
        self.initialize(identifier)
    }

    fn get_field(&mut self, index: &CpIndex) -> Result<()> {
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

        let class_identifier = current_class.class_identifier(class_index)?;
        let (name, descriptor) = current_class.name_and_type(name_and_type_index)?;

        self.resolve_field(&class_identifier, name, descriptor)?;
        let object_ref = self.stack.pop_operand()?;
        if !object_ref.is_reference() || object_ref.is_array() {
            bail!("objectref has to be a reference but no array, is {object_ref:?}");
        }

        if let FrameValue::Reference(ReferenceValue::Class(identifier)) = object_ref {
            let class = self.class(&identifier)?;
            let field_value = class.get_field_value(name)?;
            self.stack.push_operand(field_value.into())
        } else {
            bail!("TODO: get_field for non class references")
        }
    }

    fn astore(&mut self, index: u8) -> Result<()> {
        let objectref = self.stack.pop_operand()?;
        if !objectref.is_reference() && !objectref.is_return_address() {
            bail!("TODO: astore objectref has to be reference or return address")
        }

        self.stack.set_local_variable(index.into(), objectref)
    }

    fn if_null(&mut self, offset: i16) -> Result<()> {
        let value = self.stack.pop_operand()?;
        if !value.is_reference() {
            bail!("ifnull value has to be reference, is {value:?}");
        }

        if value.is_null() {
            self.stack.offset_pc(offset)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_non_null(&mut self, offset: i16) -> Result<()> {
        let value = self.stack.pop_operand()?;
        if !value.is_reference() {
            bail!("ifnonnull value has to be reference, is {value:?}");
        }

        if !value.is_null() {
            self.stack.offset_pc(offset)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn new_instruction(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;

        let class_identifier = current_class.class_identifier(index)?;
        let class = self.resolve_class(&class_identifier)?;
        let fields = self.default_instance_fields(&class)?;
        let object_id = self.heap.allocate(class.identifier().clone(), fields);
        self.stack
            .push_operand(FrameValue::Reference(ReferenceValue::Object(object_id)))
    }

    fn dup(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        self.stack.push_operand(operand.clone())?;
        self.stack.push_operand(operand)
    }

    fn invoke_special(&mut self, index: &CpIndex) -> Result<()> {
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
        let class = self.initialize(&class_identifier)?;
        let method_descriptor = class.method_descriptor(&method)?;

        if !Self::is_instance_initialization_method(method_name, &method_descriptor) {
            bail!("TODO: invokespecial for non instance initialization methods")
        }

        if class.contains_method(&method) {
            if method.is_synchronized() {
                bail!("TODO: invokespecial synchronized method")
            }

            if method.is_native() {
                bail!("TODO: invokespecial native method")
            }

            let operands = self
                .stack
                .pop_operands(method_descriptor.parameters.len() + 1)?;
            let code = method
                .code()
                .context(format!("no code found for {method_name} method"))?
                .to_vec();
            self.stack.push(
                method_name.to_string(),
                method_descriptor,
                operands,
                code,
                class.identifier().clone(),
            );
            self.execute()
        } else {
            bail!("TODO: invokespecial method lookup")
        }
    }

    fn invoke_dynamic(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        let (bootstrap_method_attr_index, name_and_type_index) = if let CpInfo::InvokeDynamic {
            bootstrap_method_attr_index,
            name_and_type_index,
        } =
            current_class.cp_item(index)?
        {
            (bootstrap_method_attr_index, name_and_type_index)
        } else {
            bail!("no invoke dynamic item at index {index:?}")
        };

        self.resolve_dynamically_computed(bootstrap_method_attr_index, name_and_type_index)?;
        bail!("TODO: invokedynamic")
    }

    fn resolve_dynamically_computed(
        &mut self,
        bootstrap_method_attr_index: &CpIndex,
        name_and_type_index: &CpIndex,
    ) -> Result<()> {
        let method_handle =
            self.resolve_method_handle(bootstrap_method_attr_index, name_and_type_index)?;
        bail!("TODO: callsite resolution")
    }

    fn resolve_method_handle(
        &mut self,
        bootstrap_method_attr_index: &CpIndex,
        name_and_type_index: &CpIndex,
    ) -> Result<ObjectId> {
        let current_class = self.current_class()?;

        let (name, descriptor) = current_class.name_and_type(name_and_type_index)?;
        let method_descriptor = MethodDescriptor::new(descriptor)?;
        if let ReturnDescriptor::FieldType(FieldType::ObjectType { class_name }) =
            method_descriptor.return_descriptor
        {
            self.initialize(&ClassIdentifier::from_path(&class_name)?)?;
        }

        for parameter in method_descriptor.parameters {
            if let FieldType::ObjectType { class_name } = parameter {
                self.initialize(&ClassIdentifier::from_path(&class_name)?)?;
            }
        }

        let method_type_identifier =
            ClassIdentifier::new("java.lang.invoke.MethodType".to_string())?;
        let class = self.resolve_class(&method_type_identifier)?;

        bail!("TODO: method handle resolution")
    }

    fn is_instance_initialization_method(name: &str, descriptor: &MethodDescriptor) -> bool {
        name == "<init>" && descriptor.is_void()
    }

    fn run_native_method(
        &mut self,
        class: &Class,
        name: &str,
        operands: Vec<FrameValue>,
    ) -> Result<Option<FrameValue>> {
        debug!(
            "running native method '{name}' in {:?} with operands {:?}",
            class.identifier(),
            operands
        );

        if class.identifier() == &ClassIdentifier::new("java.lang.Class".to_string())? {
            match name {
                "registerNatives" => Ok(None),
                "initClassName" => {
                    if let FrameValue::Reference(ReferenceValue::Class(identifier)) =
                        operands.first().context("no operands provided")?
                    {
                        let object_id = self.new_string(format!("{identifier:?}").to_string())?;
                        Ok(Some(FrameValue::Reference(ReferenceValue::Object(
                            object_id,
                        ))))
                    } else {
                        bail!("first operand has to be a reference")
                    }
                }
                "desiredAssertionStatus0" => Ok(Some(FrameValue::Int(0))),
                _ => bail!("native method not implemented"),
            }
        } else if class.identifier() == &ClassIdentifier::new("java.lang.Runtime".to_string())? {
            match name {
                "availableProcessors" => {
                    let cpus = std::thread::available_parallelism()?;
                    Ok(Some(FrameValue::Int(cpus.get().try_into()?)))
                }
                _ => bail!("native method not implemented"),
            }
        } else {
            bail!("native method not implemented")
        }
    }

    fn new_string(&mut self, value: String) -> Result<ObjectId> {
        let string_identifier = ClassIdentifier::new("java.lang.String".to_string())?;
        let class = self.resolve_class(&string_identifier)?;

        let fields = self.default_instance_fields(&class)?;
        let object_id = self.heap.allocate(class.identifier().clone(), fields);
        let bytes = value
            .into_bytes()
            .iter()
            .map(|b| ArrayValue::Byte(*b))
            .collect();
        let byte_array = FrameValue::Reference(ReferenceValue::Array(ArrayType::Byte, bytes));
        self.heap
            .set_field(&object_id, "value", byte_array.into())?;
        Ok(object_id)
    }

    fn default_instance_fields(&mut self, class: &Class) -> Result<HashMap<String, FieldValue>> {
        let mut fields = HashMap::new();
        for field in class.fields() {
            if field.is_static() {
                continue;
            }

            let field_name = class.utf8(&field.name_index)?;
            let descriptor = class.utf8(&field.descriptor_index)?;
            fields.insert(
                field_name.to_string(),
                FieldDescriptor::new(descriptor)?.into(),
            );

            if class.has_super_class() {
                let super_class = self.initialize(&class.super_class()?)?;
                let super_class_fields = self.default_instance_fields(&super_class)?;
                fields.extend(super_class_fields.into_iter());
            }
        }

        Ok(fields)
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
            FrameValue::Reference(reference_value) => Self::Reference(reference_value),
            FrameValue::Int(val) => Self::Integer(val),
            FrameValue::ReturnAddress => todo!(),
        }
    }
}

impl From<FieldValue> for FrameValue {
    fn from(value: FieldValue) -> Self {
        match value {
            FieldValue::Reference(reference_value) => Self::Reference(reference_value),
            FieldValue::String(_) => todo!(),
            FieldValue::Integer(_) => todo!(),
            FieldValue::Long(_) => todo!(),
            FieldValue::Float(_) => todo!(),
            FieldValue::Double(_) => todo!(),
        }
    }
}

/// Identifies a class using package and name
#[derive(Clone, Eq, Hash, PartialEq)]
struct ClassIdentifier {
    is_array: bool,
    package: String,
    name: String,
}

impl ClassIdentifier {
    fn new(value: String) -> Result<Self> {
        if let Ok(descriptor) = FieldDescriptor::new(&value) {
            if let FieldType::ComponentType(field_type) = descriptor.field_type {
                match *field_type {
                    FieldType::ObjectType { class_name } => Self::new(class_name),
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

    // TODO: this could be merged into new
    fn from_path(path: &str) -> Result<Self> {
        Self::new(path.replace("/", "."))
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

#[derive(Debug, Clone)]
enum ReferenceValue {
    Object(ObjectId),
    Class(ClassIdentifier),
    Array(ArrayType, Vec<ArrayValue>),
    Null,
}

#[derive(Debug, Clone)]
enum ArrayType {
    Reference(ClassIdentifier),
    Byte,
}

#[derive(Debug, Clone)]
enum ArrayValue {
    Reference(ReferenceValue),
    Byte(u8),
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
