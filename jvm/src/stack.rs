use crate::{
    ClassIdentifier,
    code::{Code, Instruction},
};
use anyhow::{Context, Result, bail};
use tracing::debug;

#[derive(Debug, Default)]
pub struct Stack {
    frames: Vec<Frame>,
}

impl Stack {
    pub fn push(&mut self, method_name: String, local_variables: Vec<FrameValue>, code: Code) {
        self.frames
            .push(Frame::new(method_name, local_variables, code));
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

    pub fn done(&self) -> bool {
        self.frames.is_empty()
    }
}

#[derive(Debug)]
struct Frame {
    method_name: String,
    operand_stack: Vec<FrameValue>,
    local_variables: Vec<FrameValue>,
    code: Code,
    pc: usize,
}

impl Frame {
    fn new(method_name: String, local_variables: Vec<FrameValue>, code: Code) -> Self {
        Self {
            method_name,
            operand_stack: Vec::new(),
            local_variables,
            code,
            pc: 0,
        }
    }

    fn push_operand(&mut self, operand: FrameValue) {
        debug!("pushing operand: {operand:?}");
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

    pub fn pop_int(&mut self) -> Result<i32> {
        if let Some(FrameValue::Int(val)) = self.operand_stack.pop() {
            return Ok(val);
        }

        bail!("no int found on top of operand stack")
    }

    pub fn current_instruction(&mut self) -> Result<Instruction> {
        let instruction = self
            .code
            .instructions
            .get(self.pc)
            .context(format!("no instruction found at pc {}", self.pc))?;
        self.pc += 1;
        Ok(instruction.clone())
    }
}

#[derive(Debug)]
pub enum FrameValue {
    ClassReference(ClassIdentifier),
    ReferenceArray(ClassIdentifier, Vec<Reference>),
    Int(i32),
}

#[derive(Debug, Clone)]
pub enum Reference {
    Null,
}
