use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use zip::ZipArchive;

fn main() {
    let java_home = std::env::var("JAVA_HOME").unwrap();
    let java_home_path = PathBuf::from(java_home);
    let java_base_path = java_home_path.join("jmods/java.base.jmod");

    let mut archive = ZipArchive::new(File::open(&java_base_path).unwrap()).unwrap();
    let mut file_paths: HashSet<PathBuf> = HashSet::default();

    for name in archive.file_names() {
        let path = PathBuf::from(name);
        if path.starts_with("classes") && name.ends_with(".class") {
            file_paths.insert(path);
        }
    }

    let mut class_map: HashMap<String, Vec<u8>> = HashMap::new();
    for path in file_paths {
        let mut reader = archive.by_name(path.to_str().unwrap()).unwrap();
        let mut contents = Vec::new();
        reader.read_to_end(&mut contents).unwrap();
        let class_name: PathBuf = path.as_path().components().skip(1).collect();
        class_map.insert(class_name.to_str().unwrap().to_string(), contents);
    }

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&class_map).unwrap();
    std::fs::write("../target/class_cache.bin", bytes).unwrap();
}
