use crate::ClassIdentifier;
use anyhow::{Context, Result, bail};
use tracing::debug;

#[derive(Debug, Default)]
pub struct Stack {
    frames: Vec<Frame>,
}

impl Stack {
    pub fn push(&mut self, method_name: String, local_variables: Vec<FrameValue>) {
        self.frames.push(Frame::new(method_name, local_variables));
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

    pub fn pop_int(&mut self) -> Result<i32> {
        let frame = self.frames.last_mut().context("no frame found")?;
        frame.pop_int()
    }
}

#[derive(Debug)]
struct Frame {
    method_name: String,
    operand_stack: Vec<FrameValue>,
    local_variables: Vec<FrameValue>,
}

impl Frame {
    fn new(method_name: String, local_variables: Vec<FrameValue>) -> Self {
        Self {
            method_name,
            operand_stack: Vec::new(),
            local_variables,
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

    pub fn pop_int(&mut self) -> Result<i32> {
        if let Some(int) = self.operand_stack.pop() {
            if let FrameValue::Int(val) = int {
                return Ok(val);
            }
        }

        bail!("no int found on top of operand stack")
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
