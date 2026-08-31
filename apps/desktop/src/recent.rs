use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const LIMIT: usize = 10;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentProject {
    pub binary: PathBuf,
    pub metadata: PathBuf,
    pub opened_unix_seconds: u64,
}
#[derive(Default, Serialize, Deserialize)]
pub struct RecentProjects {
    pub projects: Vec<RecentProject>,
}
impl RecentProjects {
    pub fn load() -> Self {
        config_path()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
    pub fn remember(&mut self, binary: PathBuf, metadata: PathBuf) {
        self.projects
            .retain(|item| item.binary != binary || item.metadata != metadata);
        self.projects.insert(
            0,
            RecentProject {
                binary,
                metadata,
                opened_unix_seconds: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |value| value.as_secs()),
            },
        );
        self.projects.truncate(LIMIT);
        self.save();
    }
    fn save(&self) {
        if let Some(path) = config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(bytes) = serde_json::to_vec_pretty(self) {
                let _ = std::fs::write(path, bytes);
            }
        }
    }
}
fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|value| PathBuf::from(value).join(".config")))
        .map(|path| path.join("il2cpp-explorer").join("recent-projects.json"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recent_deduplicates_and_limits() {
        let mut recent = RecentProjects::default();
        for index in 0..12 {
            recent.remember(
                PathBuf::from(format!("b{index}")),
                PathBuf::from(format!("m{index}")),
            );
        }
        assert_eq!(recent.projects.len(), LIMIT);
    }
}
