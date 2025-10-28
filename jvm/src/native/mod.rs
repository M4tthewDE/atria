use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

use crate::{ClassIdentifier, ReferenceValue, stack::FrameValue, thread::JvmThread};

mod class;
mod misc;
mod reflect;
mod runtime;
mod system;
mod thread;
mod r#unsafe;

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
        "java.lang.Class" => class::run(jvm, name, operands),
        "java.lang.Runtime" => runtime::run(name),
        "jdk.internal.misc.Unsafe" => r#unsafe::run(jvm, name, operands),
        "java.lang.Thread" => thread::run(jvm, name, operands),
        "java.lang.System" => system::run(jvm, name, operands),
        "jdk.internal.misc.CDS" => misc::run_cds(name),
        "jdk.internal.misc.VM" => misc::run_vm(name),
        "jdk.internal.reflect.Reflection" => reflect::run(jvm, name),
        "java.lang.Object" => match name {
            "getClass" => {
                let heap_id = operands
                    .first()
                    .context("operands are empty")?
                    .reference()?
                    .heap_id()?;
                let heap_item = jvm.heap_get(heap_id)?;
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
            _ => bail!("TODO"),
        },
        "java.security.AccessController" => match name {
            // TODO: this will be used at some point
            "getStackAccessControlContext" => Ok(Some(FrameValue::Reference(ReferenceValue::Null))),
            _ => bail!("TODO"),
        },
        "java.lang.ref.Reference" => match name {
            // TODO: this will be used at some point
            "waitForReferencePendingList" => {
                warn!("parking this thread, reference pending list not implemented yet");
                std::thread::park();
                Ok(None)
            }
            _ => bail!("TODO"),
        },
        "java.lang.ClassLoader" => match name {
            // TODO: this will be used at some point
            "registerNatives" => Ok(None),
            _ => bail!("TODO"),
        },
        "java.lang.Float" => match name {
            "floatToRawIntBits" => {
                let float = operands
                    .first()
                    .context("no float to convert to int")?
                    .float()?;
                Ok(Some(FrameValue::Int(float as i32)))
            }
            _ => bail!("TODO"),
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
            _ => bail!("TODO"),
        },
        "jdk.internal.util.SystemProps$Raw" => match name {
            "platformProperties" => {
                let string_class = ClassIdentifier::new("java.lang.String")?;
                jvm.initialize(&string_class)?;
                let array = jvm.allocate_array(string_class, 39)?;
                Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(array))))
            }
            "vmProperties" => {
                let string_class = ClassIdentifier::new("java.lang.String")?;
                jvm.initialize(&string_class)?;
                let array = jvm.allocate_array(string_class, 0)?;
                Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(array))))
            }
            _ => bail!("TODO"),
        },
        _ => bail!("native method {name} on {class_identifier:?} not implemented",),
    }
}
