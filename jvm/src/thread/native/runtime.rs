use anyhow::{Result, bail};
use common::FrameValue;

pub fn run(name: &str) -> Result<Option<FrameValue>> {
    match name {
        "availableProcessors" => {
            let cpus = std::thread::available_parallelism()?;
            Ok(Some(FrameValue::Int(cpus.get().try_into()?)))
        }
        "maxMemory" => Ok(Some(FrameValue::Long(8192 * 1024 * 1024 * 1024))),
        _ => bail!("TODO"),
    }
}
