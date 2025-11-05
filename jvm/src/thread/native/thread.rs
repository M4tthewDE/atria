use anyhow::{Context, Result, bail};
use common::{FrameValue, ReferenceValue};

use crate::thread::JvmThread;

pub fn run(
    jvm: &mut JvmThread,
    name: &str,
    operands: Vec<FrameValue>,
) -> Result<Option<FrameValue>> {
    match name {
        "registerNatives" => Ok(None),
        "currentThread" => Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(
            jvm.thread_object().context("no current thread found")?,
        )))),
        "setPriority0" => {
            let objectref = operands.first().context("no first operand")?;
            let priority = operands.get(1).context("no second operand")?;
            let heap_id = objectref.reference()?.heap_id()?;
            jvm.heap_set_field(heap_id, "priority", priority.clone().into())?;
            Ok(None)
        }
        "start0" => {
            let objectref = operands
                .first()
                .context("no first operand, no thread to start")?;
            let heap_id = objectref.reference()?.heap_id()?;
            let heap_item = jvm.heap_get(heap_id)?;
            let object = heap_item.object()?;
            let class_identifier = object.class();

            let name = jvm.heap_get_field(heap_id, "name")?;
            let byte_value = jvm.heap_get_field(name.heap_id()?, "value")?;
            let (_, primitive_array) = jvm.get_primitive_array(byte_value.heap_id()?)?;
            let bytes: Vec<u8> = primitive_array
                .iter()
                .map(|p| p.byte())
                .collect::<Result<Vec<u8>>>()?;
            let name = String::from_utf8(bytes)?;

            let new_thread = jvm.new_thread(name.to_string());

            JvmThread::run_with_method(
                new_thread,
                class_identifier.clone(),
                "run".to_string(),
                "()V".to_string(),
            );

            Ok(None)
        }
        _ => bail!("TODO"),
    }
}
