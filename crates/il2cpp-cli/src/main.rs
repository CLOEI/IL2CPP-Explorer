mod commands;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
    /// Inspect repository-root development fixtures.
    Target {
        /// Print metadata table offsets and sizes.
        #[arg(long)]
        verbose: bool,
    },
    /// Print developer-oriented details for repository-root fixtures.
    InspectTarget,
    /// Inspect one ELF64 executable.
    Binary { binary: PathBuf },
    /// Inspect one global-metadata.dat file.
    Metadata {
        metadata: PathBuf,
        /// Print metadata table offsets and sizes.
        #[arg(long)]
        verbose: bool,
    },
    /// Resolve one metadata string index.
    MetadataString {
        index: u32,
        #[arg(default_value = "./global-metadata.dat")]
        metadata: PathBuf,
    },
    /// List managed images.
    Images { metadata: PathBuf },
    /// List managed assemblies.
    Assemblies { metadata: PathBuf },
    /// Search or list managed types.
    Types {
        metadata: PathBuf,
        query: Option<String>,
        /// Print every matching type.
        #[arg(long)]
        all: bool,
    },
    /// Inspect one exact fully qualified or unique short type name.
    #[command(name = "type")]
    TypeInfo { metadata: PathBuf, name: String },
    /// Show basic executable and metadata information.
    Info { binary: PathBuf, metadata: PathBuf },
    /// Discover IL2CPP native registration structures.
    Registrations {
        binary: PathBuf,
        metadata: PathBuf,
        /// Print every validated CodeGenModule.
        #[arg(long)]
        verbose: bool,
    },
    /// Resolve matching metadata methods to native addresses.
    Method {
        binary: PathBuf,
        metadata: PathBuf,
        /// Metadata method index or a Type::Method text query.
        query: String,
    },
    /// Find methods mapped to one native virtual address.
    Address {
        binary: PathBuf,
        metadata: PathBuf,
        /// Virtual address in decimal or 0x-prefixed hexadecimal.
        address: String,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize logging: {error}"))?;

    match Cli::parse().command {
        Command::Target { verbose } => commands::target(verbose),
        Command::InspectTarget => commands::inspect_target(),
        Command::Binary { binary } => commands::binary(&binary),
        Command::Metadata { metadata, verbose } => commands::metadata(&metadata, verbose),
        Command::MetadataString { index, metadata } => commands::metadata_string(&metadata, index),
        Command::Images { metadata } => commands::images(&metadata),
        Command::Assemblies { metadata } => commands::assemblies(&metadata),
        Command::Types {
            metadata,
            query,
            all,
        } => commands::types(&metadata, query.as_deref(), all),
        Command::TypeInfo { metadata, name } => commands::type_info(&metadata, &name),
        Command::Info { binary, metadata } => commands::info(&binary, &metadata),
        Command::Registrations {
            binary,
            metadata,
            verbose,
        } => commands::registrations(&binary, &metadata, verbose),
        Command::Method {
            binary,
            metadata,
            query,
        } => commands::method(&binary, &metadata, &query),
        Command::Address {
            binary,
            metadata,
            address,
        } => commands::address(&binary, &metadata, &address),
    }
}
