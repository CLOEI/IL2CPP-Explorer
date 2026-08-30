# Metadata Format

Unity IL2CPP builds commonly ship `global-metadata.dat`. An unprotected file begins with the
little-endian sanity value `0xFAB11BAF`, followed by a metadata version. The remainder contains
version-dependent table descriptors and records.

The current reader validates only this common eight-byte prefix and accepts versions 24, 27, and
29. Future work will document each supported layout, validate table ranges before access, and
normalize raw records into the public domain model. Version differences must remain under
`il2cpp-core/src/metadata/versions/`.
