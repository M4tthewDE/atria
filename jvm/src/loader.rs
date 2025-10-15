use std::{collections::HashMap, io::Cursor};

use crate::ClassIdentifier;
use anyhow::{Context, Result, bail};
use parser::class::{ClassFile, access_flags::AccessFlag};
use tracing::trace;

pub trait ReadClass {
    fn read_class(&mut self, identifier: &ClassIdentifier) -> Result<Vec<u8>>;
}

pub struct BootstrapClassLoader {
    sources: Vec<Box<dyn ReadClass>>,
    class_files: HashMap<ClassIdentifier, ClassFile>,
}

impl BootstrapClassLoader {
    pub fn new(sources: Vec<Box<dyn ReadClass>>) -> Self {
        Self {
            sources,
            class_files: HashMap::new(),
        }
    }

    pub fn load(&mut self, identifier: &ClassIdentifier) -> Result<ClassFile> {
        if let Some(cf) = self.class_files.get(identifier) {
            return Ok(cf.clone());
        }

        trace!("loading {identifier}");

        for source in &mut self.sources {
            let class_bytes = match source.read_class(identifier) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            let class_file = parser::parse(&mut Cursor::new(class_bytes))
                .context("should throw ClassFormatError")?;

            Self::check_version(&class_file)?;
            Self::check_name(&class_file, identifier)?;

            if class_file.super_class != 0 {
                let name = class_file
                    .constant_pool
                    .class_name(&class_file.super_class)?;
                let identifier = ClassIdentifier::new(name)?;
                self.load(&identifier)?;
            }

            self.class_files
                .insert(identifier.clone(), class_file.clone());
            trace!("loaded {identifier}");
            return Ok(class_file);
        }

        bail!("class {identifier:?} not found")
    }

    fn check_version(class_file: &ClassFile) -> Result<()> {
        if class_file.major_version != 61 && class_file.minor_version != 0 {
            bail!(
                "unsupported class file version {}.{} (TODO: throw UnsupportedClassVersionError)",
                class_file.major_version,
                class_file.minor_version
            )
        } else {
            Ok(())
        }
    }

    fn check_name(class_file: &ClassFile, identifier: &ClassIdentifier) -> Result<()> {
        let name = class_file
            .constant_pool
            .class_name(&class_file.this_class)?;
        let identifier = identifier.with_slashes()?;
        if name != identifier {
            bail!(
                "identifier does not match class file, {identifier} vs {name}, (TODO: throw NoClassDefFoundError)"
            )
        }

        if class_file.access_flags.contains(&AccessFlag::Module) {
            bail!("class file has access flag ACC_MODULE (TODO: throw NoClassDefFoundError)")
        }

        Ok(())
    }
}
