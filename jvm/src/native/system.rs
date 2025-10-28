use std::hash::Hash;
use std::hash::Hasher;
use std::{hash::DefaultHasher, time::Instant};

use anyhow::{Context, Result, bail};

use crate::{stack::FrameValue, thread::JvmThread};

pub fn run(
    jvm: &mut JvmThread,
    name: &str,
    operands: Vec<FrameValue>,
) -> Result<Option<FrameValue>> {
    match name {
        "registerNatives" => Ok(None),
        "nanoTime" => {
            let now = Instant::now();
            let elapsed = now.duration_since(jvm.creation_time).as_nanos();
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

            if let Ok((_, src_arr)) = jvm.get_primitive_array(src) {
                for i in src_pos..src_pos + length {
                    let val = src_arr
                        .get(i as usize)
                        .context("TODO: throw IndexOutOfBoundsException")?;
                    jvm.store_into_primitive_array(dest, dest_pos as usize, val.clone())?;
                    dest_pos += 1;
                }

                return Ok(None);
            }

            bail!("arraycopy for reference array");
        }

        _ => bail!("TODO"),
    }
}
