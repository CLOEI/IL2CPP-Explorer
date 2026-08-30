# Metadata Format

Unity IL2CPP builds commonly ship `global-metadata.dat`. An unprotected file begins with the
little-endian sanity value `0xFAB11BAF`, followed by a metadata version. The remainder contains
version-dependent table descriptors and records.

Current support targets metadata version 31. Its 256-byte header contains 31 offset/byte-count
pairs after the common prefix. Every pair is checked for integer overflow and file bounds before
record parsing. Strings are constrained to the string-data table and must be null-terminated UTF-8.

Validated version 31 record widths are 40 bytes for images, 64 for assemblies, 88 for type
definitions, 12 for fields, 36 for methods, and 12 for parameters. Version 31 adds a return-parameter
token to the method definition used by this target. Table divisibility, ownership ranges, string
indices, image/assembly links, method/type links, and parameter ranges are validated before a
normalized model is returned.

Format layout was cross-checked against public IL2CPP structure references and the target's table
boundaries. Version differences remain under `il2cpp-core/src/metadata/versions/`; other versions
are intentionally rejected rather than parsed using version 31 assumptions.
