use std::{
    char,
    collections::HashSet,
    io::{Read, Seek},
};

use anyhow::{Result, bail};
use tracing::trace;

use crate::{
    class::constant_pool::{ConstantPool, CpIndex},
    util::{u1, u2, u4, vec},
};

const CONSTANT_VALUE_ATTR_NAME: &str = "ConstantValue";
const RUNTIME_VISIBLE_ANNOTATIONS_ATTR_NAME: &str = "RuntimeVisibleAnnotations";
const CODE_ATTR_NAME: &str = "Code";
const LINE_NUMBER_TABLE_ATTR_NAME: &str = "LineNumberTable";
const LOCAL_VARIABLE_TABLE_ATTR_NAME: &str = "LocalVariableTable";
const STACK_MAP_TABLE_ATTR_NAME: &str = "StackMapTable";
const EXCEPTIONS_ATTR_NAME: &str = "Exceptions";
const LOCAL_VARIABLE_TYPE_TABLE_ATTR_NAME: &str = "LocalVariableTypeTable";
const SIGNATURE_ATTR_NAME: &str = "Signature";
const DEPRECATED_ATTR_NAME: &str = "Deprecated";
const SOURCE_FILE_ATTR_NAME: &str = "SourceFile";
const NEST_MEMBERS_ATTR_NAME: &str = "NestMembers";
const BOOTSTRAP_METHODS_ATTR_NAME: &str = "BootstrapMethods";
const INNER_CLASSES_ATTR_NAME: &str = "InnerClasses";
const METHOD_PARAMETERS_ATTR_NAME: &str = "MethodParameters";
const NEST_HOST_ATTR_NAME: &str = "NestHost";
const ENCLOSING_METHOD_ATTR_NAME: &str = "EnclosingMethod";
const PERMITTED_SUBCLASSES_ATTR_NAME: &str = "PermittedSubclasses";

#[derive(Clone, Debug, PartialEq)]
pub enum Attribute {
    ConstantValue {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        constant_value_index: CpIndex,
    },
    RuntimeVisibleAnnoations {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        annotations: Vec<Annotation>,
    },
    Code {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        max_stack: u16,
        max_locals: u16,
        code: Vec<u8>,
        exception_table: Vec<ExceptionHandler>,
        attributes: Vec<Attribute>,
    },
    LineNumberTable {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        line_number_table: Vec<LineNumberTableEntry>,
    },
    LocalVariableTable {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        local_variable_table: Vec<LocalVariableTableEntry>,
    },
    StackMapTable {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        entries: Vec<StackMapTableEntry>,
    },
    Exceptions {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        index_table: Vec<CpIndex>,
    },
    LocalVariableTypeTable {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        local_variable_type_table: Vec<LocalVariableTypeTableEntry>,
    },
    Signature {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        signature_index: CpIndex,
    },
    Deprecated {
        attribute_name_index: CpIndex,
        attribute_length: u32,
    },
    SourceFile {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        source_file_index: CpIndex,
    },
    NestMembers {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        classes: Vec<CpIndex>,
    },
    BootstrapMethods {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        methods: Vec<BootStrapMethod>,
    },
    InnerClasses {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        inner_classes: Vec<InnerClass>,
    },
    MethodParameters {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        parameters: Vec<MethodParameter>,
    },
    NestHost {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        host_class_index: CpIndex,
    },
    EnclosingMethod {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        class_index: CpIndex,
        method_index: CpIndex,
    },
    PermittedSubclasses {
        attribute_name_index: CpIndex,
        attribute_length: u32,
        classes: Vec<CpIndex>,
    },
}

impl Attribute {
    pub fn new(r: &mut (impl Read + Seek), cp: &ConstantPool) -> Result<Self> {
        let attribute_name_index = u2(r)?.into();
        let attribute_length = u4(r)?;

        let name = cp.utf8(&attribute_name_index)?;

        trace!("parsing {name} attribute");
        trace!("attribute_length: {attribute_length}");
        let before = r.stream_position()?;

        let attr = Ok(match name {
            CONSTANT_VALUE_ATTR_NAME => Self::ConstantValue {
                attribute_name_index,
                attribute_length,
                constant_value_index: u2(r)?.into(),
            },
            RUNTIME_VISIBLE_ANNOTATIONS_ATTR_NAME => {
                let num_annotations = u2(r)?;

                Self::RuntimeVisibleAnnoations {
                    attribute_name_index,
                    attribute_length,
                    annotations: Annotation::annotations(r, num_annotations.into())?,
                }
            }
            CODE_ATTR_NAME => {
                let max_stack = u2(r)?;
                let max_locals = u2(r)?;
                let code_length = u4(r)?;
                let code = vec(r, code_length as usize)?;

                let exception_table_length = u2(r)?;
                let mut exception_table = Vec::new();
                for _ in 0..exception_table_length {
                    exception_table.push(ExceptionHandler::new(r)?);
                }

                let attributes_count = u2(r)?;
                let attributes = Attribute::attributes(r, cp, attributes_count.into())?;

                Self::Code {
                    attribute_name_index,
                    attribute_length,
                    max_stack,
                    max_locals,
                    code,
                    exception_table,
                    attributes,
                }
            }
            LINE_NUMBER_TABLE_ATTR_NAME => {
                let line_number_table_length = u2(r)?;

                let mut line_number_table = Vec::new();
                for _ in 0..line_number_table_length {
                    line_number_table.push(LineNumberTableEntry {
                        start_pc: u2(r)?,
                        line_number: u2(r)?,
                    });
                }

                Self::LineNumberTable {
                    attribute_name_index,
                    attribute_length,
                    line_number_table,
                }
            }
            LOCAL_VARIABLE_TABLE_ATTR_NAME => {
                let local_variable_table_length = u2(r)?;

                let mut local_variable_table = Vec::new();
                for _ in 0..local_variable_table_length {
                    local_variable_table.push(LocalVariableTableEntry {
                        start_pc: u2(r)?,
                        length: u2(r)?,
                        name_index: u2(r)?.into(),
                        descriptor_index: u2(r)?.into(),
                        index: u2(r)?,
                    });
                }

                Self::LocalVariableTable {
                    attribute_name_index,
                    attribute_length,
                    local_variable_table,
                }
            }
            STACK_MAP_TABLE_ATTR_NAME => {
                let num_of_entries = u2(r)?;

                let mut entries = Vec::new();
                trace!("entries: {}", num_of_entries);
                for _ in 0..num_of_entries {
                    entries.push(StackMapTableEntry::new(r)?);
                }

                Self::StackMapTable {
                    attribute_name_index,
                    attribute_length,
                    entries,
                }
            }
            EXCEPTIONS_ATTR_NAME => {
                let number_of_exceptions = u2(r)?;

                let mut index_table = Vec::new();
                for _ in 0..number_of_exceptions {
                    index_table.push(u2(r)?.into());
                }

                Self::Exceptions {
                    attribute_name_index,
                    attribute_length,
                    index_table,
                }
            }
            LOCAL_VARIABLE_TYPE_TABLE_ATTR_NAME => {
                let local_variable_type_table_length = u2(r)?;

                let mut local_variable_type_table = Vec::new();
                for _ in 0..local_variable_type_table_length {
                    local_variable_type_table.push(LocalVariableTypeTableEntry::new(r)?);
                }

                Self::LocalVariableTypeTable {
                    attribute_name_index,
                    attribute_length,
                    local_variable_type_table,
                }
            }
            SIGNATURE_ATTR_NAME => Self::Signature {
                attribute_name_index,
                attribute_length,
                signature_index: u2(r)?.into(),
            },
            DEPRECATED_ATTR_NAME => Self::Deprecated {
                attribute_name_index,
                attribute_length,
            },
            SOURCE_FILE_ATTR_NAME => Self::SourceFile {
                attribute_name_index,
                attribute_length,
                source_file_index: u2(r)?.into(),
            },
            NEST_MEMBERS_ATTR_NAME => {
                let number_of_classes = u2(r)?;
                let mut classes = Vec::new();
                for _ in 0..number_of_classes {
                    classes.push(u2(r)?.into());
                }
                Self::NestMembers {
                    attribute_name_index,
                    attribute_length,
                    classes,
                }
            }
            BOOTSTRAP_METHODS_ATTR_NAME => {
                let mut methods = Vec::new();
                for _ in 0..u2(r)? {
                    methods.push(BootStrapMethod::new(r)?);
                }

                Self::BootstrapMethods {
                    attribute_name_index,
                    attribute_length,
                    methods,
                }
            }
            INNER_CLASSES_ATTR_NAME => {
                let mut inner_classes = Vec::new();
                for _ in 0..u2(r)? {
                    inner_classes.push(InnerClass::new(r)?);
                }

                Self::InnerClasses {
                    attribute_name_index,
                    attribute_length,
                    inner_classes,
                }
            }
            METHOD_PARAMETERS_ATTR_NAME => {
                let mut parameters = Vec::new();
                for _ in 0..u1(r)? {
                    parameters.push(MethodParameter::new(r)?);
                }

                Self::MethodParameters {
                    attribute_name_index,
                    attribute_length,
                    parameters,
                }
            }
            NEST_HOST_ATTR_NAME => Self::NestHost {
                attribute_name_index,
                attribute_length,
                host_class_index: u2(r)?.into(),
            },
            ENCLOSING_METHOD_ATTR_NAME => Self::EnclosingMethod {
                attribute_name_index,
                attribute_length,
                class_index: u2(r)?.into(),
                method_index: u2(r)?.into(),
            },
            PERMITTED_SUBCLASSES_ATTR_NAME => {
                let number_of_classes = u2(r)?;
                let mut classes = Vec::new();
                for _ in 0..number_of_classes {
                    classes.push(u2(r)?.into());
                }
                Self::PermittedSubclasses {
                    attribute_name_index,
                    attribute_length,
                    classes,
                }
            }
            _ => bail!("unknown attribute {}", name),
        });
        trace!("parsed bytes: {}", r.stream_position()? - before);
        attr
    }

    pub fn attributes(
        r: &mut (impl Read + Seek),
        cp: &ConstantPool,
        count: usize,
    ) -> Result<Vec<Self>> {
        let mut attributes = Vec::new();

        for _ in 0..count {
            attributes.push(Attribute::new(r, cp)?);
        }

        Ok(attributes)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub type_index: CpIndex,
    pub element_value_pairs: Vec<ElementValuePair>,
}

impl Annotation {
    fn new(r: &mut impl Read) -> Result<Self> {
        let type_index = u2(r)?.into();
        let num_element_value_pairs = u2(r)?;

        let mut element_value_pairs = Vec::new();
        for _ in 0..num_element_value_pairs {
            element_value_pairs.push(ElementValuePair::new(r)?);
        }

        Ok(Self {
            type_index,
            element_value_pairs,
        })
    }

    fn annotations(r: &mut impl Read, count: usize) -> Result<Vec<Self>> {
        let mut annotations = Vec::new();
        for _ in 0..count {
            annotations.push(Annotation::new(r)?);
        }

        Ok(annotations)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ElementValuePair {
    pub element_name_index: CpIndex,
    pub value: ElementValue,
}

impl ElementValuePair {
    fn new(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            element_name_index: u2(r)?.into(),
            value: ElementValue::new(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElementValue {
    Boolean(CpIndex),
    String(CpIndex),
}

impl ElementValue {
    fn new(r: &mut impl Read) -> Result<Self> {
        let tag: char = u1(r)?.into();

        Ok(match tag {
            'Z' => Self::Boolean(u2(r)?.into()),
            's' => Self::String(u2(r)?.into()),
            _ => bail!("invalid element value tag: {tag}"),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

impl ExceptionHandler {
    fn new(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            start_pc: u2(r)?,
            end_pc: u2(r)?,
            handler_pc: u2(r)?,
            catch_type: u2(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineNumberTableEntry {
    pub start_pc: u16,
    pub line_number: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalVariableTableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: CpIndex,
    pub descriptor_index: CpIndex,
    pub index: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VerificationType {
    Top,
    Integer,
    Float,
    Double,
    Long,
    Object(CpIndex),
    Uninitialized(u16),
}

impl VerificationType {
    fn new(r: &mut impl Read) -> Result<Self> {
        let tag = u1(r)?;

        Ok(match tag {
            0 => Self::Top,
            1 => Self::Integer,
            2 => Self::Float,
            3 => Self::Double,
            4 => Self::Long,
            7 => Self::Object(u2(r)?.into()),
            8 => Self::Uninitialized(u2(r)?),
            _ => bail!("invalid verification type tag: {tag}"),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StackMapTableEntry {
    Same,
    SameLocals1StackItem {
        verification_type: VerificationType,
    },
    SameLocals1StackItemExtended {
        offset_delta: u16,
        stack: VerificationType,
    },
    Chop {
        offset_delta: u16,
    },
    Extended {
        offset_delta: u16,
    },
    Append {
        offset_delta: u16,
        locals: Vec<VerificationType>,
    },
    Full {
        offset_delta: u16,
        locals: Vec<VerificationType>,
        stack: Vec<VerificationType>,
    },
}

impl StackMapTableEntry {
    fn new(r: &mut impl Read) -> Result<Self> {
        let tag = u1(r)?;
        trace!("stack map table entry tag: {tag}");

        Ok(match tag {
            0..=63 => Self::Same,
            64..=127 => Self::SameLocals1StackItem {
                verification_type: VerificationType::new(r)?,
            },
            247 => Self::SameLocals1StackItemExtended {
                offset_delta: u2(r)?,
                stack: VerificationType::new(r)?,
            },
            248..=250 => Self::Chop {
                offset_delta: u2(r)?,
            },
            251 => Self::Extended {
                offset_delta: u2(r)?,
            },
            252..=254 => {
                let offset_delta = u2(r)?;
                let mut locals = Vec::new();
                for _ in 0..(tag - 251) {
                    locals.push(VerificationType::new(r)?);
                }

                Self::Append {
                    offset_delta,
                    locals,
                }
            }
            255 => {
                let offset_delta = u2(r)?;

                let number_of_locals = u2(r)?;
                let mut locals = Vec::new();
                for _ in 0..number_of_locals {
                    locals.push(VerificationType::new(r)?);
                }

                let number_of_stack_items = u2(r)?;
                let mut stack = Vec::new();
                for _ in 0..number_of_stack_items {
                    stack.push(VerificationType::new(r)?);
                }

                Self::Full {
                    offset_delta,
                    locals,
                    stack,
                }
            }
            _ => bail!("invalid stack map table entry tag: {tag}"),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalVariableTypeTableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: CpIndex,
    pub signature_index: CpIndex,
    pub index: u16,
}

impl LocalVariableTypeTableEntry {
    fn new(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            start_pc: u2(r)?,
            length: u2(r)?,
            name_index: u2(r)?.into(),
            signature_index: u2(r)?.into(),
            index: u2(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BootStrapMethod {
    pub method_ref: CpIndex,
    pub arguments: Vec<CpIndex>,
}

impl BootStrapMethod {
    fn new(r: &mut impl Read) -> Result<Self> {
        let method_ref = u2(r)?.into();
        let mut arguments = Vec::new();
        for _ in 0..u2(r)? {
            arguments.push(u2(r)?.into());
        }
        Ok(Self {
            method_ref,
            arguments,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InnerClass {
    pub inner_class_info_index: CpIndex,
    pub outer_class_info_index: CpIndex,
    pub inner_name_index: CpIndex,
    pub access_flags: HashSet<InnerClassAccessFlag>,
}

impl InnerClass {
    fn new(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            inner_class_info_index: u2(r)?.into(),
            outer_class_info_index: u2(r)?.into(),
            inner_name_index: u2(r)?.into(),
            access_flags: InnerClassAccessFlag::flags(r)?,
        })
    }
}

const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0002;
const ACC_PROTECTED: u16 = 0x0004;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_INTERFACE: u16 = 0x0200;
const ACC_ABSTRACT: u16 = 0x0400;
const ACC_SYNTHETIC: u16 = 0x1000;
const ACC_ANNOTATION: u16 = 0x2000;
const ACC_ENUM: u16 = 0x4000;
const ACC_MANDATED: u16 = 0x8000;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum InnerClassAccessFlag {
    Public,
    Private,
    Protected,
    Static,
    Final,
    Interface,
    Abstract,
    Synthetic,
    Annotation,
    Enum,
}

impl InnerClassAccessFlag {
    pub fn flags(r: &mut impl Read) -> Result<HashSet<Self>> {
        let raw_flags = u2(r)?;

        let mut flags = HashSet::new();

        if raw_flags & ACC_PUBLIC > 0 {
            flags.insert(Self::Public);
        }

        if raw_flags & ACC_PRIVATE > 0 {
            flags.insert(Self::Private);
        }

        if raw_flags & ACC_PROTECTED > 0 {
            flags.insert(Self::Protected);
        }

        if raw_flags & ACC_STATIC > 0 {
            flags.insert(Self::Static);
        }

        if raw_flags & ACC_FINAL > 0 {
            flags.insert(Self::Final);
        }

        if raw_flags & ACC_INTERFACE > 0 {
            flags.insert(Self::Interface);
        }

        if raw_flags & ACC_ABSTRACT > 0 {
            flags.insert(Self::Abstract);
        }

        if raw_flags & ACC_SYNTHETIC > 0 {
            flags.insert(Self::Synthetic);
        }

        if raw_flags & ACC_ANNOTATION > 0 {
            flags.insert(Self::Annotation);
        }

        if raw_flags & ACC_ENUM > 0 {
            flags.insert(Self::Enum);
        }

        Ok(flags)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MethodParameter {
    pub name_index: CpIndex,
    pub access_flags: HashSet<MethodParameterAccessFlag>,
}

impl MethodParameter {
    pub fn new(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            name_index: u2(r)?.into(),
            access_flags: MethodParameterAccessFlag::flags(r)?,
        })
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum MethodParameterAccessFlag {
    Final,
    Synthetic,
    Mandated,
}

impl MethodParameterAccessFlag {
    pub fn flags(r: &mut impl Read) -> Result<HashSet<Self>> {
        let raw_flags = u2(r)?;

        let mut flags = HashSet::new();

        if raw_flags & ACC_FINAL > 0 {
            flags.insert(Self::Final);
        }

        if raw_flags & ACC_SYNTHETIC > 0 {
            flags.insert(Self::Synthetic);
        }

        if raw_flags & ACC_MANDATED > 0 {
            flags.insert(Self::Mandated);
        }

        Ok(flags)
    }
}
