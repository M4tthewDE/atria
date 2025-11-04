use crate::thread::FrameValue;
use anyhow::{Context, Result, bail};
use common::ClassIdentifier;
use common::ReferenceValue;

use crate::thread::JvmThread;

pub fn run(
    jvm: &mut JvmThread,
    name: &str,
    operands: Vec<FrameValue>,
) -> Result<Option<FrameValue>> {
    match name {
        "registerNatives" => Ok(None),
        "initClassName" => {
            if let FrameValue::Reference(ReferenceValue::Class(identifier)) =
                operands.first().context("no operands provided")?
            {
                let object_id = jvm.new_string(format!("{identifier:?}").to_string())?;
                Ok(Some(FrameValue::Reference(ReferenceValue::HeapItem(
                    object_id,
                ))))
            } else {
                bail!("first operand has to be a reference")
            }
        }
        "desiredAssertionStatus0" => Ok(Some(FrameValue::Int(0))),
        "getPrimitiveClass" => {
            let operand = operands.first().context("operands are empty")?;
            let heap_id = if let FrameValue::Reference(ReferenceValue::HeapItem(heap_id)) = operand
            {
                heap_id
            } else {
                bail!("no reference found, instead: {operand:?}")
            };

            let value_heap_item = jvm.heap_get_field(heap_id, "value")?;
            let (_, primitive_array) = jvm.get_primitive_array(value_heap_item.heap_id()?)?;

            let bytes: Vec<u8> = primitive_array
                .iter()
                .map(|p| p.byte())
                .collect::<Result<Vec<u8>>>()?;
            let name = String::from_utf8(bytes)?;
            match name.as_str() {
                "int" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Integer".to_owned()),
                )))),
                "boolean" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Boolean".to_owned()),
                )))),
                "byte" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Byte".to_owned()),
                )))),
                "short" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Short".to_owned()),
                )))),
                "char" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Character".to_owned()),
                )))),
                "double" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Double".to_owned()),
                )))),
                "long" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Long".to_owned()),
                )))),
                "float" => Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                    ClassIdentifier::new("java.lang".to_owned(), "Float".to_owned()),
                )))),
                _ => bail!("invalid primitive class name: '{name}'"),
            }
        }
        "forName0" => {
            let heap_id = operands
                .first()
                .context("no first operand")?
                .reference()?
                .heap_id()?;
            let byte_value = jvm.heap_get_field(heap_id, "value")?;
            let (_, primitive_array) = jvm.get_primitive_array(byte_value.heap_id()?)?;
            let bytes: Vec<u8> = primitive_array
                .iter()
                .map(|p| p.byte())
                .collect::<Result<Vec<u8>>>()?;
            let name = String::from_utf8(bytes)?;
            Ok(Some(FrameValue::Reference(ReferenceValue::Class(
                ClassIdentifier::parse(&name)?,
            ))))
        }
        "isPrimitive" => {
            let value = match format!(
                "{:?}",
                operands
                    .first()
                    .context("no first operand")?
                    .reference()?
                    .class_identifier()?
            )
            .as_str()
            {
                "java.lang.Byte" => FrameValue::Int(1),
                "java.lang.Character" => FrameValue::Int(1),
                "java.lang.Double" => FrameValue::Int(1),
                "java.lang.Float" => FrameValue::Int(1),
                "java.lang.Integer" => FrameValue::Int(1),
                "java.lang.Long" => FrameValue::Int(1),
                "java.lang.Short" => FrameValue::Int(1),
                "java.lang.Boolean" => FrameValue::Int(1),
                "java.lang.Void" => FrameValue::Int(1),
                _ => FrameValue::Int(0),
            };
            Ok(Some(value))
        }
        _ => bail!("TODO"),
    }
}
