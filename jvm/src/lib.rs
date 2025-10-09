use std::io::{Read, Seek};

use anyhow::{Result, bail};
use tracing::debug;
use zip::ZipArchive;

pub fn run_jar(r: impl Read + Seek) -> Result<()> {
    let mut jar = ZipArchive::new(r)?;
    let manifest = Manifest::new(&mut jar)?;
    debug!("Running main class {}", manifest.main_class);

    Ok(())
}

struct Manifest {
    main_class: String,
}

impl Manifest {
    fn new(archive: &mut ZipArchive<impl Read + Seek>) -> Result<Self> {
        let mut r = archive.by_name("META-INF/MANIFEST.MF")?;
        let mut contents = String::new();
        r.read_to_string(&mut contents)?;

        for line in contents.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts[0] == "Main-Class:" {
                return Ok(Self {
                    main_class: parts[1].to_string(),
                });
            }
        }

        bail!("unable to parse MANIFEST.MF")
    }
}
