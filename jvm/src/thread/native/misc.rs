use crate::thread::FrameValue;
use anyhow::{Result, bail};

pub fn run_cds(name: &str) -> Result<Option<FrameValue>> {
    match name {
        "isDumpingClassList0" => Ok(Some(FrameValue::Int(0))),
        "isDumpingArchive0" => Ok(Some(FrameValue::Int(0))),
        "isSharingEnabled0" => Ok(Some(FrameValue::Int(0))),
        // TODO: provide a proper seed
        "getRandomSeedForDumping" => Ok(Some(FrameValue::Long(0))),
        "initializeFromArchive" => Ok(None),
        _ => bail!("TODO"),
    }
}

pub fn run_vm(name: &str) -> Result<Option<FrameValue>> {
    match name {
        "initialize" => Ok(None),
        _ => bail!("TODO"),
    }
}
