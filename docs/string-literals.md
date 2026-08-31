# String literals

Identifier strings (`PlayerController`, `Move`, namespaces, assembly names) use the ordinary
metadata `strings` table. Managed literals use separate `stringLiteral` records and
`stringLiteralData`; they are parsed into `model::StringLiteral`.

For metadata v31 this project reads each 8-byte record as little-endian:

```text
u32 length
u32 data_index
```

The parser validates the record table alignment, both table ranges, and every
`data_index + length` range. Bytes are read exactly by record length—never by C-string
termination. UTF-8 is decoded lossily only after recording `valid_utf8 = false` for invalid data.

```text
metadata header
  -> stringLiteral table
  -> (length, data index)
  -> stringLiteralData
  -> decoded StringLiteral
```
