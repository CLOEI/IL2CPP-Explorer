# Architecture

```text
               il2cpp-core

      metadata       binary
          \           /
           \         /
            analysis
               |
        Il2CppProject
          /    |    \
         /     |     \
       CLI    Diff   Export
```

## Ownership

`il2cpp-core` owns all IL2CPP-specific knowledge. It loads metadata and executable images, exposes
normalized domain models, discovers registration structures through pluggable strategies, and
assembles those parts into `Il2CppProject`.

The CLI and future desktop GUI are consumers. They may coordinate input, rendering, and user
interaction, but parsing and analysis behavior does not belong in either application.

## Boundaries

Raw metadata layouts remain internal to the metadata reader. Public consumers use normalized
models such as assemblies, types, methods, and fields rather than table offsets or parser records.

Executable access is hidden behind `BinaryImage`. Format readers own virtual-address translation
and byte access. ELF64 is the first target; PE and Mach-O remain separate future implementations.

Metadata version dispatch lives under `metadata/versions`. Version modules will decode their own
layouts into the shared model, avoiding version conditionals throughout analysis code.

Registration discovery implements `RegistrationResolver`. Symbol, heuristic, and manual strategies
can evolve independently while returning the same registration model.

Extension crates depend on `il2cpp-core` only when they consume project data. Core never depends on
CLI, GUI, disassembly, diff, or export crates, preventing circular dependencies.
