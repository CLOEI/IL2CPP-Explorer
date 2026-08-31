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
    /// Disassemble one resolved IL2CPP method as AArch64.
    Disasm {
        binary: PathBuf,
        metadata: PathBuf,
        /// Fully qualified or unique Type::Method query.
        query: Option<String>,
        /// Select one exact metadata method index.
        #[arg(long, conflicts_with = "query")]
        method_id: Option<usize>,
        /// Maximum disassembly window in bytes.
        #[arg(long, default_value_t = 256)]
        bytes: usize,
        /// Omit instruction bytes from text output.
        #[arg(long)]
        no_bytes: bool,
        /// Print basic instruction counts.
        #[arg(long, conflicts_with = "json")]
        stats: bool,
        /// Emit backend-independent JSON.
        #[arg(long)]
        json: bool,
    },
    /// Disassemble a raw AArch64 executable address.
    DisasmAddress {
        binary: PathBuf,
        /// RVA in decimal or 0x-prefixed hexadecimal.
        address: Option<String>,
        /// Explicit RVA in decimal or 0x-prefixed hexadecimal.
        #[arg(long, conflicts_with_all = ["address", "va"])]
        rva: Option<String>,
        /// Explicit VA in decimal or 0x-prefixed hexadecimal.
        #[arg(long, conflicts_with_all = ["address", "rva"])]
        va: Option<String>,
        /// Maximum disassembly window in bytes.
        #[arg(long, default_value_t = 256)]
        bytes: usize,
        /// Omit instruction bytes from text output.
        #[arg(long)]
        no_bytes: bool,
        /// Print basic instruction counts.
        #[arg(long)]
        stats: bool,
    },
    /// List direct AArch64 BL calls from one resolved IL2CPP method.
    Calls {
        binary: PathBuf,
        metadata: PathBuf,
        /// Fully qualified or unique Type::Method query.
        query: Option<String>,
        /// Select one exact metadata method index.
        #[arg(long, conflicts_with = "query")]
        method_id: Option<usize>,
        /// Maximum disassembly window in bytes.
        #[arg(long, default_value_t = 256)]
        bytes: usize,
    },
    /// List managed IL2CPP string literals (not metadata identifier strings).
    Strings {
        metadata: PathBuf,
        query: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        json: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Inspect one managed IL2CPP string literal by index.
    #[command(name = "string")]
    StringLiteral { metadata: PathBuf, index: usize },
    /// Compare two IL2CPP builds using managed identities, then native bodies when available.
    Diff {
        #[arg(long)]
        old_binary: Option<PathBuf>,
        #[arg(long)]
        old_metadata: PathBuf,
        #[arg(long)]
        new_binary: Option<PathBuf>,
        #[arg(long)]
        new_metadata: PathBuf,
        #[arg(long)]
        assembly: Option<String>,
        #[arg(long, visible_alias = "type")]
        type_filter: Option<String>,
        #[arg(long, visible_alias = "method")]
        method_filter: Option<String>,
        /// Include unchanged entities. Changed-only is default.
        #[arg(long)]
        all: bool,
        /// Disable ARM64 native-body fingerprints.
        #[arg(long)]
        metadata_only: bool,
        /// Emit stable public diff DTO JSON.
        #[arg(long)]
        json: bool,
        /// Write JSON report to this path. Implies --json.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate a C#-style metadata dump.
    Dump {
        /// Binary path, or metadata path in metadata-only mode.
        input: PathBuf,
        /// Metadata path when a binary path is supplied first.
        metadata: Option<PathBuf>,
        /// Output file. Writes dump.cs to stdout when omitted.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Omit native RVA comments.
        #[arg(long)]
        no_addresses: bool,
        #[arg(long)]
        file_offsets: bool,
        /// Omit metadata token comments.
        #[arg(long)]
        no_tokens: bool,
        #[arg(long)]
        indices: bool,
        /// Omit resolved field offsets.
        #[arg(long)]
        no_field_offsets: bool,
        #[arg(long)]
        fully_qualified_types: bool,
        /// Print phase-level progress.
        #[arg(short, long)]
        verbose: bool,
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
        Command::Disasm {
            binary,
            metadata,
            query,
            method_id,
            bytes,
            no_bytes,
            stats,
            json,
        } => commands::disasm(
            &binary,
            &metadata,
            query.as_deref(),
            method_id,
            bytes,
            no_bytes,
            stats,
            json,
        ),
        Command::DisasmAddress {
            binary,
            address,
            rva,
            va,
            bytes,
            no_bytes,
            stats,
        } => commands::disasm_address(
            &binary,
            address.as_deref(),
            rva.as_deref(),
            va.as_deref(),
            bytes,
            no_bytes,
            stats,
        ),
        Command::Calls {
            binary,
            metadata,
            query,
            method_id,
            bytes,
        } => commands::calls(&binary, &metadata, query.as_deref(), method_id, bytes),
        Command::Strings {
            metadata,
            query,
            all,
            limit,
            json,
            output,
        } => commands::strings(
            &metadata,
            query.as_deref(),
            all,
            limit,
            json || output.is_some(),
            output.as_deref(),
        ),
        Command::StringLiteral { metadata, index } => commands::string_literal(&metadata, index),
        Command::Diff {
            old_binary,
            old_metadata,
            new_binary,
            new_metadata,
            assembly,
            type_filter,
            method_filter,
            all,
            metadata_only,
            json,
            output,
        } => commands::diff(
            old_binary.as_deref(),
            &old_metadata,
            new_binary.as_deref(),
            &new_metadata,
            commands::DiffFlags {
                assembly: assembly.as_deref(),
                type_filter: type_filter.as_deref(),
                method_filter: method_filter.as_deref(),
                all,
                metadata_only,
                json: json || output.is_some(),
                output: output.as_deref(),
            },
        ),
        Command::Dump {
            input,
            metadata,
            output,
            no_addresses,
            file_offsets,
            no_tokens,
            indices,
            no_field_offsets,
            fully_qualified_types,
            verbose,
        } => commands::dump(
            &input,
            metadata.as_deref(),
            output.as_deref(),
            commands::DumpFlags {
                no_addresses,
                file_offsets,
                no_tokens,
                indices,
                no_field_offsets,
                fully_qualified_types,
                verbose,
            },
        ),
    }
}
