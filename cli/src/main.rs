use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// path to the jar
    #[arg(long)]
    jar: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let jar_file = File::open(args.jar)?;

    jvm::run_jar(&jar_file)
}
