//! Export interfaces for loaded IL2CPP projects.

use il2cpp_core::analysis::Il2CppProject;
use serde_json::json;

/// Converts a loaded project into a portable representation.
pub trait Exporter {
    fn export(&self, project: &Il2CppProject) -> anyhow::Result<String>;
}

/// JSON exporter for currently available project summary information.
#[derive(Debug, Default)]
pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn export(&self, project: &Il2CppProject) -> anyhow::Result<String> {
        let summary = json!({
            "binary": {
                "format": project.binary_format().to_string(),
                "architecture": project.architecture().to_string(),
                "image_base": project.binary().image_base(),
            },
            "metadata": {
                "version": project.metadata_version().raw(),
            },
        });

        Ok(serde_json::to_string_pretty(&summary)?)
    }
}
