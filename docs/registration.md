# Registration Discovery

IL2CPP native binaries contain code and metadata registration structures used to connect managed
definitions to runtime data and native method pointers. Their locations vary by platform, Unity
version, symbols, and build configuration.

`RegistrationResolver` provides one stable interface for discovery. `SymbolResolver` will use
available symbols, `HeuristicResolver` will inspect executable patterns and metadata counts, and
`ManualResolver` accepts caller-provided addresses. Automatic strategies are placeholders and do
not scan binaries yet.
