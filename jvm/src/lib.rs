use std::sync::{Arc, Mutex};
use std::{collections::HashMap, fs::File};

use anyhow::{Result, anyhow, bail};
use common::ClassIdentifier;
use zip::ZipArchive;

use crate::heap::Heap;
use crate::monitor::Monitors;
use crate::thread::JvmThread;
use crate::{
    class::{Class, FieldValue},
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
    stack::FrameValue,
};

pub mod class;
mod code;
pub mod heap;
pub mod instruction;
pub mod jar;
pub mod jdk;
pub mod loader;
mod monitor;
mod native;
pub mod stack;
pub mod thread;

pub struct Jvm {
    class_loader: Arc<Mutex<BootstrapClassLoader>>,
    classes: Arc<Mutex<HashMap<ClassIdentifier, Class>>>,
    main_class: ClassIdentifier,
    heap: Arc<Mutex<Heap>>,
    monitors: Arc<Mutex<Monitors>>,
}

impl Jvm {
    pub fn from_jar(file: File) -> Result<Self> {
        let archive = ZipArchive::new(file)?;
        let mut jar = Jar::new(archive);
        let main_class = jar.manifest()?.main_class;
        let sources: Vec<Box<dyn ReadClass>> = vec![Box::new(jar), Box::new(Jdk::new()?)];
        let class_loader = Arc::new(Mutex::new(BootstrapClassLoader::new(sources)));

        Ok(Self {
            class_loader,
            classes: Arc::new(Mutex::new(HashMap::new())),
            main_class,
            heap: Arc::new(Mutex::new(Heap::default())),
            monitors: Arc::new(Mutex::new(Monitors::default())),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let main_thread = JvmThread::new(
            "main".to_string(),
            self.class_loader.clone(),
            self.classes.clone(),
            self.heap.clone(),
            self.monitors.clone(),
        );

        let main_handle = JvmThread::run_with_class(main_thread, self.main_class.clone());
        main_handle
            .join()
            .map_err(|err| anyhow!("thread error: {err:?}"))??;
        bail!("TODO: After main thread exits")
    }
}

impl From<FrameValue> for FieldValue {
    fn from(value: FrameValue) -> Self {
        match value {
            FrameValue::Reference(reference_value) => Self::Reference(reference_value),
            FrameValue::Int(val) => Self::Integer(val),
            FrameValue::Long(val) => Self::Long(val),
            FrameValue::Float(val) => Self::Float(val),
            FrameValue::Double(val) => Self::Double(val),
            FrameValue::Reserved => panic!("impossible"),
        }
    }
}

impl From<FieldValue> for FrameValue {
    fn from(value: FieldValue) -> Self {
        match value {
            FieldValue::Reference(reference_value) => Self::Reference(reference_value),
            FieldValue::Integer(val) => Self::Int(val),
            FieldValue::Long(val) => Self::Long(val),
            FieldValue::Float(val) => Self::Float(val),
            FieldValue::Double(val) => Self::Double(val),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    use super::*;

    #[test]
    fn system() {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();

        let file = File::open("../spring-boot-demo/target/demo-0.0.1-SNAPSHOT.jar").unwrap();
        let mut jvm = Jvm::from_jar(file).unwrap();
        let res = jvm.run();
        assert_eq!(
            "Err(thread 'main' has crashed: no value at offset at
jdk.internal.misc.Unsafe.getReferenceAcquire::2148
java.util.concurrent.ConcurrentHashMap.tabAt::760
java.util.concurrent.ConcurrentHashMap.putVal::1018
java.util.concurrent.ConcurrentHashMap.put::1006
java.util.Properties.put::1301
java.lang.System.createProperties::2087
java.lang.System.initPhase1::2120
sun.security.action.GetPropertyAction.privilegedGetProperties::152
java.lang.invoke.MethodHandleStatics.<clinit>::66
java.lang.invoke.MethodHandle.<clinit>::1777
java.lang.invoke.MethodType.<clinit>::688
org.springframework.boot.loader.launch.JarModeRunner.<clinit>::33
org.springframework.boot.loader.launch.Launcher.<clinit>::42
)",
            format!("{res:?}")
        );
    }
}
