use crate::ClassIdentifier;
use anyhow::{Context, Result};

#[derive(Debug, Default)]
pub struct Stack {
    frames: Vec<Frame>,
}

impl Stack {
    pub fn push(&mut self, method_name: String) {
        self.frames.push(Frame::new(method_name));
    }

    pub fn push_operand(&mut self, operand: OperandValue) -> Result<()> {
        let frame = self.frames.last_mut().context("no frame found")?;
        frame.push_operand(operand);
        Ok(())
    }
}

#[derive(Debug)]
struct Frame {
    method_name: String,
    operand_stack: Vec<OperandValue>,
}

impl Frame {
    fn new(method_name: String) -> Self {
        Self {
            method_name,
            operand_stack: Vec::new(),
        }
    }

    fn push_operand(&mut self, operand: OperandValue) {
        self.operand_stack.push(operand);
    }
}

#[derive(Debug)]
pub enum OperandValue {
    ClassReference(ClassIdentifier),
}
