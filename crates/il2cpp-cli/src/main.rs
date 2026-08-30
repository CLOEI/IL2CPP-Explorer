use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use il2cpp_core::analysis::Il2CppProject;

#[derive(Debug, Parser)]
#[command(name = "il2cpp-explorer")]
#[command(about = "Inspect and analyze Unity IL2CPP builds")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Info { binary: PathBuf, metadata: PathBuf },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize logging: {error}"))?;

    let cli = Cli::parse();
    match cli.command {
        Command::Info { binary, metadata } => info(binary, metadata),
    }
}

fn info(binary: PathBuf, metadata: PathBuf) -> Result<()> {
    tracing::debug!(?binary, ?metadata, "loading IL2CPP project");
    let project = Il2CppProject::load(&binary, &metadata).with_context(|| {
        format!(
            "failed to load binary '{}' and metadata '{}'",
            binary.display(),
            metadata.display()
        )
    })?;

    println!("IL2CPP Explorer");
    println!();
    println!("Binary");
    println!("  Format: {}", project.binary_format());
    println!("  Architecture: {}", project.architecture());
    println!();
    println!("Metadata");
    println!("  Version: {}", project.metadata_version());

    Ok(())
}
