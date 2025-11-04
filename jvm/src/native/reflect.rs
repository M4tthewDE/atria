use anyhow::{Result, bail};

use crate::{ReferenceValue, stack::FrameValue, thread::JvmThread};

pub fn run(jvm: &mut JvmThread, name: &str) -> Result<Option<FrameValue>> {
    match name {
        "getCallerClass" => {
            let caller_class = jvm.caller_class()?;
            Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                caller_class.clone(),
            ))))
        }
        _ => bail!("TODO"),
    }
}
