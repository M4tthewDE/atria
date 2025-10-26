use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use parser::class::ClassFile;
use parser::class::descriptor::{FieldDescriptor, FieldType, MethodDescriptor, ReturnDescriptor};
use parser::class::{
    constant_pool::{CpIndex, CpInfo},
    field::Field,
    method::Method,
};
use tracing::{debug, error, info, instrument, trace, warn};

use crate::code::Code;
use crate::heap::{Heap, HeapId, HeapItem, InstanceField, PrimitiveArrayType, PrimitiveArrayValue};
use crate::monitor::Monitors;
use crate::{ClassIdentifier, ReferenceValue};
use crate::{
    class::{Class, FieldValue},
    instruction::Instruction,
    loader::BootstrapClassLoader,
    stack::{FrameValue, Stack},
};

#[derive(Debug, PartialEq, Clone)]
pub struct ThreadId(i64);

impl From<i64> for ThreadId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

pub struct JvmThread {
    name: String,
    class_loader: Arc<Mutex<BootstrapClassLoader>>,
    classes: Arc<Mutex<HashMap<ClassIdentifier, Class>>>,
    heap: Arc<Mutex<Heap>>,
    monitors: Arc<Mutex<Monitors>>,

    stack: Stack,
    creation_time: Instant,
    current_thread_object: Option<HeapId>,
    current_thread_id: Option<ThreadId>,
}

impl JvmThread {
    pub fn new(
        name: String,
        class_loader: Arc<Mutex<BootstrapClassLoader>>,
        classes: Arc<Mutex<HashMap<ClassIdentifier, Class>>>,
        heap: Arc<Mutex<Heap>>,
        monitors: Arc<Mutex<Monitors>>,
    ) -> Self {
        Self {
            name,
            class_loader,
            classes,
            heap,
            monitors,
            stack: Stack::default(),
            creation_time: Instant::now(),
            current_thread_object: None,
            current_thread_id: None,
        }
    }

    pub fn run_with_class(mut thread: Self, main_class: ClassIdentifier) -> JoinHandle<Result<()>> {
        std::thread::spawn(move || match thread.run_main(&main_class) {
            Ok(_) => Ok(()),
            Err(err) => {
                error!(
                    "thread '{}' has crashed: {err:?} at\n{}",
                    thread.name,
                    thread.stack.stack_trace()
                );
                Err(err)
            }
        })
    }

    pub fn run_with_method(
        mut thread: Self,
        class: ClassIdentifier,
        name: String,
        descriptor: String,
    ) {
        std::thread::spawn(
            move || match thread.run_method(&class, &name, &descriptor) {
                Ok(_) => {
                    info!("thread '{}' has exited normally", thread.name)
                }
                Err(err) => error!("thread '{}' has crashed: {err:?}", thread.name),
            },
        );
    }

    #[instrument(name = "", skip_all, fields(t = self.name))]
    pub fn run_main(&mut self, main_class: &ClassIdentifier) -> Result<()> {
        self.initialize(&ClassIdentifier::new("java.lang.Class")?)?;
        self.initialize(&ClassIdentifier::new("java.lang.Object")?)?;
        let thread_object_heap_id =
            self.new_thread_object(self.name.to_string(), "system".to_string())?;
        self.current_thread_object = Some(thread_object_heap_id);
        self.initialize(main_class)?;
        bail!("TODO: run_main")
    }

    #[instrument(name = "", skip_all, fields(t = self.name))]
    fn run_method(
        &mut self,
        class_identifier: &ClassIdentifier,
        name: &str,
        descriptor: &str,
    ) -> Result<()> {
        let (_, method) = self.resolve_method(class_identifier, name, descriptor)?;
        let class = self.class(class_identifier)?;
        let descriptor = class.method_descriptor(&method)?;
        let code = method
            .code()
            .context(format!("method {name} has no code"))?;
        self.stack.push(
            name.to_string(),
            descriptor,
            vec![],
            Code::new(code.clone())?,
            class_identifier.clone(),
            None,
        );
        self.execute()
    }

    fn maybe_class(&self, identifier: &ClassIdentifier) -> Result<Option<Class>> {
        let classes = self
            .classes
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        Ok(classes.get(identifier).cloned())
    }

    fn class(&self, identifier: &ClassIdentifier) -> Result<Class> {
        let classes = self
            .classes
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        classes
            .get(identifier)
            .context(format!("class {identifier:?} is not initialized"))
            .cloned()
    }

    fn insert_class(&self, identifier: ClassIdentifier, class: Class) -> Result<()> {
        let mut classes = self
            .classes
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        classes.insert(identifier, class);
        Ok(())
    }

    fn current_class(&self) -> Result<Class> {
        let current_class = self.stack.current_class()?;
        self.class(&current_class)
    }

    fn heap_get(&self, heap_id: &HeapId) -> Result<HeapItem> {
        let heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        heap.get(heap_id).cloned()
    }

    pub fn get_primitive_array(
        &self,
        id: &HeapId,
    ) -> Result<(PrimitiveArrayType, Vec<PrimitiveArrayValue>)> {
        let heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        let (typ, arr) = heap.get_primitive_array(id)?;
        Ok((typ.clone(), arr.clone()))
    }

    pub fn get_reference_array(&self, id: &HeapId) -> Result<Vec<ReferenceValue>> {
        let heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        heap.get_reference_array(id).cloned()
    }

    pub fn get_array_length(&self, id: &HeapId) -> Result<usize> {
        let heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        heap.get_array_length(id)
    }

    pub fn allocate_primitive_array(
        &mut self,
        array_type: PrimitiveArrayType,
        values: Vec<PrimitiveArrayValue>,
    ) -> Result<HeapId> {
        let mut heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        Ok(heap.allocate_primitive_array(array_type, values))
    }

    pub fn store_into_primitive_array(
        &mut self,
        id: &HeapId,
        index: usize,
        value: PrimitiveArrayValue,
    ) -> Result<()> {
        let mut heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        heap.store_into_primitive_array(id, index, value)
    }

    pub fn store_into_reference_array(
        &mut self,
        id: &HeapId,
        index: usize,
        value: ReferenceValue,
    ) -> Result<()> {
        let mut heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        heap.store_into_reference_array(id, index, value)
    }

    pub fn allocate(
        &mut self,
        class_identifier: ClassIdentifier,
        fields: HashMap<String, InstanceField>,
    ) -> Result<HeapId> {
        let mut heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;

        Ok(heap.allocate(class_identifier, fields))
    }

    pub fn allocate_array(&mut self, class: ClassIdentifier, length: usize) -> Result<HeapId> {
        let mut heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        Ok(heap.allocate_array(class, length))
    }

    pub fn heap_get_field(&self, id: &HeapId, name: &str) -> Result<FieldValue> {
        let heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        heap.get_field(id, name)
    }

    pub fn heap_set_field(
        &mut self,
        object_id: &HeapId,
        name: &str,
        value: FieldValue,
    ) -> Result<()> {
        let mut heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        heap.set_field(object_id, name, value)
    }

    pub fn allocate_default_primitive_array(
        &mut self,
        array_type: PrimitiveArrayType,
        count: usize,
    ) -> Result<HeapId> {
        let mut heap = self
            .heap
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        Ok(heap.allocate_default_primitive_array(array_type, count))
    }

    fn load(&self, identifier: &ClassIdentifier) -> Result<ClassFile> {
        let mut loader = self
            .class_loader
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        loader.load(identifier)
    }

    fn enter_object_monitor(&mut self, heap_id: &HeapId, thread_id: &ThreadId) -> Result<bool> {
        let mut monitors = self
            .monitors
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        Ok(monitors.enter_object_monitor(heap_id, thread_id))
    }

    fn exit_object_monitor(&mut self, heap_id: &HeapId, thread_id: &ThreadId) -> Result<()> {
        let mut monitors = self
            .monitors
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        monitors.exit_object_monitor(heap_id, thread_id)
    }

    fn enter_class_monitor(
        &mut self,
        class_identifier: &ClassIdentifier,
        thread_id: &ThreadId,
    ) -> Result<bool> {
        let mut monitors = self
            .monitors
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        Ok(monitors.enter_class_monitor(class_identifier, thread_id))
    }

    fn exit_class_monitor(
        &mut self,
        class_identifier: &ClassIdentifier,
        thread_id: &ThreadId,
    ) -> Result<()> {
        let mut monitors = self
            .monitors
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        monitors.exit_class_monitor(class_identifier, thread_id)
    }

    fn initialize(&mut self, identifier: &ClassIdentifier) -> Result<Class> {
        if let Some(c) = self.maybe_class(identifier)?
            && (c.initialized() || c.being_initialized())
        {
            return Ok(c.clone());
        }

        info!("initializing {identifier:?}");
        let class_file = self.load(identifier)?;

        let mut class = Class::new(identifier.clone(), class_file);
        class.initializing();
        self.initialize_static_fields(&mut class)?;

        let class_identifier = ClassIdentifier::new("java.lang.Class")?;
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

        self.insert_class(identifier.clone(), class.clone())?;

        if class.has_super_class() {
            let super_class_identifier = class.super_class()?;
            self.initialize(&super_class_identifier)?;
        }

        self.execute_clinit(&class)?;
        if identifier == &ClassIdentifier::new("java.lang.System")? {
            let (_, method) = self.resolve_method(identifier, "initPhase1", "()V")?;
            let class = self.class(identifier)?;
            let descriptor = class.method_descriptor(&method)?;

            let operands = self.stack.pop_operands(descriptor.parameters.len())?;
            let code = method.code().context("method {method_name} has no code")?;
            self.stack.push(
                "initPhase1".to_string(),
                descriptor,
                operands,
                Code::new(code.clone())?,
                identifier.clone(),
                None,
            );
            self.execute()?;
        }
        let mut classes = self
            .classes
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        classes
            .get_mut(identifier)
            .context(format!("class {identifier:?} is not initialized"))?
            .finished_initialization();

        info!("initialized {identifier:?}");
        Ok(class)
    }

    pub fn initialize_static_fields(&mut self, class: &mut Class) -> Result<()> {
        for field in &class.fields().clone() {
            if field.is_static() {
                self.initialize_static_final_field(class, field)?;
            }
        }

        Ok(())
    }

    pub fn initialize_static_final_field(
        &mut self,
        class: &mut Class,
        field: &Field,
    ) -> Result<()> {
        let name = class.utf8(&field.name_index)?.to_string();

        trace!("initializing field {name}");

        let field_value = if let Some(constant_value_index) = field.get_constant_value_index() {
            self.resolve_constant_value(class, constant_value_index)?
        } else {
            FieldDescriptor::new(class.utf8(&field.descriptor_index)?)?.into()
        };

        class.set_static_field(&name, field_value)
    }

    fn resolve_constant_value(
        &mut self,
        class: &Class,
        constant_value_index: &CpIndex,
    ) -> Result<FieldValue> {
        Ok(match class.cp_item(constant_value_index)? {
            CpInfo::String { string_index } => {
                let value = class.utf8(string_index)?;
                let heap_id = self.new_string(value.to_string())?;
                FieldValue::Reference(ReferenceValue::HeapItem(heap_id))
            }
            CpInfo::Integer(val) => FieldValue::Integer(*val),
            CpInfo::Long(val) => FieldValue::Long(*val),
            CpInfo::Float(val) => FieldValue::Float(*val),
            CpInfo::Double(val) => FieldValue::Double(*val),
            item => bail!("invalid constant pool item: {item:?}"),
        })
    }

    fn execute_clinit(&mut self, class: &Class) -> Result<()> {
        if let Ok(clinit_method) = class.method("<clinit>", "()V") {
            let descriptor = class.method_descriptor(clinit_method)?;
            let code = clinit_method
                .code()
                .context("no code found for <clinit> method")?;
            self.stack.push(
                "<clinit>".to_string(),
                descriptor,
                vec![],
                Code::new(code.clone())?,
                class.identifier().clone(),
                None,
            );
            info!("running <clinit> for {:?}", class.identifier());
            self.execute()?;
        }

        Ok(())
    }

    #[instrument(level = "debug", name = "", skip(self), fields(c = %self.stack.current_class()?))]
    fn execute(&mut self) -> Result<()> {
        info!(
            "running {} {:?} in {:?}",
            self.stack.method_name()?,
            self.stack.local_variables()?,
            self.stack.current_class()?,
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
                    self.handle_synchronized_return()?;
                    self.stack.pop()?;
                    break;
                }
                Instruction::Aload(index) => self.aload(index)?,
                Instruction::Aload0 => self.aload(0)?,
                Instruction::Aload1 => self.aload(1)?,
                Instruction::Aload2 => self.aload(2)?,
                Instruction::Aload3 => self.aload(3)?,
                Instruction::GetField(ref index) => self.get_field(index)?,
                Instruction::Astore(index) => self.astore(index)?,
                Instruction::IfNull(offset) => self.if_null(offset)?,
                Instruction::New(ref index) => self.new_instruction(index)?,
                Instruction::Dup => self.dup()?,
                Instruction::Dup2 => self.dup2()?,
                Instruction::InvokeSpecial(ref index) => self.invoke_special(index)?,
                Instruction::Areturn => {
                    self.handle_synchronized_return()?;
                    let object_ref = self.stack.pop_operand()?;
                    self.stack.pop()?;
                    info!("returning {object_ref:?}");
                    self.stack.push_operand(object_ref)?;
                    break;
                }
                Instruction::Dreturn => {
                    self.handle_synchronized_return()?;
                    let double = self.stack.pop_operand()?;
                    self.stack.pop()?;
                    info!("returning {double:?}");
                    self.stack.push_operand(double)?;
                    break;
                }
                Instruction::InvokeDynamic(ref index) => self.invoke_dynamic(index)?,
                Instruction::IfNonNull(offset) => self.if_non_null(offset)?,
                Instruction::Ireturn => {
                    self.handle_synchronized_return()?;
                    self.ireturn()?;
                    break;
                }
                Instruction::IfNe(offset) => self.if_ne(offset)?,
                Instruction::GetStatic(ref index) => self.get_static(index)?,
                Instruction::PutField(ref index) => self.put_field(index)?,
                Instruction::Iload(index) => self.iload(index)?,
                Instruction::AconstNull => self
                    .stack
                    .push_operand(FrameValue::Reference(ReferenceValue::Null))?,
                Instruction::Aastore => self.aastore()?,
                Instruction::Bipush(value) => {
                    self.stack.push_operand(FrameValue::Int(value.into()))?
                }
                Instruction::Newarray(atype) => self.new_array(atype)?,
                Instruction::Castore => self.castore()?,
                Instruction::Bastore => self.bastore()?,
                Instruction::Iastore => self.iastore()?,
                Instruction::Sipush(value) => {
                    self.stack.push_operand(FrameValue::Int(value.into()))?
                }
                Instruction::Lreturn => {
                    self.handle_synchronized_return()?;
                    self.lreturn()?;
                    break;
                }
                Instruction::Istore(index) => self.istore(index)?,
                Instruction::Isub => self.isub()?,
                Instruction::Lsub => self.lsub()?,
                Instruction::Iand => self.iand()?,
                Instruction::Land => self.land()?,
                Instruction::Ifeq(offset) => self.if_eq(offset)?,
                Instruction::Goto(offset) => self.stack.offset_pc(offset as i32)?,
                Instruction::Ifgt(offset) => self.if_gt(offset)?,
                Instruction::Fload0 => self.fload(0)?,
                Instruction::Fload1 => self.fload(1)?,
                Instruction::Fload2 => self.fload(2)?,
                Instruction::Fload3 => self.fload(3)?,
                Instruction::Iload0 => self.iload(0)?,
                Instruction::Iload1 => self.iload(1)?,
                Instruction::Iload2 => self.iload(2)?,
                Instruction::Iload3 => self.iload(3)?,
                Instruction::Fconst(val) => self.stack.push_operand(FrameValue::Float(val))?,
                Instruction::Fcmpl => self.fcmpl()?,
                Instruction::Fcmpg => self.fcmpg()?,
                Instruction::Ifle(offset) => self.if_le(offset)?,
                Instruction::Iflt(offset) => self.if_lt(offset)?,
                Instruction::IfIcmpge(offset) => self.if_icmpge(offset)?,
                Instruction::Dconst(val) => self.stack.push_operand(FrameValue::Double(val))?,
                Instruction::I2l => self.i2l()?,
                Instruction::I2f => self.i2f()?,
                Instruction::L2f => self.l2f()?,
                Instruction::Fdiv => self.fdiv()?,
                Instruction::F2d => self.f2d()?,
                Instruction::F2i => self.f2i()?,
                Instruction::Dadd => self.dadd()?,
                Instruction::Fadd => self.fadd()?,
                Instruction::D2l => self.d2l()?,
                Instruction::Lstore(index) => self.lstore(index)?,
                Instruction::Fstore(index) => self.fstore(index)?,
                Instruction::Lload(index) => self.lload(index)?,
                Instruction::Fload(index) => self.fload(index)?,
                Instruction::Ldc2W(ref index) => self.ldc2_w(index)?,
                Instruction::Lcmp => self.lcmp()?,
                Instruction::L2i => self.l2i()?,
                Instruction::IfIcmplt(offset) => self.if_icmplt(offset)?,
                Instruction::Iinc(index, constant) => self.iinc(index as usize, constant)?,
                Instruction::Iushr => self.iushr()?,
                Instruction::Lushr => self.lushr()?,
                Instruction::Ifge(offset) => self.if_ge(offset)?,
                Instruction::Iadd => self.iadd()?,
                Instruction::Ladd => self.ladd()?,
                Instruction::Lconst(value) => self.stack.push_operand(FrameValue::Long(value))?,
                Instruction::IfIcmpeq(offset) => self.if_icmpeq(offset)?,
                Instruction::ArrayLength => self.array_length()?,
                Instruction::Ishr => self.ishr()?,
                Instruction::Lshr => self.lshr()?,
                Instruction::Lshl => self.lshl()?,
                Instruction::Ishl => self.ishl()?,
                Instruction::Baload => self.baload()?,
                Instruction::Aaload => self.aaload()?,
                Instruction::I2c => self.i2c()?,
                Instruction::I2b => self.i2b()?,
                Instruction::IfIcmpne(offset) => self.if_icmpne(offset)?,
                Instruction::IfIcmpgt(offset) => self.if_icmpgt(offset)?,
                Instruction::IfIcmple(offset) => self.if_icmple(offset)?,
                Instruction::IfAcmpne(offset) => self.if_acmpne(offset)?,
                Instruction::IfAcmpeq(offset) => self.if_acmpeq(offset)?,
                Instruction::Instanceof(ref index) => self.instance_of(index)?,
                Instruction::Checkcast(ref index) => self.check_cast(index)?,
                Instruction::Lstore0 => self.lstore(0)?,
                Instruction::Lstore1 => self.lstore(1)?,
                Instruction::Lstore2 => self.lstore(2)?,
                Instruction::Lstore3 => self.lstore(3)?,
                Instruction::Istore0 => self.istore(0)?,
                Instruction::Istore1 => self.istore(1)?,
                Instruction::Istore2 => self.istore(2)?,
                Instruction::Istore3 => self.istore(3)?,
                Instruction::Astore0 => self.astore(0)?,
                Instruction::Astore1 => self.astore(1)?,
                Instruction::Astore2 => self.astore(2)?,
                Instruction::Astore3 => self.astore(3)?,
                Instruction::Lload0 => self.lload(0)?,
                Instruction::Lload1 => self.lload(1)?,
                Instruction::Lload2 => self.lload(2)?,
                Instruction::Lload3 => self.lload(3)?,
                Instruction::Lmul => self.lmul()?,
                Instruction::Imul => self.imul()?,
                Instruction::Fmul => self.fmul()?,
                Instruction::InvokeInterface(ref index, count) => {
                    self.invoke_interface(index, count)?
                }
                Instruction::Pop => self.pop()?,
                Instruction::Ixor => self.ixor()?,
                Instruction::DupX1 => self.dup_x1()?,
                Instruction::MonitorEnter => self.monitor_enter()?,
                Instruction::MonitorExit => self.monitor_exit()?,
                Instruction::Irem => self.irem()?,
                Instruction::Idiv => self.idiv()?,
                Instruction::Ineg => self.ineg()?,
                Instruction::TableSwitch {
                    default,
                    low,
                    high,
                    ref jump_offsets,
                    ..
                } => self.table_switch(default, low, high, jump_offsets)?,
                Instruction::LookupSwitch {
                    default,
                    ref offset_pairs,
                    ..
                } => self.lookup_switch(default, offset_pairs)?,
            }

            // TODO: this is very brittle
            match instruction {
                Instruction::IfNull(_)
                | Instruction::IfNe(_)
                | Instruction::IfNonNull(_)
                | Instruction::Ifeq(_)
                | Instruction::Ifle(_)
                | Instruction::Iflt(_)
                | Instruction::Ifgt(_)
                | Instruction::IfIcmpge(_)
                | Instruction::IfIcmpgt(_)
                | Instruction::IfIcmple(_)
                | Instruction::IfIcmplt(_)
                | Instruction::Ifge(_)
                | Instruction::IfIcmpeq(_)
                | Instruction::IfIcmpne(_)
                | Instruction::IfAcmpne(_)
                | Instruction::Goto(_) => {}
                Instruction::TableSwitch { .. } => {}
                Instruction::LookupSwitch { .. } => {}
                _ => self.stack.offset_pc(instruction.length() as i32)?,
            }
        }

        Ok(())
    }

    fn lookup_switch(&mut self, default: i32, offset_pairs: &[(i32, i32)]) -> Result<()> {
        let key = self.stack.pop_operand()?.int()?;

        for (index, offset) in offset_pairs {
            if *index == key {
                return self.stack.offset_pc(*offset);
            }
        }

        self.stack.offset_pc(default)
    }

    fn table_switch(
        &mut self,
        default: i32,
        low: i32,
        high: i32,
        jump_offsets: &[i32],
    ) -> Result<()> {
        let index = self.stack.pop_operand()?.int()?;
        if index < low || index > high {
            self.stack.offset_pc(default)
        } else {
            self.stack.offset_pc(
                *jump_offsets
                    .get(index as usize - low as usize)
                    .context(format!("no jump offset at index {index} found"))?,
            )
        }
    }

    fn ineg(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?.int()?;
        self.stack.push_operand(FrameValue::Int(-value))
    }

    fn idiv(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        self.stack.push_operand(FrameValue::Int(value1 / value2))
    }

    fn irem(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;
        if value2 == 0 {
            bail!("TODO: ArithmeticException")
        }

        let result = value1 - (value1 / value2) * value2;
        self.stack.push_operand(FrameValue::Int(result))
    }

    fn monitor_exit(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        let objectref = operand.reference()?;
        let heap_id = objectref.heap_id()?;
        let thread_id = self
            .current_thread_id
            .clone()
            .context("how do we not have a thread id?")?;
        self.exit_object_monitor(heap_id, &thread_id)
    }

    fn monitor_enter(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        let objectref = operand.reference()?;
        let heap_id = objectref.heap_id()?;
        let thread_id = self
            .current_thread_id
            .clone()
            .context("how do we not have a thread id?")?;
        if !self.enter_object_monitor(heap_id, &thread_id)? {
            bail!("TODO: wait for monitor to be available")
        }

        Ok(())
    }

    fn pop(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?;
        if matches!(value, FrameValue::Long(_)) || matches!(value, FrameValue::Double(_)) {
            bail!("pop value has to be of computational type with category 1, is {value:?}");
        }

        Ok(())
    }

    fn ixor(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        self.stack.push_operand(FrameValue::Int(value1 ^ value2))
    }

    fn check_cast(&mut self, index: &CpIndex) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        let object_ref = operand.reference()?;

        if object_ref.is_null() {
            return self.stack.push_operand(operand);
        }

        let current_class = self.current_class()?;

        match current_class.cp_item(index)? {
            CpInfo::Class { name_index } => {
                let identifier = ClassIdentifier::new(current_class.utf8(name_index)?)?;
                let class = self.resolve_class(&identifier)?;
                let heap_item = self.heap_get(object_ref.heap_id()?)?.clone();
                let object_identifier = heap_item.class_identifier()?;

                if class.is_interface() {
                    let object_ref_class = self.resolve_class(&object_identifier)?;
                    if object_ref_class.implements(&identifier)? {
                        return self.stack.push_operand(operand);
                    }
                }

                if object_identifier == identifier {
                    return self.stack.push_operand(operand);
                }

                bail!("TODO: return false?");
            }
            item => bail!("invalid instanceof type {item:?}"),
        }
    }

    fn instance_of(&mut self, index: &CpIndex) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        let object_ref = operand.reference()?;

        if object_ref.is_null() {
            return self.stack.push_operand(FrameValue::Int(0));
        }

        let current_class = self.current_class()?;

        let heap_item = self.heap_get(object_ref.heap_id()?)?.clone();
        if heap_item.is_array() {
            bail!("TOOD: instanceof for arrays")
        }

        match current_class.cp_item(index)? {
            CpInfo::Class { name_index } => {
                let identifier = ClassIdentifier::new(current_class.utf8(name_index)?)?;
                let class = self.resolve_class(&identifier)?;
                let object_identifier = heap_item.class_identifier()?;

                if class.is_interface() {
                    bail!("TODO: instanceof check with interface")
                }

                if object_identifier == identifier {
                    return self.stack.push_operand(FrameValue::Int(1));
                }

                if self.check_super_classes(&class, &identifier)? {
                    return self.stack.push_operand(FrameValue::Int(1));
                }

                self.stack.push_operand(FrameValue::Int(0))
            }
            item => bail!("invalid instanceof type {item:?}"),
        }
    }

    fn check_super_classes(&mut self, class: &Class, identifier: &ClassIdentifier) -> Result<bool> {
        if class.has_super_class() {
            let super_class = class.super_class()?;
            if &super_class == identifier {
                return Ok(true);
            }

            let class = self.resolve_class(&super_class)?;
            return self.check_super_classes(&class, identifier);
        }

        Ok(false)
    }

    fn baload(&mut self) -> Result<()> {
        let index = self.stack.pop_operand()?.int()?;
        let arrayref_operand = self.stack.pop_operand()?;
        let arrayref = arrayref_operand.reference()?;

        if arrayref.is_null() {
            bail!("TODO: throw NullPointerException")
        }

        let (_, values) = self.get_primitive_array(arrayref.heap_id()?)?;
        let array_value = values
            .get(index as usize)
            .context("no array value at index {index}")?;

        let value = match array_value {
            PrimitiveArrayValue::Boolean(val) => FrameValue::Int((*val).into()),
            PrimitiveArrayValue::Byte(val) => FrameValue::Int((*val).into()),
            _ => bail!("baload array value must be boolean or byte"),
        };

        self.stack.push_operand(value)
    }

    fn aaload(&mut self) -> Result<()> {
        let index = self.stack.pop_operand()?.int()?;
        let arrayref_operand = self.stack.pop_operand()?;
        let arrayref = arrayref_operand.reference()?;

        if arrayref.is_null() {
            bail!("TODO: throw NullPointerException")
        }

        let values = self.get_reference_array(arrayref.heap_id()?)?;
        let reference = values
            .get(index as usize)
            .context(format!("no array value at index {index}"))?;

        self.stack
            .push_operand(FrameValue::Reference(reference.clone()))
    }

    fn array_length(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        let heap_id = operand.reference()?.heap_id()?;
        let len = self.get_array_length(heap_id)?;
        debug!("array_length {len}");
        self.stack.push_operand(FrameValue::Int(len as i32))
    }

    fn iushr(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        let result = ((value1 as u32) >> (value2 & 31)) as i32;
        self.stack.push_operand(FrameValue::Int(result))
    }

    fn lushr(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.long()?;

        let result = ((value1 as u64) >> (value2 & 63)) as i64;
        self.stack.push_operand(FrameValue::Long(result))
    }

    fn ishr(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        let result = value1 >> (value2 & 31);
        self.stack.push_operand(FrameValue::Int(result))
    }

    fn lshr(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.long()?;

        let result = value1 >> (value2 & 63);
        self.stack.push_operand(FrameValue::Long(result))
    }

    fn lshl(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.long()?;

        let result = value1 << (value2 & 63);
        self.stack.push_operand(FrameValue::Long(result))
    }

    fn ishl(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        let result = value1 << (value2 & 31);
        self.stack.push_operand(FrameValue::Int(result))
    }

    fn iinc(&mut self, index: usize, constant: i8) -> Result<()> {
        let local_variable = self.stack.local_variable(index)?.int()?;
        self.stack
            .set_local_variable(index, FrameValue::Int(local_variable + constant as i32))
    }

    fn imul(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;
        self.stack
            .push_operand(FrameValue::Int(value1.wrapping_mul(value2)))
    }

    fn fmul(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.float()?;
        let value1 = self.stack.pop_operand()?.float()?;
        self.stack.push_operand(FrameValue::Float(value1 * value2))
    }

    fn lmul(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.long()?;
        let value1 = self.stack.pop_operand()?.long()?;
        self.stack
            .push_operand(FrameValue::Long(value1.wrapping_mul(value2)))
    }

    fn lcmp(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.long()?;
        let value1 = self.stack.pop_operand()?.long()?;

        let value = if value1 > value2 {
            FrameValue::Int(1)
        } else if value1 == value2 {
            FrameValue::Int(0)
        } else {
            FrameValue::Int(-1)
        };

        self.stack.push_operand(value)
    }

    fn lload(&mut self, index: u8) -> Result<()> {
        let value = self.stack.local_variable(index.into())?;
        if value.long().is_err() {
            bail!("lload can only load longs, is {value:?}")
        }

        self.stack.push_operand(value)
    }

    fn lstore(&mut self, index: u8) -> Result<()> {
        let value = self.stack.pop_operand()?;
        if value.long().is_err() {
            bail!("lstore can only store longs, is {value:?}")
        }

        self.stack.set_local_variable(index.into(), value)
    }

    fn fstore(&mut self, index: u8) -> Result<()> {
        let value = self.stack.pop_operand()?;
        if value.float().is_err() {
            bail!("fstore can only store floats, is {value:?}")
        }

        self.stack.set_local_variable(index.into(), value)
    }

    fn i2c(&mut self) -> Result<()> {
        let int = self.stack.pop_operand()?.int()?;
        let char = int as u16;
        self.stack.push_operand(FrameValue::Int(char as i32))
    }

    fn i2b(&mut self) -> Result<()> {
        let int = self.stack.pop_operand()?.int()?;
        let byte = int as i8;
        self.stack.push_operand(FrameValue::Int(byte as i32))
    }

    fn l2i(&mut self) -> Result<()> {
        let long = self.stack.pop_operand()?.long()?;
        self.stack.push_operand(FrameValue::Int(long as i32))
    }

    fn d2l(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?.double()?;
        self.stack.push_operand(FrameValue::Long(value as i64))
    }

    fn iadd(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;
        self.stack.push_operand(FrameValue::Int(value1 + value2))
    }

    fn ladd(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.long()?;
        let value1 = self.stack.pop_operand()?.long()?;
        self.stack.push_operand(FrameValue::Long(value1 + value2))
    }

    fn dadd(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.double()?;
        let value1 = self.stack.pop_operand()?.double()?;
        self.stack.push_operand(FrameValue::Double(value1 + value2))
    }

    fn fadd(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.float()?;
        let value1 = self.stack.pop_operand()?.float()?;
        self.stack.push_operand(FrameValue::Float(value1 + value2))
    }

    fn f2d(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?.float()?;
        self.stack.push_operand(FrameValue::Double(value.into()))
    }

    fn f2i(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?.float()?;
        self.stack.push_operand(FrameValue::Int(value as i32))
    }

    fn fdiv(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.float()?;
        let value1 = self.stack.pop_operand()?.float()?;
        self.stack.push_operand(FrameValue::Float(value1 / value2))
    }

    fn i2l(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?.int()?;
        self.stack.push_operand(FrameValue::Long(value.into()))
    }

    fn i2f(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?.int()?;
        self.stack.push_operand(FrameValue::Float(value as f32))
    }

    fn l2f(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?.long()?;
        self.stack.push_operand(FrameValue::Float(value as f32))
    }

    fn fcmpl(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.float()?;
        let value1 = self.stack.pop_operand()?.float()?;

        let value = if value1 > value2 {
            FrameValue::Int(1)
        } else if value1 == value2 {
            FrameValue::Int(0)
        } else {
            FrameValue::Int(-1)
        };

        self.stack.push_operand(value)
    }

    fn fcmpg(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.float()?;
        let value1 = self.stack.pop_operand()?.float()?;

        let value = if value1 > value2 {
            FrameValue::Int(1)
        } else if value1 == value2 {
            FrameValue::Int(0)
        } else if value1 < value2 {
            FrameValue::Int(-1)
        } else {
            FrameValue::Int(1)
        };

        self.stack.push_operand(value)
    }

    fn iand(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        self.stack.push_operand(FrameValue::Int(value1 & value2))
    }

    fn land(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.long()?;
        let value1 = self.stack.pop_operand()?.long()?;

        self.stack.push_operand(FrameValue::Long(value1 & value2))
    }

    fn isub(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        self.stack.push_operand(FrameValue::Int(value1 - value2))
    }

    fn lsub(&mut self) -> Result<()> {
        let value2 = self.stack.pop_operand()?.long()?;
        let value1 = self.stack.pop_operand()?.long()?;

        self.stack.push_operand(FrameValue::Long(value1 - value2))
    }

    fn bastore(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?;
        let index = self.stack.pop_operand()?;
        let array_ref = self.stack.pop_operand()?;

        if !array_ref.is_reference() && self.is_array(&array_ref)? {
            bail!("arrayref has to be a reference to an array, is {array_ref:?}")
        }

        let index = index.int()? as usize;
        let heap_id = array_ref.reference()?.heap_id()?;
        let (array_type, _) = self.get_primitive_array(heap_id)?;

        let value = match array_type {
            PrimitiveArrayType::Boolean => PrimitiveArrayValue::Boolean((value.int()? & 1) != 0),
            PrimitiveArrayType::Byte => PrimitiveArrayValue::Byte(value.int()? as u8),
            _ => bail!("array type has to be bool or byte, is {array_type:?}"),
        };

        self.store_into_primitive_array(heap_id, index, value)
    }

    fn iastore(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?;
        let index = self.stack.pop_operand()?;
        let array_ref = self.stack.pop_operand()?;

        if !array_ref.is_reference() && self.is_array(&array_ref)? {
            bail!("arrayref has to be a reference to an array, is {array_ref:?}")
        }

        let index = index.int()? as usize;
        let heap_id = array_ref.reference()?.heap_id()?;

        self.store_into_primitive_array(heap_id, index, PrimitiveArrayValue::Int(value.int()?))
    }

    fn castore(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?;
        let index = self.stack.pop_operand()?;
        let array_ref = self.stack.pop_operand()?;

        if !array_ref.is_reference() && self.is_array(&array_ref)? {
            bail!("arrayref has to be a reference to an array, is {array_ref:?}")
        }

        let index = index.int()? as usize;
        let value = value.int()?;
        let heap_id = array_ref.reference()?.heap_id()?;

        self.store_into_primitive_array(heap_id, index, PrimitiveArrayValue::Short(value as u16))
    }

    fn new_array(&mut self, atype: u8) -> Result<()> {
        let array_type = PrimitiveArrayType::new(atype)?;
        let count = self.stack.pop_operand()?.int()?;

        if count < 0 {
            bail!("TODO: throw NegativeArraySizeException");
        }

        let heap_id = self.allocate_default_primitive_array(array_type, count.try_into()?)?;
        self.stack
            .push_operand(FrameValue::Reference(ReferenceValue::HeapItem(heap_id)))
    }

    fn is_array(&self, value: &FrameValue) -> Result<bool> {
        if let FrameValue::Reference(ReferenceValue::HeapItem(heap_id)) = value {
            Ok(self.heap_get(heap_id)?.is_array())
        } else {
            Ok(false)
        }
    }

    fn aastore(&mut self) -> Result<()> {
        let value = self.stack.pop_operand()?;
        let index = self.stack.pop_operand()?;
        let array_ref = self.stack.pop_operand()?;

        if !array_ref.is_reference() && self.is_array(&array_ref)? {
            bail!("arrayref has to be a reference to an array, is {array_ref:?}")
        }

        let index = index.int()? as usize;
        let value = value.reference()?.clone();
        let heap_id = array_ref.reference()?.heap_id()?;

        self.store_into_reference_array(heap_id, index, value)
    }

    fn fload(&mut self, index: u8) -> Result<()> {
        let value = self.stack.local_variable(index.into())?;
        if value.float().is_err() {
            bail!("value has to be float, is {value:?}");
        }
        self.stack.push_operand(value)
    }

    fn iload(&mut self, index: u8) -> Result<()> {
        let value = self.stack.local_variable(index.into())?;

        if value.int().is_err() {
            bail!("value has to be int, is {value:?}");
        }

        self.stack.push_operand(value)
    }

    fn put_field(&mut self, index: &CpIndex) -> Result<()> {
        let (class_identifier, name, descriptor) = self.field_ref(index)?;

        self.resolve_field(&class_identifier, &name, descriptor.raw())?;

        let value = self.stack.pop_operand()?;
        let object_ref = self.stack.pop_operand()?;

        if self.is_array(&object_ref)? || !object_ref.is_reference() {
            bail!("object ref has to be reference but not array, is {object_ref:?}")
        }

        let heap_id = object_ref.reference()?.heap_id()?;
        debug!("put field {name}: {value:?}");
        self.heap_set_field(heap_id, &name, value.into())
    }

    fn get_static(&mut self, index: &CpIndex) -> Result<()> {
        let (class_identifier, name, descriptor) = self.field_ref(index)?;
        self.resolve_field(&class_identifier, &name, descriptor.raw())?;

        let class = self.class(&class_identifier)?;
        let field_value = class.get_static_field_value(&name)?;

        self.stack.push_operand(field_value.into())
    }

    fn if_ge(&mut self, offset: i16) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        if operand.int()? >= 0 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_lt(&mut self, offset: i16) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        if operand.int()? < 0 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_icmpgt(&mut self, offset: i16) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        if value1 > value2 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_icmple(&mut self, offset: i16) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        if value1 <= value2 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_icmpne(&mut self, offset: i16) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        if value1 != value2 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_acmpne(&mut self, offset: i16) -> Result<()> {
        let operand2 = self.stack.pop_operand()?;
        let operand1 = self.stack.pop_operand()?;

        if operand1.reference()? != operand2.reference()? {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_acmpeq(&mut self, offset: i16) -> Result<()> {
        let operand2 = self.stack.pop_operand()?;
        let operand1 = self.stack.pop_operand()?;

        if operand1.reference()? == operand2.reference()? {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_icmpeq(&mut self, offset: i16) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        if value1 == value2 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_icmplt(&mut self, offset: i16) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        if value1 < value2 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_icmpge(&mut self, offset: i16) -> Result<()> {
        let value2 = self.stack.pop_operand()?.int()?;
        let value1 = self.stack.pop_operand()?.int()?;

        if value1 >= value2 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_le(&mut self, offset: i16) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        if operand.int()? <= 0 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_gt(&mut self, offset: i16) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        if operand.int()? > 0 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_eq(&mut self, offset: i16) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        if operand.int()? == 0 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn if_ne(&mut self, offset: i16) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        if operand.int()? != 0 {
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn lreturn(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;

        if operand.long().is_ok() {
            self.stack.pop()?;
            info!("returning {operand:?}");
            self.stack.push_operand(operand)
        } else {
            bail!("ireturn can only return int, is {operand:?}")
        }
    }

    fn ireturn(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;

        if operand.int().is_ok() {
            self.stack.pop()?;
            info!("returning {operand:?}");
            self.stack.push_operand(operand)
        } else {
            bail!("ireturn can only return int, is {operand:?}")
        }
    }

    fn ldc2_w(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;

        let value = match current_class.cp_item(index)? {
            CpInfo::Long(value) => FrameValue::Long(*value),
            CpInfo::Double(value) => FrameValue::Double(*value),
            info => bail!("item {info:?} at index {index:?} is not loadable"),
        };

        self.stack.push_operand(value)
    }

    fn ldc(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;

        let value = match current_class.cp_item(index)? {
            CpInfo::Class { name_index } => {
                let name = current_class.utf8(name_index)?;
                let identifier = ClassIdentifier::new(name)?;
                self.resolve_class(&identifier)?;

                FrameValue::Reference(ReferenceValue::Class(identifier))
            }
            CpInfo::String { string_index } => {
                let value = current_class.utf8(string_index)?;
                let object_id = self.new_string(value.to_string())?;
                FrameValue::Reference(ReferenceValue::HeapItem(object_id))
            }
            CpInfo::Integer(value) => FrameValue::Int(*value),
            CpInfo::Float(value) => FrameValue::Float(*value),
            info => bail!("item {info:?} at index {index:?} is not loadable"),
        };

        self.stack.push_operand(value)
    }

    fn invoke_virtual(&mut self, index: &CpIndex) -> Result<()> {
        let (class_identifier, name, descriptor) = self.method_ref(index)?;
        let (class_identifier, method) =
            self.resolve_method(&class_identifier, &name, &descriptor)?;
        let class = self.class(&class_identifier)?;

        let method_descriptor = MethodDescriptor::new(class.utf8(&method.descriptor_index)?)?;
        let operands = self
            .stack
            .pop_operands(method_descriptor.parameters.len() + 1)?;

        let objectref = operands.first().context("no objectref found")?;
        if objectref.reference()?.is_null() {
            bail!("TODO: throw NullPointerException");
        }

        let (class, method) = if method.is_private()
            || class_identifier == ClassIdentifier::new("java.lang.Class")?
            || objectref.reference()?.is_class()
        {
            (class, method)
        } else {
            let objectref_identifier = self.class_identifier(objectref.reference()?)?;
            let class = self.class(&objectref_identifier)?;
            let (class, method) = self.select_method(&class, &method, &name, &method_descriptor)?;
            (class, method)
        };

        let method_name = class.method_name(&method)?.to_string();

        let heap_id = objectref.reference()?.heap_id().ok();

        if method.is_synchronized() {
            let thread_id = self
                .current_thread_id
                .clone()
                .context("how do we not have a thread id?")?;
            if let Some(heap_id) = heap_id {
                if !self.enter_object_monitor(heap_id, &thread_id)? {
                    bail!("TODO: wait for monitor to be available")
                }
            } else {
                let identifier = objectref.reference()?.class_identifier()?;
                if !self.enter_class_monitor(identifier, &thread_id)? {
                    bail!("TODO: wait for monitor to be available")
                }
            }
        }

        if !method.is_native() {
            let code = method
                .code()
                .context(format!("no code found for {name} method"))?;
            self.stack.push(
                method_name,
                method_descriptor,
                operands.clone(),
                Code::new(code.clone())?,
                class.identifier().clone(),
                heap_id.cloned(),
            );
            self.execute()
        } else if let Some(return_value) = self.run_native_method(&class, &method_name, operands)? {
            self.stack.push_operand(return_value)
        } else {
            Ok(())
        }
    }

    fn select_method(
        &self,
        class: &Class,
        method: &Method,
        name: &str,
        method_descriptor: &MethodDescriptor,
    ) -> Result<(Class, Method)> {
        if let Some(m) = class.overriden_method(method, name, method_descriptor)? {
            return Ok((class.clone(), m));
        } else if class.has_super_class() {
            let super_class = self.class(&class.super_class()?)?;
            return self.select_method(&super_class, method, name, method_descriptor);
        }

        bail!("no method found")
    }

    fn invoke_interface(&mut self, index: &CpIndex, _: u8) -> Result<()> {
        let (class_identifier, name, descriptor) = self.method_ref(index)?;
        let method = self.resolve_interface_method(&class_identifier, &name, &descriptor)?;

        if method.is_synchronized() {
            bail!("TODO: synchronized interface method")
        }

        if method.is_native() {
            bail!("TODO: native interface method")
        }

        let interface_class = self.class(&class_identifier)?;
        let method_descriptor = interface_class.method_descriptor(&method)?;
        let operands = self
            .stack
            .pop_operands(method_descriptor.parameters.len() + 1)?;
        let objectref = operands.first().context("no first operand")?.reference()?;
        let heap_id = objectref.heap_id()?;
        let class_identifier = self.class_identifier_from_reference(objectref)?;
        let class = self.class(&class_identifier)?;
        let (class, method) = self.select_method(&class, &method, &name, &method_descriptor)?;
        let code = method
            .code()
            .context(format!("no code found for {name} method"))?;
        self.stack.push(
            name,
            method_descriptor,
            operands.clone(),
            Code::new(code.clone())?,
            class.identifier().clone(),
            Some(heap_id.clone()),
        );
        self.execute()
    }

    fn invoke_static(&mut self, index: &CpIndex) -> Result<()> {
        let (class_identifier, name, descriptor) = self.method_ref(index)?;

        let (_, method) = self.resolve_method(&class_identifier, &name, &descriptor)?;
        let class = self.class(&class_identifier)?;

        if !method.is_static() {
            bail!("method has to be static");
        }

        if method.is_abstract() {
            bail!("method cannot be static");
        }

        if method.is_synchronized() {
            let thread_id = self
                .current_thread_id
                .clone()
                .context("how do we not have a thread id?")?;
            if !self.enter_class_monitor(&class_identifier, &thread_id)? {
                bail!("TODO: wait for monitor to be available")
            }
        }

        let descriptor = class.method_descriptor(&method)?;

        let operands = self.stack.pop_operands(descriptor.parameters.len())?;
        if method.is_native() {
            if let Some(return_value) = self.run_native_method(&class, &name, operands)? {
                self.stack.push_operand(return_value)
            } else {
                Ok(())
            }
        } else {
            let code = method
                .code()
                .context(format!("no code found for {name} method"))?;
            self.stack.push(
                name,
                descriptor,
                operands,
                Code::new(code.clone())?,
                class_identifier,
                None,
            );
            self.execute()
        }
    }

    fn a_new_array(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        let array_class = current_class.class_identifier(index)?;
        self.initialize(&array_class)?;
        let length = self.stack.pop_int()?;
        let array = self.allocate_array(array_class, length as usize)?;
        let value = FrameValue::Reference(ReferenceValue::HeapItem(array));
        self.stack.push_operand(value)
    }

    fn put_static(&mut self, index: &CpIndex) -> Result<()> {
        let (identifier, name, descriptor) = self.field_ref(index)?;

        self.resolve_field(&identifier, &name, descriptor.raw())?;
        let value = self.stack.pop_operand()?;
        debug!("put static field {name}: {value:?}");
        let mut classes = self
            .classes
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        classes
            .get_mut(&identifier)
            .context(format!("class {identifier:?} is not initialized"))?
            .set_static_field(&name, value.into())
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
        let (class_identifier, name, descriptor) = self.field_ref(index)?;

        self.resolve_field(&class_identifier, &name, descriptor.raw())?;
        let object_ref = self.stack.pop_operand()?;
        if !object_ref.is_reference() || self.is_array(&object_ref)? {
            bail!("objectref has to be a reference but no array, is {object_ref:?}");
        }

        // TODO: is this good? maybe classes should live on the heap as well?
        if class_identifier == ClassIdentifier::new("java.lang.Class")? {
            let identifier = self.class_identifier_from_reference(object_ref.reference()?)?;
            let class = self.class(&identifier)?;
            let field_value = class.get_class_field_value(&name)?;
            self.stack.push_operand(field_value.into())
        } else {
            let heap_id = object_ref.reference()?.heap_id()?;
            let field_value = self.heap_get_field(heap_id, &name)?;
            debug!("get field {name}: {field_value:?}");
            self.stack.push_operand(field_value.into())
        }
    }

    // TODO: is this the same as self.class_identifier() ?
    fn class_identifier_from_reference(
        &self,
        reference: &ReferenceValue,
    ) -> Result<ClassIdentifier> {
        match reference {
            ReferenceValue::Class(class_identifier) => Ok(class_identifier.clone()),
            ReferenceValue::HeapItem(heap_id) => self.heap_get(heap_id)?.class_identifier(),
            _ => bail!("no class identifier found for value {reference:?}"),
        }
    }

    fn istore(&mut self, index: u8) -> Result<()> {
        let int = self.stack.pop_operand()?;
        if int.int().is_err() {
            bail!("istore value has to be int")
        }

        self.stack.set_local_variable(index.into(), int)
    }

    fn astore(&mut self, index: u8) -> Result<()> {
        let objectref = self.stack.pop_operand()?;
        if !objectref.is_reference() {
            bail!("TODO: astore objectref has to be reference")
        }

        self.stack.set_local_variable(index.into(), objectref)
    }

    fn if_null(&mut self, offset: i16) -> Result<()> {
        let value = self.stack.pop_operand()?;
        if !value.is_reference() {
            bail!("ifnull value has to be reference, is {value:?}");
        }

        if value.is_null() {
            self.stack.offset_pc(offset as i32)
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
            self.stack.offset_pc(offset as i32)
        } else {
            self.stack.offset_pc(3)
        }
    }

    fn new_instruction(&mut self, index: &CpIndex) -> Result<()> {
        let current_class = self.current_class()?;
        let class_identifier = current_class.class_identifier(index)?;
        let class = self.resolve_class(&class_identifier)?;
        let fields = self.default_instance_fields(&class, 0)?;
        let object_id = self.allocate(class.identifier().clone(), fields)?;
        self.stack
            .push_operand(FrameValue::Reference(ReferenceValue::HeapItem(object_id)))
    }

    fn dup(&mut self) -> Result<()> {
        let operand = self.stack.pop_operand()?;
        self.stack.push_operand(operand.clone())?;
        self.stack.push_operand(operand)
    }

    fn dup2(&mut self) -> Result<()> {
        let value1 = self.stack.pop_operand()?;
        if value1.is_category1() {
            let value2 = self.stack.pop_operand()?;
            if !value2.is_category1() {
                bail!("bot values have to be category 1");
            }
            self.stack.push_operand(value2.clone())?;
            self.stack.push_operand(value1.clone())?;
            self.stack.push_operand(value2)?;
            self.stack.push_operand(value1)
        } else {
            self.stack.push_operand(value1.clone())?;
            self.stack.push_operand(value1)
        }
    }

    fn dup_x1(&mut self) -> Result<()> {
        let value1 = self.stack.pop_operand()?;
        let value2 = self.stack.pop_operand()?;
        self.stack.push_operand(value1.clone())?;
        self.stack.push_operand(value2)?;
        self.stack.push_operand(value1)
    }

    fn invoke_special(&mut self, index: &CpIndex) -> Result<()> {
        let (class_identifier, name, descriptor) = self.method_ref(index)?;
        let (_, method) = self.resolve_method(&class_identifier, &name, &descriptor)?;
        let class = self.initialize(&class_identifier)?;
        let method_descriptor = class.method_descriptor(&method)?;

        if !Self::is_instance_initialization_method(&name, &method_descriptor) {
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
                .context(format!("no code found for {name} method"))?;
            self.stack.push(
                name.to_string(),
                method_descriptor,
                operands,
                Code::new(code.clone())?,
                class.identifier().clone(),
                None,
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
        let _method_handle =
            self.resolve_method_handle(bootstrap_method_attr_index, name_and_type_index)?;
        bail!("TODO: callsite resolution")
    }

    fn resolve_method_handle(
        &mut self,
        _bootstrap_method_attr_index: &CpIndex,
        name_and_type_index: &CpIndex,
    ) -> Result<HeapId> {
        let current_class = self.current_class()?;

        let (_name, descriptor) = current_class.name_and_type(name_and_type_index)?;
        let method_descriptor = MethodDescriptor::new(descriptor)?;
        if let ReturnDescriptor::FieldType(FieldType::ObjectType { class_name }) =
            method_descriptor.return_descriptor
        {
            self.initialize(&ClassIdentifier::new(&class_name)?)?;
        }

        for parameter in method_descriptor.parameters {
            if let FieldType::ObjectType { class_name } = parameter {
                self.initialize(&ClassIdentifier::new(&class_name)?)?;
            }
        }

        let method_type_identifier = ClassIdentifier::new("java.lang.invoke.MethodType")?;
        let _class = self.resolve_class(&method_type_identifier)?;

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
        info!(
            "running native method '{name}' in {:?} with operands {:?}",
            class.identifier(),
            operands
        );

        match format!("{:?}", class.identifier()).as_str() {
            "java.lang.Class" => match name {
                "registerNatives" => Ok(None),
                "initClassName" => {
                    if let FrameValue::Reference(ReferenceValue::Class(identifier)) =
                        operands.first().context("no operands provided")?
                    {
                        let object_id = self.new_string(format!("{identifier:?}").to_string())?;
                        Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(
                            object_id,
                        ))))
                    } else {
                        bail!("first operand has to be a reference")
                    }
                }
                "desiredAssertionStatus0" => Ok(Some(FrameValue::Int(0))),
                "getPrimitiveClass" => {
                    let operand = operands.first().context("operands are empty")?;
                    let heap_id =
                        if let FrameValue::Reference(ReferenceValue::HeapItem(heap_id)) = operand {
                            heap_id
                        } else {
                            bail!("no reference found, instead: {operand:?}")
                        };

                    let value_heap_item = self.heap_get_field(heap_id, "value")?;
                    let (_, primitive_array) =
                        self.get_primitive_array(value_heap_item.heap_id()?)?;

                    let bytes: Vec<u8> = primitive_array
                        .iter()
                        .map(|p| p.byte())
                        .collect::<Result<Vec<u8>>>()?;
                    let name = String::from_utf8(bytes)?;
                    match name.as_str() {
                        "int" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Integer")?,
                        )))),
                        "boolean" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Boolean")?,
                        )))),
                        "byte" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Byte")?,
                        )))),
                        "short" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Short")?,
                        )))),
                        "char" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Character")?,
                        )))),
                        "double" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Double")?,
                        )))),
                        "long" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Long")?,
                        )))),
                        "float" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                            ClassIdentifier::new("java.lang.Float")?,
                        )))),
                        _ => bail!("invalid primitive class name: '{name}'"),
                    }
                }
                "forName0" => {
                    let heap_id = operands
                        .first()
                        .context("no first operand")?
                        .reference()?
                        .heap_id()?;
                    let byte_value = self.heap_get_field(heap_id, "value")?;
                    let (_, primitive_array) = self.get_primitive_array(byte_value.heap_id()?)?;
                    let bytes: Vec<u8> = primitive_array
                        .iter()
                        .map(|p| p.byte())
                        .collect::<Result<Vec<u8>>>()?;
                    let name = String::from_utf8(bytes)?;
                    Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                        ClassIdentifier::new(&name)?,
                    ))))
                }
                "isPrimitive" => {
                    let value = match format!(
                        "{:?}",
                        operands
                            .first()
                            .context("no first operand")?
                            .reference()?
                            .class_identifier()?
                    )
                    .as_str()
                    {
                        "java.lang.Byte" => FrameValue::Int(1),
                        "java.lang.Character" => FrameValue::Int(1),
                        "java.lang.Double" => FrameValue::Int(1),
                        "java.lang.Float" => FrameValue::Int(1),
                        "java.lang.Integer" => FrameValue::Int(1),
                        "java.lang.Long" => FrameValue::Int(1),
                        "java.lang.Short" => FrameValue::Int(1),
                        "java.lang.Boolean" => FrameValue::Int(1),
                        "java.lang.Void" => FrameValue::Int(1),
                        _ => FrameValue::Int(0),
                    };
                    Ok(Some(value))
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.Runtime" => match name {
                "availableProcessors" => {
                    let cpus = std::thread::available_parallelism()?;
                    Ok(Some(FrameValue::Int(cpus.get().try_into()?)))
                }
                "maxMemory" => Ok(Some(FrameValue::Long(8192 * 1024 * 1024 * 1024))),
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "jdk.internal.misc.Unsafe" => match name {
                "registerNatives" => Ok(None),
                "storeFence" => Ok(None),
                "arrayBaseOffset0" => Ok(Some(FrameValue::Int(0))),
                "arrayIndexScale0" => Ok(Some(FrameValue::Int(0))),
                "objectFieldOffset1" => {
                    let class = operands.get(1).context("no class operand found")?;
                    let name = operands.get(2).context("no String operand found")?;
                    let byte_value = self.heap_get_field(name.reference()?.heap_id()?, "value")?;
                    let (_, primitive_array) = self.get_primitive_array(byte_value.heap_id()?)?;
                    let bytes: Vec<u8> = primitive_array
                        .iter()
                        .map(|p| p.byte())
                        .collect::<Result<Vec<u8>>>()?;
                    let name = String::from_utf8(bytes)?;
                    let class = self.class(class.reference()?.class_identifier()?)?;
                    let offset = self
                        .default_instance_fields(&class, 0)?
                        .get(&name)
                        .context(format!("no field '{name}' found"))?
                        .offset();
                    Ok(Some(FrameValue::Long(offset)))
                }
                "compareAndSetInt" => {
                    let object = operands.get(1).context("no 'object' operand found")?;
                    let offset = operands
                        .get(2)
                        .context("no 'offset' operand found")?
                        .long()?;
                    let expected = operands
                        .get(3)
                        .context("no 'expected' operand found")?
                        .int()?;
                    let x = operands.get(4).context("no 'x' operand found")?.int()?;
                    let heap_id = object.reference()?.heap_id()?;

                    let object = self.heap_get(heap_id)?;

                    let class = self.class(&object.class_identifier()?)?;
                    for (name, field) in self.default_instance_fields(&class, 0)? {
                        if field.offset() == offset
                            && self.heap_get_field(heap_id, &name)?.int()? == expected
                        {
                            self.heap_set_field(heap_id, &name, FieldValue::Integer(x))?;
                            return Ok(Some(FrameValue::Int(1)));
                        }
                    }

                    bail!("no field with offset '{offset}' found");
                }
                "compareAndSetLong" => {
                    let object = operands.get(1).context("no 'object' operand found")?;
                    let offset = operands
                        .get(2)
                        .context("no 'offset' operand found")?
                        .long()?;
                    let expected = operands
                        .get(3)
                        .context("no 'expected' operand found")?
                        .long()?;
                    let x = operands.get(4).context("no 'x' operand found")?.long()?;
                    let heap_id = object.reference()?.heap_id()?;

                    let object = self.heap_get(heap_id)?;

                    let class = self.class(&object.class_identifier()?)?;
                    for (name, field) in self.default_instance_fields(&class, 0)? {
                        if field.offset() == offset
                            && self.heap_get_field(heap_id, &name)?.long()? == expected
                        {
                            self.heap_set_field(heap_id, &name, FieldValue::Long(x))?;
                            return Ok(Some(FrameValue::Int(1)));
                        }
                    }

                    bail!("no field with offset '{offset}' found");
                }
                "compareAndSetReference" => {
                    let object = operands.get(1).context("no 'object' operand found")?;
                    let offset = operands
                        .get(2)
                        .context("no 'offset' operand found")?
                        .long()?;
                    let expected = operands
                        .get(3)
                        .context("no 'expected' operand found")?
                        .reference()?;
                    let x = operands
                        .get(4)
                        .context("no 'x' operand found")?
                        .reference()?;
                    let heap_id = object.reference()?.heap_id()?;

                    let object = self.heap_get(heap_id)?;

                    if object.is_array() {
                        self.store_into_reference_array(heap_id, offset as usize, x.clone())?;
                        return Ok(Some(FrameValue::Int(1)));
                    }

                    let class = self.class(&object.class_identifier()?)?;
                    for (name, field) in self.default_instance_fields(&class, 0)? {
                        if field.offset() == offset
                            && self.heap_get_field(heap_id, &name)?.reference()? == *expected
                        {
                            self.heap_set_field(heap_id, &name, FieldValue::Reference(x.clone()))?;
                            return Ok(Some(FrameValue::Int(1)));
                        }
                    }

                    bail!("no field with offset '{offset}' found");
                }
                "getReferenceVolatile" => {
                    let object = operands.get(1).context("no 'object' operand found")?;
                    let offset = operands
                        .get(2)
                        .context("no 'offset' operand found")?
                        .long()?;
                    let heap_id = object.reference()?.heap_id()?;
                    let object = self.heap_get(heap_id)?;

                    if let HeapItem::ReferenceArray { values, .. } = object {
                        let value = values.get(offset as usize).context("no value at offset")?;
                        return Ok(Some(FrameValue::Reference(value.clone())));
                    }

                    let class = self.class(&object.class_identifier()?)?;

                    for (name, field) in self.default_instance_fields(&class, 0)? {
                        if field.offset() == offset {
                            let field_value = self.heap_get_field(heap_id, &name)?;
                            return Ok(Some(field_value.into()));
                        }
                    }

                    bail!("no field with offset '{offset}' found");
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.Thread" => match name {
                "registerNatives" => Ok(None),
                "currentThread" => Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(
                    self.current_thread_object
                        .clone()
                        .context("no current thread found")?,
                )))),
                "setPriority0" => {
                    let objectref = operands.first().context("no first operand")?;
                    let priority = operands.get(1).context("no second operand")?;
                    let heap_id = objectref.reference()?.heap_id()?;
                    self.heap_set_field(heap_id, "priority", priority.clone().into())?;
                    Ok(None)
                }
                "start0" => {
                    let objectref = operands
                        .first()
                        .context("no first operand, no thread to start")?;
                    let heap_id = objectref.reference()?.heap_id()?;
                    let heap_item = self.heap_get(heap_id)?;
                    let object = heap_item.object()?;
                    let class_identifier = object.class();

                    let name = self.heap_get_field(heap_id, "name")?;
                    let byte_value = self.heap_get_field(name.heap_id()?, "value")?;
                    let (_, primitive_array) = self.get_primitive_array(byte_value.heap_id()?)?;
                    let bytes: Vec<u8> = primitive_array
                        .iter()
                        .map(|p| p.byte())
                        .collect::<Result<Vec<u8>>>()?;
                    let name = String::from_utf8(bytes)?;

                    let new_thread = Self::new(
                        name.to_string(),
                        self.class_loader.clone(),
                        self.classes.clone(),
                        self.heap.clone(),
                        self.monitors.clone(),
                    );

                    Self::run_with_method(
                        new_thread,
                        class_identifier.clone(),
                        "run".to_string(),
                        "()V".to_string(),
                    );

                    Ok(None)
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.System" => match name {
                "registerNatives" => Ok(None),
                "nanoTime" => {
                    let now = Instant::now();
                    let elapsed = now.duration_since(self.creation_time).as_nanos();
                    Ok(Some(FrameValue::Long(elapsed as i64)))
                }
                "identityHashCode" => {
                    let mut hasher = DefaultHasher::new();
                    operands
                        .first()
                        .context("operands are empty")?
                        .reference()?
                        .hash(&mut hasher);
                    let value = hasher.finish();
                    Ok(Some(FrameValue::Int(value as i32)))
                }
                "arraycopy" => {
                    let src = operands
                        .first()
                        .context("no src operand")?
                        .reference()?
                        .heap_id()?;
                    let src_pos = operands.get(1).context("no src_pos operand")?.int()?;
                    let dest = operands
                        .get(2)
                        .context("no dest operand")?
                        .reference()?
                        .heap_id()?;
                    let mut dest_pos = operands.get(3).context("no dest_pos operand")?.int()?;
                    let length = operands.get(4).context("no length operand")?.int()?;

                    if let Ok((_, src_arr)) = self.get_primitive_array(src) {
                        for i in src_pos..src_pos + length {
                            let val = src_arr
                                .get(i as usize)
                                .context("TODO: throw IndexOutOfBoundsException")?;
                            self.store_into_primitive_array(dest, dest_pos as usize, val.clone())?;
                            dest_pos += 1;
                        }

                        return Ok(None);
                    }

                    bail!("arraycopy for reference array");
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "jdk.internal.misc.CDS" => match name {
                "isDumpingClassList0" => Ok(Some(FrameValue::Int(0))),
                "isDumpingArchive0" => Ok(Some(FrameValue::Int(0))),
                "isSharingEnabled0" => Ok(Some(FrameValue::Int(0))),
                // TODO: provide a proper seed
                "getRandomSeedForDumping" => Ok(Some(FrameValue::Long(0))),
                "initializeFromArchive" => Ok(None),
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "jdk.internal.misc.VM" => match name {
                "initialize" => Ok(None),
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "jdk.internal.reflect.Reflection" => match name {
                "getCallerClass" => {
                    let caller_class = self.stack.caller_class()?;
                    Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                        caller_class.clone(),
                    ))))
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.Object" => match name {
                "getClass" => {
                    let heap_id = operands
                        .first()
                        .context("operands are empty")?
                        .reference()?
                        .heap_id()?;
                    let heap_item = self.heap_get(heap_id)?;
                    let class_identifier = heap_item.class_identifier()?;
                    Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                        class_identifier.clone(),
                    ))))
                }
                "hashCode" => {
                    // TODO: this is ultra wrong, need to hash values not references
                    let mut hasher = DefaultHasher::new();
                    operands
                        .first()
                        .context("operands are empty")?
                        .reference()?
                        .hash(&mut hasher);
                    let value = hasher.finish();
                    Ok(Some(FrameValue::Int(value as i32)))
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.security.AccessController" => match name {
                // TODO: this will be used at some point
                "getStackAccessControlContext" => {
                    Ok(Some(FrameValue::Reference(ReferenceValue::Null)))
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.ref.Reference" => match name {
                // TODO: this will be used at some point
                "waitForReferencePendingList" => {
                    warn!("parking this thread, reference pending list not implemented yet");
                    std::thread::park();
                    Ok(None)
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.ClassLoader" => match name {
                // TODO: this will be used at some point
                "registerNatives" => Ok(None),
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.Float" => match name {
                "floatToRawIntBits" => {
                    let float = operands
                        .first()
                        .context("no float to convert to int")?
                        .float()?;
                    Ok(Some(FrameValue::Int(float as i32)))
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "java.lang.Double" => match name {
                "doubleToRawLongBits" => {
                    let double = operands
                        .first()
                        .context("no double to convert to long")?
                        .double()?;
                    Ok(Some(FrameValue::Long(double as i64)))
                }
                "longBitsToDouble" => {
                    let long = operands
                        .first()
                        .context("no long to convert to double")?
                        .long()?;
                    Ok(Some(FrameValue::Double(long as f64)))
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            "jdk.internal.util.SystemProps$Raw" => match name {
                "platformProperties" => {
                    let string_class = ClassIdentifier::new("java.lang.String")?;
                    self.initialize(&string_class)?;
                    let array = self.allocate_array(string_class, 39)?;
                    Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(array))))
                }
                "vmProperties" => {
                    let string_class = ClassIdentifier::new("java.lang.String")?;
                    self.initialize(&string_class)?;
                    let array = self.allocate_array(string_class, 0)?;
                    Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(array))))
                }
                _ => bail!(
                    "native method {name} on {} not implemented",
                    class.identifier()
                ),
            },
            _ => bail!(
                "native method {name} on {} not implemented",
                class.identifier()
            ),
        }
    }

    fn new_string(&mut self, value: String) -> Result<HeapId> {
        let string_identifier = ClassIdentifier::new("java.lang.String")?;
        let class = self.resolve_class(&string_identifier)?;

        let fields = self.default_instance_fields(&class, 0)?;
        let object_id = self.allocate(class.identifier().clone(), fields)?;
        let bytes = value
            .into_bytes()
            .iter()
            .map(|b| PrimitiveArrayValue::Byte(*b))
            .collect();
        let heap_item = self.allocate_primitive_array(PrimitiveArrayType::Byte, bytes)?;
        let byte_array = FrameValue::Reference(ReferenceValue::HeapItem(heap_item));
        self.heap_set_field(&object_id, "value", byte_array.into())?;
        Ok(object_id)
    }

    fn new_thread_object(&mut self, name: String, thread_group_name: String) -> Result<HeapId> {
        let name_string = self.new_string(name)?;
        let thread_group = self.new_thread_group_object(thread_group_name)?;
        let thread_identifier = ClassIdentifier::new("java.lang.Thread")?;
        let class = self.resolve_class(&thread_identifier)?;

        let fields = self.default_instance_fields(&class, 0)?;
        let object_id = self.allocate(class.identifier().clone(), fields)?;
        self.heap_set_field(
            &object_id,
            "name",
            FieldValue::Reference(ReferenceValue::HeapItem(name_string)),
        )?;
        self.heap_set_field(
            &object_id,
            "group",
            FieldValue::Reference(ReferenceValue::HeapItem(thread_group)),
        )?;
        self.heap_set_field(&object_id, "priority", FieldValue::Integer(1))?;

        let thread_id = class.get_static_field_value("threadSeqNumber")?.long()?;
        self.heap_set_field(&object_id, "tid", FieldValue::Long(thread_id))?;

        let mut classes = self
            .classes
            .lock()
            .map_err(|e| anyhow!("Lock poisoned: {}", e))?;
        classes
            .get_mut(&thread_identifier)
            .context(format!("class {thread_identifier:?} is not initialized"))?
            .set_static_field("threadSeqNumber", FieldValue::Long(thread_id + 1))?;

        self.current_thread_id = Some(thread_id.into());
        Ok(object_id)
    }

    fn new_thread_group_object(&mut self, name: String) -> Result<HeapId> {
        let name_string = self.new_string(name)?;
        let thread_identifier = ClassIdentifier::new("java.lang.ThreadGroup")?;
        let class = self.resolve_class(&thread_identifier)?;

        let fields = self.default_instance_fields(&class, 0)?;
        let object_id = self.allocate(class.identifier().clone(), fields)?;
        self.heap_set_field(
            &object_id,
            "name",
            FieldValue::Reference(ReferenceValue::HeapItem(name_string)),
        )?;
        self.heap_set_field(&object_id, "maxPriority", FieldValue::Integer(10))?;
        Ok(object_id)
    }

    fn default_instance_fields(
        &mut self,
        class: &Class,
        mut offset: i64,
    ) -> Result<HashMap<String, InstanceField>> {
        let mut fields = HashMap::new();
        for field in class.fields() {
            if field.is_static() {
                continue;
            }

            let field_name = class.utf8(&field.name_index)?;
            let descriptor = class.utf8(&field.descriptor_index)?;
            fields.insert(
                field_name.to_string(),
                InstanceField::new(offset, FieldDescriptor::new(descriptor)?.into()),
            );
            offset += 1;
        }

        if class.has_super_class() {
            let super_class = self.initialize(&class.super_class()?)?;
            let super_class_fields = self.default_instance_fields(&super_class, offset)?;
            fields.extend(super_class_fields);
        }

        Ok(fields)
    }

    fn resolve_method(
        &mut self,
        class: &ClassIdentifier,
        name: &str,
        descriptor: &str,
    ) -> Result<(ClassIdentifier, Method)> {
        let class = self.initialize(class)?;

        if let Ok(m) = class.method(name, descriptor) {
            if class.is_method_signature_polymorphic(m)? {
                bail!("TODO: method is signature polymorphic");
            }

            Ok((class.identifier().clone(), m.clone()))
        } else {
            let super_class = class
                .super_class()
                .context("method not found, maybe check interfaces?")?;
            self.resolve_method(&super_class.clone(), name, descriptor)
        }
    }

    fn resolve_interface_method(
        &mut self,
        class: &ClassIdentifier,
        name: &str,
        descriptor: &str,
    ) -> Result<Method> {
        let class = self.initialize(class)?;

        if !class.is_interface() {
            bail!(
                "class {:?} is not a interface, TODO: throw IncompatibleClassChangeError",
                class.identifier()
            );
        }

        if let Ok(m) = class.method(name, descriptor) {
            return Ok(m.clone());
        }

        let object_class = self.class(&ClassIdentifier::new("java.lang.Object")?)?;
        if let Ok(object_method) = object_class.method(name, descriptor)
            && object_method.is_public()
            && !object_method.is_static()
        {
            return Ok(object_method.clone());
        }

        for super_interface in class.super_interfaces()? {
            if let Ok(method) = self.resolve_interface_method(&super_interface, name, descriptor) {
                return Ok(method.clone());
            }
        }

        bail!("TODO: 5.4.3.4 interface method resolution")
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

    fn field_ref(&self, index: &CpIndex) -> Result<(ClassIdentifier, String, FieldDescriptor)> {
        let current_class = self.current_class()?;
        if let CpInfo::FieldRef {
            class_index,
            name_and_type_index,
        } = current_class.cp_item(index)?
        {
            let class_identifier = current_class.class_identifier(class_index)?;
            let (name, descriptor) = current_class.name_and_type(name_and_type_index)?;
            let field_descriptor = FieldDescriptor::new(descriptor)?;
            Ok((class_identifier, name.to_string(), field_descriptor))
        } else {
            bail!("no field reference at index {index:?}")
        }
    }

    fn method_ref(&mut self, index: &CpIndex) -> Result<(ClassIdentifier, String, String)> {
        let current_class = self.current_class()?;

        match current_class.cp_item(index)? {
            CpInfo::MethodRef {
                class_index,
                name_and_type_index,
            }
            | CpInfo::InterfaceMethodRef {
                class_index,
                name_and_type_index,
            } => {
                let class_identifier = current_class.class_identifier(class_index)?;
                let (name, descriptor) = current_class.name_and_type(name_and_type_index)?;
                Ok((class_identifier, name.to_string(), descriptor.to_string()))
            }
            _ => bail!("no method reference at index {index:?}"),
        }
    }

    fn class_identifier(&self, reference: &ReferenceValue) -> Result<ClassIdentifier> {
        match reference {
            ReferenceValue::HeapItem(heap_id) => self.heap_get(heap_id)?.class_identifier(),
            ReferenceValue::Class(class_identifier) => Ok(class_identifier.clone()),
            ReferenceValue::Null => bail!("reference is null"),
        }
    }

    fn handle_synchronized_return(&mut self) -> Result<()> {
        let current_class = self.current_class()?;
        let method = current_class.method(
            self.stack.method_name()?,
            self.stack.method_descriptor()?.raw(),
        )?;

        if method.is_synchronized() {
            let thread_id = self
                .current_thread_id
                .clone()
                .context("how do we not have a thread id?")?;
            if method.is_static() {
                self.exit_class_monitor(current_class.identifier(), &thread_id)
            } else {
                let heap_id = self
                    .stack
                    .object_ref()?
                    .context("no object_ref in current frame")?;
                self.exit_object_monitor(&heap_id, &thread_id)
            }
        } else {
            Ok(())
        }
    }
}
