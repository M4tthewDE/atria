use crate::{ClassIdentifier, ReferenceValue, instruction::Instruction};
use anyhow::{Context, Result, bail};
use tracing::trace;

#[derive(Debug, Default)]
pub struct Stack {
    frames: Vec<Frame>,
}

impl Stack {
    pub fn push(
        &mut self,
        method_name: String,
        local_variables: Vec<FrameValue>,
        code: Vec<u8>,
        class: ClassIdentifier,
    ) {
        self.frames
            .push(Frame::new(method_name, local_variables, code, class));
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
        frame.set_local_variable(index, value)
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
}

#[derive(Debug)]
struct Frame {
    method_name: String,
    operand_stack: Vec<FrameValue>,
    local_variables: Vec<FrameValue>,
    code: Vec<u8>,
    pc: usize,
    class: ClassIdentifier,
}

impl Frame {
    fn new(
        method_name: String,
        local_variables: Vec<FrameValue>,
        code: Vec<u8>,
        class: ClassIdentifier,
    ) -> Self {
        Self {
            method_name,
            operand_stack: Vec::new(),
            local_variables,
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
            .context("no local variable at index {index}")
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
}

#[derive(Debug, Clone)]
pub enum FrameValue {
    ReturnAddress,
    Reference(ReferenceValue),
    Int(i32),
}

impl FrameValue {
    pub fn is_reference(&self) -> bool {
        matches!(self, Self::Reference(_))
    }

    pub fn is_return_address(&self) -> bool {
        matches!(self, FrameValue::ReturnAddress)
    }

    pub fn is_array(&self) -> bool {
        matches!(self, FrameValue::Reference(ReferenceValue::Array(_, _)))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, FrameValue::Reference(ReferenceValue::Null))
    }
}
