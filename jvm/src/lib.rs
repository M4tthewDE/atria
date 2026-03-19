use std::fs::File;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow, bail};
use zip::ZipArchive;

use crate::thread::JvmThread;
use crate::{
    jar::Jar,
    loader::{BootstrapClassLoader, ReadClass},
};

mod jar;
mod loader;
pub mod thread;

pub fn run_jar(file: File) -> Result<()> {
    let archive = ZipArchive::new(file)?;
    let mut jar = Jar::new(archive);
    let main_class = jar.manifest()?.main_class;
    let sources: Vec<Box<dyn ReadClass>> = vec![Box::new(jar), Box::new(Jdk {})];
    let class_loader = Arc::new(Mutex::new(BootstrapClassLoader::new(sources)));
    let main_thread = JvmThread::default("main".to_string(), class_loader);

    let main_handle = JvmThread::run_with_class(main_thread, main_class);
    main_handle
        .join()
        .map_err(|err| anyhow!("thread error: {err:?}"))??;
    Ok(())
}

struct Jdk;

impl ReadClass for Jdk {
    fn read_class(&mut self, identifier: &common::ClassIdentifier) -> Result<Vec<u8>> {
        Ok(match identifier.package.as_str() {
            "java.lang" => match identifier.name.as_str() {
                "Class" => jdk::JAVA_LANG_CLASS_BYTES.to_vec(),
                "Object" => jdk::JAVA_LANG_OBJECT_BYTES.to_vec(),
                "String" => jdk::JAVA_LANG_STRING_BYTES.to_vec(),
                "ThreadGroup" => jdk::JAVA_LANG_THREAD_GROUP_BYTES.to_vec(),
                "Thread" => jdk::JAVA_LANG_THREAD_BYTES.to_vec(),
                "System" => jdk::JAVA_LANG_SYSTEM_BYTES.to_vec(),
                _ => bail!("class not found"),
            },
            "java.io" => match identifier.name.as_str() {
                "PrintStream" => jdk::JAVA_IO_PRINT_STREAM_BYTES.to_vec(),
                _ => bail!("class not found"),
            },
            _ => bail!("class not found"),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tracing_subscriber::{
        EnvFilter,
        fmt::{self},
        layer::SubscriberExt,
        util::SubscriberInitExt,
    };

    use super::*;

    #[test]
    fn spring_boot_demo() {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .try_init()
            .ok();

        let file = File::open("../spring-boot-demo/target/demo-0.0.1-SNAPSHOT.jar").unwrap();
        let res = run_jar(file);
        assert_eq!(
            "Err(thread 'main' has crashed: method not found, maybe check interfaces?\n\nCaused by:\n    no utf8 constant pool item found at index CpIndex(0) at\norg.springframework.boot.loader.launch.JarModeRunner.<clinit>(JarModeRunner:33)\norg.springframework.boot.loader.launch.Launcher.<clinit>(Launcher:42)\n)",
            format!("{res:?}")
        );
    }

    #[test]
    fn hello_world() {
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .try_init()
            .ok();

        let file = File::open("../samples/hello_world.jar").unwrap();
        run_jar(file).unwrap();
    }
}
