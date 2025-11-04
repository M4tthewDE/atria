use std::fs::File;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow, bail};
use common::ClassIdentifier;
use zip::ZipArchive;

use crate::thread::JvmThread;
use crate::{
    jar::Jar,
    jdk::Jdk,
    loader::{BootstrapClassLoader, ReadClass},
};

pub mod class;
pub mod heap;
pub mod jar;
pub mod jdk;
pub mod loader;
pub mod thread;

pub fn run_jar(file: File) -> Result<()> {
    let archive = ZipArchive::new(file)?;
    let mut jar = Jar::new(archive);
    let main_class = jar.manifest()?.main_class;
    let sources: Vec<Box<dyn ReadClass>> = vec![Box::new(jar), Box::new(Jdk::new()?)];
    let class_loader = Arc::new(Mutex::new(BootstrapClassLoader::new(sources)));
    let main_thread = JvmThread::default("main".to_string(), class_loader);

    let main_handle = JvmThread::run_with_class(main_thread, main_class);
    main_handle
        .join()
        .map_err(|err| anyhow!("thread error: {err:?}"))??;
    bail!("TODO: After main thread exits")
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
        let res = run_jar(file);
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
