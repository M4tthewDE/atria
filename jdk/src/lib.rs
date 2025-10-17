use std::collections::HashMap;

use anyhow::Result;
use rkyv::util::AlignedVec;

static CLASSE_CACHE: &[u8] = include_bytes!("../../target/class_cache.bin");

pub fn classes() -> Result<HashMap<String, Vec<u8>>> {
    let mut aligned: AlignedVec = AlignedVec::new();
    aligned.extend_from_slice(CLASSE_CACHE);
    let classes: HashMap<String, Vec<u8>> =
        rkyv::from_bytes::<HashMap<String, Vec<u8>>, rkyv::rancor::Error>(&aligned)?;
    Ok(classes)
}
