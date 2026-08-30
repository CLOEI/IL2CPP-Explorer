# IL2CPP Explorer

A fast, cross-platform IL2CPP inspection and analysis toolkit written in Rust.

## Overview

IL2CPP Explorer aims to provide reusable libraries and user-facing tools for inspecting Unity
IL2CPP metadata and native executables. The workspace separates metadata modeling, executable
access, analysis, disassembly, build comparison, export, and application concerns.

## Status

**Early stage.** The repository currently provides foundational APIs, validated metadata header
loading, ELF64 inspection, and a basic information command. It does not yet parse IL2CPP type or
method tables and should not be treated as a replacement for established analysis tools.

## Goals

- Keep IL2CPP knowledge in a reusable Rust core library.
- Normalize version-specific metadata into stable public models.
- Support CLI and desktop workflows without coupling either to parsing internals.
- Map managed definitions to native addresses and instructions.
- Make build search, comparison, and export reproducible.

## Architecture

`il2cpp-core` owns metadata, binary, registration, model, and project abstractions. The CLI,
disassembler, differ, exporters, and future desktop app consume those APIs without introducing
dependencies back into the core. See [docs/architecture.md](docs/architecture.md).

## Supported Platforms

Initial support target:

- Binary: ELF64
- Architecture: ARM64
- Platform: Android
- Metadata: unprotected `global-metadata.dat`

ELF64 files can currently be identified as AArch64 or x86_64. Full platform analysis is not yet
implemented.

## Installation

Build the CLI from a local checkout:

```bash
cargo install --path crates/il2cpp-cli
```

## CLI Usage

Inspect an ELF64 binary and metadata header:

```bash
il2cpp-explorer info libil2cpp.so global-metadata.dat
```

Discover available commands:

```bash
il2cpp-explorer --help
il2cpp-explorer --version
```

## Development

Rust 1.85 or newer is required.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

## Roadmap

- [ ] Metadata header parsing
- [ ] Assemblies
- [ ] Types
- [ ] Fields
- [ ] Methods
- [ ] Code registration discovery
- [ ] Native method address mapping
- [ ] Search
- [ ] JSON export
- [ ] Desktop UI
- [ ] Build diffing
- [ ] Disassembly
- [ ] PE support
- [ ] Mach-O support

The metadata header milestone remains open until complete supported-version headers and their table
descriptors are parsed; only the common prefix is implemented today.

## Contributing

Issues and focused pull requests are welcome. Keep format-specific and metadata-version-specific
behavior isolated, add tests for observable behavior, and avoid copying source from Il2CppDumper or
other implementations.

## License

Licensed under the [MIT License](LICENSE).
