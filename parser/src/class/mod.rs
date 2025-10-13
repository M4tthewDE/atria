use std::{
    collections::HashSet,
    io::{Read, Seek},
};

use anyhow::{Context, Result, bail};
use tracing::trace;

use crate::{
    class::{
        access_flags::AccessFlag,
        attribute::Attribute,
        constant_pool::{ConstantPool, CpIndex, CpInfo},
        descriptor::FieldType,
        field::Field,
        method::Method,
    },
    util::{u2, u4},
};

pub mod access_flags;
pub mod attribute;
pub mod constant_pool;
pub mod descriptor;
pub mod field;
pub mod method;

/// Representation of a class, interface or module
#[derive(Clone)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
    pub access_flags: HashSet<AccessFlag>,
    pub this_class: CpIndex,
    pub super_class: CpIndex,
    pub interfaces: Vec<CpIndex>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub attributes: Vec<Attribute>,
}

impl ClassFile {
    pub fn new(r: &mut (impl Read + Seek)) -> Result<Self> {
        let magic = u4(r)?;

        if magic != 0xCAFEBABE {
            bail!("invalid magic number 0x{magic:x}");
        }

        let minor_version = u2(r)?;
        let major_version = u2(r)?;

        let constant_pool_count = u2(r)?;
        let constant_pool = ConstantPool::new(r, constant_pool_count)?;

        let access_flags = AccessFlag::flags(r)?;
        trace!("access flags: {access_flags:?}");

        let this_class = u2(r)?.into();
        let super_class = u2(r)?.into();

        let interfaces_count = u2(r)?;
        let mut interfaces = Vec::new();
        for _ in 0..interfaces_count {
            interfaces.push(u2(r)?.into());
        }

        let fields_count = u2(r)?;
        let fields = Field::fields(r, &constant_pool, fields_count.into())?;

        let methods_count = u2(r)?;
        let methods = Method::methods(r, &constant_pool, methods_count)?;

        let attributes_count = u2(r)?;
        let attributes = Attribute::attributes(r, &constant_pool, attributes_count.into())?;

        Ok(Self {
            minor_version,
            major_version,
            constant_pool,
            access_flags,
            this_class,
            super_class,
            interfaces,
            fields,
            methods,
            attributes,
        })
    }

    pub fn cp_item(&self, index: &CpIndex) -> Result<&CpInfo> {
        self.constant_pool
            .infos
            .get(index.0 as usize)
            .context(format!("no constant pool item at index {index:?}"))
    }

    pub fn method(&self, name: &str, descriptor: &str) -> Result<&Method> {
        for method in &self.methods {
            if method.name(&self.constant_pool)? != name {
                continue;
            }

            if method.raw_descriptor(&self.constant_pool)? != descriptor {
                continue;
            }

            return Ok(method);
        }

        // TODO: this silently fails, return a Result with proper error types instead
        bail!("no method with name '{name}' and descriptor '{descriptor}' found")
    }

    pub fn field(&self, name: &str, descriptor: &str) -> Result<&Field> {
        for field in &self.fields {
            if field.name(&self.constant_pool)? != name {
                continue;
            }

            if field.raw_descriptor(&self.constant_pool)? != descriptor {
                continue;
            }

            return Ok(field);
        }

        // TODO: this silently fails, return a Result with proper error types instead
        bail!("no field with name '{name}' and descriptor '{descriptor}' found")
    }

    pub fn super_class(&self) -> Result<&str> {
        self.constant_pool.class_name(&self.super_class)
    }

    pub fn is_method_signature_polymorphic(&self, method: &Method) -> Result<bool> {
        let class_name = self.constant_pool.class_name(&self.this_class)?;

        let correct_class = class_name == "java/lang/invoke/MethodHandle"
            || class_name == "java/lang/invoke/VarHandle";

        let descriptor = method.descriptor(&self.constant_pool)?;
        let object_array_paramter = descriptor.parameters
            == vec![FieldType::ComponentType(Box::new(FieldType::ObjectType {
                class_name: "java/lang/Object".to_string(),
            }))];

        Ok(correct_class && object_array_paramter && method.is_varargs() && method.is_native())
    }
}
