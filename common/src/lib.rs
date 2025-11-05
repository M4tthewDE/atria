use anyhow::Context;
use anyhow::bail;
use std::fmt::Debug;
use std::fmt::Display;
use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug, PartialEq, Clone)]
pub struct ThreadId(i64);

impl From<i64> for ThreadId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub struct HeapId(u64);

impl From<u64> for HeapId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Identifies a class using package and name
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct ClassIdentifier {
    pub package: String,
    pub name: String,
}

impl ClassIdentifier {
    pub fn new(package: String, name: String) -> Self {
        Self { package, name }
    }

    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.replace("/", ".").replace(";", "");
        let raw: String = raw.chars().skip_while(|c| *c == 'L' || *c == '[').collect();

        match raw.as_str() {
            "B" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Byte".to_owned(),
                });
            }
            "C" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Character".to_owned(),
                });
            }
            "D" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Double".to_owned(),
                });
            }
            "F" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Float".to_owned(),
                });
            }
            "I" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Integer".to_owned(),
                });
            }
            "J" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Long".to_owned(),
                });
            }
            "S" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Short".to_owned(),
                });
            }
            "Z" => {
                return Ok(Self {
                    package: "java.lang".to_owned(),
                    name: "Boolean".to_owned(),
                });
            }
            _ => {}
        }

        let mut parts: Vec<&str> = raw.split('.').collect();
        let name = parts
            .last()
            .context("invalid class identifier {value}")?
            .to_string();
        parts.truncate(parts.len() - 1);

        Ok(Self {
            package: parts.join("."),
            name,
        })
    }

    pub fn path(&self) -> Result<String> {
        let mut path = PathBuf::new();
        for package in self.package.split('.') {
            path.push(package);
        }

        path.push(format!("{}.class", self.name));
        path.to_str()
            .map(|p| p.to_owned())
            .clone()
            .context("unable to build path string")
    }
}

impl Display for ClassIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Debug for ClassIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.package, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum ReferenceValue {
    HeapItem(HeapId),
    Class(ClassIdentifier),
    Null,
}

impl ReferenceValue {
    pub fn heap_id(&self) -> Result<&HeapId> {
        match self {
            ReferenceValue::HeapItem(heap_id) => Ok(heap_id),
            _ => bail!("no heap id found"),
        }
    }

    pub fn class_identifier(&self) -> Result<&ClassIdentifier> {
        match self {
            ReferenceValue::Class(class_identifier) => Ok(class_identifier),
            _ => bail!("no class identifier found"),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_class(&self) -> bool {
        matches!(self, Self::Class(_))
    }
}

#[derive(Clone, Debug)]
pub enum FieldValue {
    Reference(ReferenceValue),
    Integer(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl FieldValue {
    pub fn heap_id(&self) -> Result<&HeapId> {
        match self {
            FieldValue::Reference(reference_value) => reference_value.heap_id(),
            _ => bail!("no heap id found"),
        }
    }

    pub fn long(&self) -> Result<i64> {
        match self {
            Self::Long(val) => Ok(*val),
            _ => bail!("no long found"),
        }
    }

    pub fn int(&self) -> Result<i32> {
        match self {
            Self::Integer(val) => Ok(*val),
            _ => bail!("no int found"),
        }
    }

    pub fn reference(&self) -> Result<ReferenceValue> {
        match self {
            Self::Reference(val) => Ok(val.clone()),
            _ => bail!("no reference found"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slashes() {
        let class_identifier = ClassIdentifier::parse("java/lang/System").unwrap();
        assert_eq!(class_identifier.package, "java.lang");
        assert_eq!(class_identifier.name, "System");
    }

    #[test]
    fn test_parse_dots() {
        let class_identifier = ClassIdentifier::parse("java.lang.System").unwrap();
        assert_eq!(class_identifier.package, "java.lang");
        assert_eq!(class_identifier.name, "System");
    }

    #[test]
    fn test_parse_no_package() {
        let class_identifier = ClassIdentifier::parse("System").unwrap();
        assert_eq!(class_identifier.package, "");
        assert_eq!(class_identifier.name, "System");
    }

    #[test]
    fn test_parse_lowercase_class() {
        let class_identifier = ClassIdentifier::parse("java.lang").unwrap();
        assert_eq!(class_identifier.package, "java");
        assert_eq!(class_identifier.name, "lang");
    }

    #[test]
    fn test_parse_from_field_descriptor() {
        let class_identifier = ClassIdentifier::parse("Ljava.lang.System").unwrap();
        assert_eq!(class_identifier.package, "java.lang");
        assert_eq!(class_identifier.name, "System");
    }

    #[test]
    fn test_parse_from_field_descriptor_array() {
        let class_identifier = ClassIdentifier::parse("[[Ljava.lang.System").unwrap();
        assert_eq!(class_identifier.package, "java.lang");
        assert_eq!(class_identifier.name, "System");
    }

    #[test]
    fn test_parse_from_field_descriptor_primitive_int() {
        let class_identifier = ClassIdentifier::parse("I").unwrap();
        assert_eq!(class_identifier.package, "java.lang");
        assert_eq!(class_identifier.name, "Integer");
    }

    #[test]
    fn test_path() {
        let class_identifier = ClassIdentifier::new("java.lang".to_owned(), "System".to_owned());
        let path = class_identifier.path().unwrap();
        assert_eq!(path, "java/lang/System.class");
    }
}
