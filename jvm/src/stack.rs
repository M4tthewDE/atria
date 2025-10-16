use crate::{ClassIdentifier, ReferenceValue, instruction::Instruction};
use anyhow::{Context, Result, bail};
use parser::class::descriptor::MethodDescriptor;
use tracing::trace;

#[derive(Debug, Default)]
pub struct Stack {
    frames: Vec<Frame>,
}

impl Stack {
    pub fn push(
        &mut self,
        method_name: String,
        method_descriptor: MethodDescriptor,
        local_variables: Vec<FrameValue>,
        code: Vec<u8>,
        class: ClassIdentifier,
    ) {
        self.frames.push(Frame::new(
            method_name,
            method_descriptor,
            local_variables,
            code,
            class,
        ));
    }

    pub fn pop(&mut self) -> Result<()> {
        if self.frames.pop().is_some() {
            Ok(())
        } else {
            bail!("nothing to pop, stack is empty")
        }
    }

    pub fn push_operand(&mut self, operand: FrameValue) -> Result<()> {
        let frame = self.frames.last_mut().context("no frame found")?;
        frame.push_operand(operand);
        Ok(())
    }

    pub fn operands(&self) -> Result<&Vec<FrameValue>> {
        Ok(&self.frames.last().context("no frame found")?.operand_stack)
    }

    pub fn pop_operands(&mut self, n: usize) -> Result<Vec<FrameValue>> {
        let frame = self.frames.last_mut().context("no frame found")?;
        frame.pop_operands(n)
    }

    pub fn pop_operand(&mut self) -> Result<FrameValue> {
        let frame = self.frames.last_mut().context("no frame found")?;
        frame.pop_operand()
    }

    pub fn pop_int(&mut self) -> Result<i32> {
        let frame = self.frames.last_mut().context("no frame found")?;
        frame.pop_int()
    }

    pub fn current_instruction(&mut self) -> Result<Instruction> {
        let frame = self.frames.last_mut().context("no frame found")?;
        frame.current_instruction()
    }

    pub fn current_class(&self) -> Result<ClassIdentifier> {
        let frame = self.frames.last().context("no frame found")?;
        Ok(frame.class.clone())
    }

    pub fn local_variable(&self, index: usize) -> Result<FrameValue> {
        let frame = self.frames.last().context("no frame found")?;
        frame.local_variable(index)
    }

    pub fn set_local_variable(&mut self, index: usize, value: FrameValue) -> Result<()> {
        let frame = self.frames.last_mut().context("no frame found")?;

        if matches!(value, FrameValue::Long(_)) || matches!(value, FrameValue::Double(_)) {
            frame.set_local_variable(index, value)?;
            frame.set_local_variable(index + 1, FrameValue::Reserved)
        } else {
            frame.set_local_variable(index, value)
        }
    }

    pub fn method_name(&self) -> Result<&str> {
        Ok(&self.frames.last().context("no frame found")?.method_name)
    }

    pub fn offset_pc(&mut self, offset: i16) -> Result<()> {
        self.frames
            .last_mut()
            .context("no frame found")?
            .offset_pc(offset)
    }

    pub fn method_descriptor(&self) -> Result<MethodDescriptor> {
        Ok(self
            .frames
            .last()
            .context("no frame found")?
            .method_descriptor())
    }

    pub fn local_variables(&self) -> Result<Vec<FrameValue>> {
        Ok(self
            .frames
            .last()
            .context("no frame found")?
            .local_variables
            .clone())
    }

    pub fn caller_class(&self) -> Result<&ClassIdentifier> {
        Ok(&self
            .frames
            .get(self.frames.len() - 2)
            .context("no caller found")?
            .class)
    }
}

#[derive(Debug)]
struct Frame {
    method_name: String,
    method_descriptor: MethodDescriptor,
    operand_stack: Vec<FrameValue>,
    local_variables: Vec<FrameValue>,
    code: Vec<u8>,
    pc: usize,
    class: ClassIdentifier,
}

impl Frame {
    fn new(
        method_name: String,
        method_descriptor: MethodDescriptor,
        local_variables: Vec<FrameValue>,
        code: Vec<u8>,
        class: ClassIdentifier,
    ) -> Self {
        let mut lvs = Vec::new();
        for lv in &local_variables {
            if matches!(lv, FrameValue::Long(_)) || matches!(lv, FrameValue::Double(_)) {
                lvs.push(lv.clone());
                lvs.push(FrameValue::Reserved);
            } else {
                lvs.push(lv.clone());
            }
        }
        Self {
            method_name,
            method_descriptor,
            operand_stack: Vec::new(),
            local_variables: lvs,
            code,
            pc: 0,
            class,
        }
    }

    fn push_operand(&mut self, operand: FrameValue) {
        trace!("pushing operand: {operand:?}");
        self.operand_stack.push(operand);
    }

    fn pop_operands(&mut self, n: usize) -> Result<Vec<FrameValue>> {
        let mut operands = Vec::new();
        for _ in 0..n {
            operands.push(
                self.operand_stack
                    .pop()
                    .context("no operands in operand stack")?,
            );
        }
        operands.reverse();

        Ok(operands)
    }

    fn pop_operand(&mut self) -> Result<FrameValue> {
        self.operand_stack
            .pop()
            .context("no operands in operand stack")
    }

    fn pop_int(&mut self) -> Result<i32> {
        if let Some(FrameValue::Int(val)) = self.operand_stack.pop() {
            return Ok(val);
        }

        bail!("no int found on top of operand stack")
    }

    fn current_instruction(&mut self) -> Result<Instruction> {
        Instruction::new(&self.code[self.pc..])
            .context(format!("no instruction found at pc {}", self.pc))
    }

    fn local_variable(&self, index: usize) -> Result<FrameValue> {
        self.local_variables
            .get(index)
            .context(format!("no local variable at index {index}"))
            .cloned()
    }

    fn set_local_variable(&mut self, index: usize, value: FrameValue) -> Result<()> {
        if index > self.local_variables.len() {
            bail!("index out of bounds of local variables")
        }

        self.local_variables.insert(index, value);
        Ok(())
    }

    fn offset_pc(&mut self, offset: i16) -> Result<()> {
        if offset < 0 {
            let offset = offset.unsigned_abs() as usize;
            if offset > self.pc {
                bail!("pc cannot be negative")
            }

            self.pc -= offset;
        } else {
            self.pc += offset as usize;
        }

        Ok(())
    }

    fn method_descriptor(&self) -> MethodDescriptor {
        self.method_descriptor.clone()
    }
}

#[derive(Debug, Clone)]
pub enum FrameValue {
    Reserved,
    Reference(ReferenceValue),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl FrameValue {
    pub fn is_reference(&self) -> bool {
        matches!(self, Self::Reference(_))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, FrameValue::Reference(ReferenceValue::Null))
    }

    pub fn reference(&self) -> Result<&ReferenceValue> {
        if let Self::Reference(reference) = self {
            Ok(reference)
        } else {
            bail!("frame value is not a reference")
        }
    }

    pub fn int(&self) -> Result<i32> {
        if let Self::Int(int) = self {
            Ok(*int)
        } else {
            bail!("frame value is not a int")
        }
    }

    pub fn long(&self) -> Result<i64> {
        if let Self::Long(long) = self {
            Ok(*long)
        } else {
            bail!("frame value is not a long")
        }
    }

    pub fn float(&self) -> Result<f32> {
        if let Self::Float(float) = self {
            Ok(*float)
        } else {
            bail!("frame value is not a float")
        }
    }

    pub fn double(&self) -> Result<f64> {
        if let Self::Double(double) = self {
            Ok(*double)
        } else {
            bail!("frame value is not a double")
        }
    }
}
