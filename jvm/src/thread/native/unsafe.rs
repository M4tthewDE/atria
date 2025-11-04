use crate::thread::FrameValue;
use anyhow::{Context, Result, bail};

use crate::{class::FieldValue, heap::HeapItem, thread::JvmThread};

pub fn run(
    jvm: &mut JvmThread,
    name: &str,
    operands: Vec<FrameValue>,
) -> Result<Option<FrameValue>> {
    match name {
        "registerNatives" => Ok(None),
        "storeFence" => Ok(None),
        "arrayBaseOffset0" => Ok(Some(FrameValue::Int(0))),
        "arrayIndexScale0" => Ok(Some(FrameValue::Int(0))),
        "objectFieldOffset1" => {
            let class = operands.get(1).context("no class operand found")?;
            let name = operands.get(2).context("no String operand found")?;
            let byte_value = jvm.heap_get_field(name.reference()?.heap_id()?, "value")?;
            let (_, primitive_array) = jvm.get_primitive_array(byte_value.heap_id()?)?;
            let bytes: Vec<u8> = primitive_array
                .iter()
                .map(|p| p.byte())
                .collect::<Result<Vec<u8>>>()?;
            let name = String::from_utf8(bytes)?;
            let class = jvm.class(class.reference()?.class_identifier()?)?;
            let offset = jvm
                .default_instance_fields(&class, 0)?
                .get(&name)
                .context(format!("no field '{name}' found"))?
                .offset();
            Ok(Some(FrameValue::Long(offset)))
        }
        "compareAndSetInt" => {
            let object = operands.get(1).context("no 'object' operand found")?;
            let offset = operands
                .get(2)
                .context("no 'offset' operand found")?
                .long()?;
            let expected = operands
                .get(3)
                .context("no 'expected' operand found")?
                .int()?;
            let x = operands.get(4).context("no 'x' operand found")?.int()?;
            let heap_id = object.reference()?.heap_id()?;

            let object = jvm.heap_get(heap_id)?;

            let class = jvm.class(&object.class_identifier()?)?;
            for (name, field) in jvm.default_instance_fields(&class, 0)? {
                if field.offset() == offset
                    && jvm.heap_get_field(heap_id, &name)?.int()? == expected
                {
                    jvm.heap_set_field(heap_id, &name, FieldValue::Integer(x))?;
                    return Ok(Some(FrameValue::Int(1)));
                }
            }

            bail!("no field with offset '{offset}' found");
        }
        "compareAndSetLong" => {
            let object = operands.get(1).context("no 'object' operand found")?;
            let offset = operands
                .get(2)
                .context("no 'offset' operand found")?
                .long()?;
            let expected = operands
                .get(3)
                .context("no 'expected' operand found")?
                .long()?;
            let x = operands.get(4).context("no 'x' operand found")?.long()?;
            let heap_id = object.reference()?.heap_id()?;

            let object = jvm.heap_get(heap_id)?;

            let class = jvm.class(&object.class_identifier()?)?;
            for (name, field) in jvm.default_instance_fields(&class, 0)? {
                if field.offset() == offset
                    && jvm.heap_get_field(heap_id, &name)?.long()? == expected
                {
                    jvm.heap_set_field(heap_id, &name, FieldValue::Long(x))?;
                    return Ok(Some(FrameValue::Int(1)));
                }
            }

            bail!("no field with offset '{offset}' found");
        }
        "compareAndSetReference" => {
            let object = operands.get(1).context("no 'object' operand found")?;
            let offset = operands
                .get(2)
                .context("no 'offset' operand found")?
                .long()?;
            let expected = operands
                .get(3)
                .context("no 'expected' operand found")?
                .reference()?;
            let x = operands
                .get(4)
                .context("no 'x' operand found")?
                .reference()?;
            let heap_id = object.reference()?.heap_id()?;

            let object = jvm.heap_get(heap_id)?;

            if object.is_array() {
                jvm.store_into_reference_array(heap_id, offset as usize, x.clone())?;
                return Ok(Some(FrameValue::Int(1)));
            }

            let class = jvm.class(&object.class_identifier()?)?;
            for (name, field) in jvm.default_instance_fields(&class, 0)? {
                if field.offset() == offset
                    && jvm.heap_get_field(heap_id, &name)?.reference()? == *expected
                {
                    jvm.heap_set_field(heap_id, &name, FieldValue::Reference(x.clone()))?;
                    return Ok(Some(FrameValue::Int(1)));
                }
            }

            bail!("no field with offset '{offset}' found");
        }
        "getReferenceVolatile" => {
            let object = operands.get(1).context("no 'object' operand found")?;
            let offset = operands
                .get(2)
                .context("no 'offset' operand found")?
                .long()?;
            let heap_id = object.reference()?.heap_id()?;
            let object = jvm.heap_get(heap_id)?;

            if let HeapItem::ReferenceArray { values, .. } = object {
                let value = values.get(offset as usize).context("no value at offset")?;
                return Ok(Some(FrameValue::Reference(value.clone())));
            }

            let class = jvm.class(&object.class_identifier()?)?;

            for (name, field) in jvm.default_instance_fields(&class, 0)? {
                if field.offset() == offset {
                    let field_value = jvm.heap_get_field(heap_id, &name)?;
                    return Ok(Some(field_value.into()));
                }
            }

            bail!("no field with offset '{offset}' found");
        }
        _ => bail!("TODO"),
    }
}
