use anyhow::{Context, Result, bail};

#[derive(Debug)]
pub struct FieldDescriptor {
    pub field_type: FieldType,
}

impl FieldDescriptor {
    pub fn new(raw: &str) -> Result<Self> {
        Ok(Self {
            field_type: FieldType::new(raw)?,
        })
    }
}

#[derive(Debug)]
pub struct MethodDescriptor {
    pub return_descriptor: ReturnDescriptor,
    pub parameters: Vec<FieldType>,
}

impl MethodDescriptor {
    pub fn new(raw: &str) -> Result<Self> {
        let end_of_parameter_descriptor =
            raw.find(")").context("invalid method descriptor: no ')'")?;

        let mut raw_parameter_descriptor = &raw[1..end_of_parameter_descriptor];
        let parameters = if raw_parameter_descriptor.is_empty() {
            Vec::new()
        } else {
            let mut parameters = Vec::new();
            loop {
                let parameter = FieldType::new(raw_parameter_descriptor)?;

                if parameter.length() == raw_parameter_descriptor.len() {
                    parameters.push(parameter);
                    break;
                }

                raw_parameter_descriptor = &raw_parameter_descriptor[parameter.length()..];
                parameters.push(parameter);
            }

            parameters
        };

        let raw_return_descriptor = &raw[end_of_parameter_descriptor + 1..];

        let return_descriptor = if raw_return_descriptor == "V" {
            ReturnDescriptor::Void
        } else {
            ReturnDescriptor::FieldType(FieldType::new(raw_return_descriptor)?)
        };

        Ok(Self {
            return_descriptor,
            parameters,
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum ReturnDescriptor {
    Void,
    FieldType(FieldType),
}

#[derive(Debug, PartialEq)]
pub enum FieldType {
    BaseType(BaseType),
    ObjectType { class_name: String },
    ComponentType(Box<FieldType>),
}

#[derive(Debug, PartialEq)]
pub enum BaseType {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
}

impl FieldType {
    fn new(raw: &str) -> Result<Self> {
        Ok(match &raw[0..1] {
            "B" => Self::BaseType(BaseType::Byte),
            "C" => Self::BaseType(BaseType::Char),
            "D" => Self::BaseType(BaseType::Double),
            "F" => Self::BaseType(BaseType::Float),
            "I" => Self::BaseType(BaseType::Int),
            "J" => Self::BaseType(BaseType::Long),
            "S" => Self::BaseType(BaseType::Short),
            "Z" => Self::BaseType(BaseType::Boolean),
            "L" => Self::ObjectType {
                class_name: raw[1..raw.len() - 1].to_string(),
            },
            "[" => Self::ComponentType(Box::new(Self::new(&raw[1..])?)),
            _ => bail!("unknown field type: {raw}"),
        })
    }

    fn length(&self) -> usize {
        match self {
            FieldType::BaseType(_) => 1,
            FieldType::ObjectType { class_name } => class_name.len() + 2,
            FieldType::ComponentType(field_type) => field_type.length() + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_descriptor() {
        let descriptor = MethodDescriptor::new("(IDLjava/lang/Thread;)Ljava/lang/Object;").unwrap();
        assert_eq!(
            descriptor.parameters,
            vec![
                FieldType::BaseType(BaseType::Int),
                FieldType::BaseType(BaseType::Double),
                FieldType::ObjectType {
                    class_name: "java/lang/Thread".to_string()
                }
            ]
        );

        assert_eq!(
            descriptor.return_descriptor,
            ReturnDescriptor::FieldType(FieldType::ObjectType {
                class_name: "java/lang/Object".to_string()
            })
        );
    }

    #[test]
    fn method_descriptor_arrays() {
        let descriptor = MethodDescriptor::new("([[[D)V").unwrap();
        assert_eq!(
            descriptor.parameters,
            vec![FieldType::ComponentType(Box::new(
                FieldType::ComponentType(Box::new(FieldType::ComponentType(Box::new(
                    FieldType::BaseType(BaseType::Double)
                ))))
            ))]
        );

        assert_eq!(descriptor.return_descriptor, ReturnDescriptor::Void);
    }
}
