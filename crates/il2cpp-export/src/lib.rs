//! Streaming exporters over normalized IL2CPP models.

mod dump_cs;

use std::io::Write;

use il2cpp_core::analysis::TypeResolver;
use il2cpp_core::metadata::Metadata;
use il2cpp_core::registration::NativeMethodIndex;
use serde_json::json;

pub use dump_cs::{DumpCsExporter, DumpCsOptions, DumpCsSummary};

/// Shared normalized source consumed by all exporters.
pub struct ExportContext<'a> {
    pub metadata: &'a Metadata,
    pub types: &'a TypeResolver<'a>,
    pub native_methods: Option<&'a NativeMethodIndex>,
}

/// Writes an export without requiring one large in-memory output string.
pub trait Exporter {
    type Summary;

    fn export<W: Write>(
        &self,
        context: &ExportContext<'_>,
        writer: &mut W,
    ) -> anyhow::Result<Self::Summary>;
}

#[derive(Debug, Default)]
pub struct JsonExporter;

impl Exporter for JsonExporter {
    type Summary = ();

    fn export<W: Write>(
        &self,
        context: &ExportContext<'_>,
        writer: &mut W,
    ) -> anyhow::Result<Self::Summary> {
        serde_json::to_writer_pretty(
            writer,
            &json!({
                "metadata": {
                    "version": context.metadata.version.raw(),
                    "assemblies": context.metadata.assemblies.len(),
                    "types": context.metadata.types.len(),
                    "fields": context.metadata.fields.len(),
                    "properties": context.metadata.properties.len(),
                    "methods": context.metadata.methods.len(),
                }
            }),
        )?;
        Ok(())
    }
}
