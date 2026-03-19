use std::{fs::File, io::Write, path::Path, process::Command};

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let java_dir = Path::new("java");

    let sources: Vec<String> = walkdir(java_dir)
        .into_iter()
        .filter(|p| p.ends_with(".java"))
        .collect();

    let argfile_path = Path::new(&out_dir).join("sources.txt");
    let mut argfile = File::create(&argfile_path).unwrap();
    for src in &sources {
        writeln!(argfile, "{src}").unwrap();
    }

    let status = Command::new("javac")
        .arg("--patch-module")
        .arg("java.base=java")
        .arg("-d")
        .arg(&out_dir)
        .arg(format!("@{}", argfile_path.display()))
        .status()
        .unwrap();

    if !status.success() {
        panic!("java compilation failed")
    }

    println!("cargo:rerun-if-changed=java");
}

fn walkdir(dir: &Path) -> Vec<String> {
    let mut results = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(walkdir(&path));
            } else {
                results.push(path.to_string_lossy().into_owned());
            }
        }
    }

    results
}
