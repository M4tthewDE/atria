use common::{FrameValue, ReferenceValue};

use anyhow::{Context, Result, bail};
use common::ClassIdentifier;
use tracing::info;

use crate::thread::JvmThread;

pub fn run(
    jvm: &mut JvmThread,
    class_identifier: &ClassIdentifier,
    name: &str,
    operands: Vec<FrameValue>,
) -> Result<Option<FrameValue>> {
    info!(
        "running native method '{name}' in {:?} with operands {:?}",
        class_identifier, operands
    );

    match format!("{:?}", class_identifier).as_str() {
        "java.io.PrintStream" => match name {
            "println" => {
                let heap_id = operands
                    .get(1)
                    .context("no first operand")?
                    .reference()?
                    .heap_id()?;
                let byte_value = jvm.heap_get_field(heap_id, "value")?;
                let (_, primitive_array) = jvm.get_primitive_array(byte_value.heap_id()?)?;
                let bytes: Vec<u8> = primitive_array
                    .iter()
                    .map(|p| p.byte())
                    .collect::<Result<Vec<u8>>>()?;
                let value = String::from_utf8(bytes)?;
                println!("{value}");
                Ok(None)
            }
            _ => bail!("native method {name} on {class_identifier:?} not implemented",),
        },
        "java.lang.Class" => match name {
            "getName" => {
                let class = operands
                    .first()
                    .context("no first operand")?
                    .reference()?
                    .class_identifier()?;
                let class_name = jvm.new_string(format!("{class:?}"))?;
                Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(
                    class_name,
                ))))
            }
            _ => bail!("native method {name} on {class_identifier:?} not implemented",),
        },
        _ => bail!("native method {name} on {class_identifier:?} not implemented",),
    }
}
