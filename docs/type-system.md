# Normalized Type System

IL2CPP Explorer resolves type information in four layers:

```text
global-metadata.dat TypeIndex
        |
Il2CppMetadataRegistration type pointer table
        |
TypeResolver
        |
TypeRef
        |
target-specific formatter (C#, Ghidra, JSON, ...)
```

## Metadata and Runtime Types

Metadata type definitions describe named classes, structs, interfaces, and enums. Field, method,
parameter, parent, interface, and generic-constraint records do not point directly to those type
definitions. They contain `TypeIndex` values into the runtime `Il2CppType` table registered by
`libil2cpp.so`.

`RuntimeMetadata` validates and reads that pointer table from `Il2CppMetadataRegistration`.
`TypeResolver` then converts each reachable runtime record into the version-independent `TypeRef`
model. Resolution is cached by type index. Pointer-based recursive records use an active-address set
and depth limit so malformed cycles fail instead of recursing indefinitely.

Metadata-only loading remains supported. Without the binary registration table, unresolved
`TypeIndex` values become explicit `TypeRef::Unknown` values; they are never silently treated as
`System.Object`.

## TypeRef

`TypeRef` represents:

- primitive CLR types, strings, objects, native integers, and typed references;
- named metadata types through stable `TypeId` values;
- single and multidimensional arrays;
- pointers and by-reference types;
- type and method generic parameters;
- closed generic instances;
- function pointers;
- unsupported or unavailable records as explicit unknown values.

Raw IL2CPP type codes remain private to `TypeResolver`. Exporters and UI code consume `TypeRef`
only.

## Generics

Generic containers own ordered generic parameters. Parameters retain names, positions, flags, and
constraint type indexes. Runtime generic instances resolve their base type and argument vector.
The C# renderer emits generic declarations, instances, and common `where` constraints.

This does not reconstruct RGCTX data or runtime generic method sharing.

## Members

The metadata parser normalizes property ownership and accessor method IDs, nested type ownership,
implemented interfaces, type flags, generic containers, and generic parameters. `TypeResolver`
provides resolved field and method signatures over those stable member IDs.

Runtime field offsets are read from `Il2CppMetadataRegistration` separately. Negative or absent
offsets remain unavailable and are not rendered as zero.

## Formatting

C# aliases and identifier sanitation belong to `il2cpp-export`, not `TypeResolver`. Other consumers
can format the same `TypeRef` graph for C, JSON, Ghidra, or a desktop UI without parsing IL2CPP
types again.
