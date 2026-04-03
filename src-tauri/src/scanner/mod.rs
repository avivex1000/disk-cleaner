pub mod apps;
pub mod caches;
pub mod docker;
pub mod downloads;
pub mod installed_apps;
pub mod logs;
pub mod node_modules;
pub mod tools;
pub mod trash;

use crate::types::Category;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub trait Scanner: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn icon(&self) -> &str;
    fn scan(&self, home: &Path) -> Option<Category>;
}

pub fn all_scanners() -> Vec<Box<dyn Scanner>> {
    vec![
        Box::new(docker::DockerScanner),
        Box::new(tools::ToolCacheScanner),
        Box::new(caches::CacheScanner),
        Box::new(node_modules::NodeModulesScanner),
        Box::new(apps::AppsScanner),
        Box::new(logs::LogsScanner),
        Box::new(downloads::DownloadsScanner),
        Box::new(trash::TrashScanner),
    ]
}

/// Get actual disk usage of a file from its metadata.
/// Uses blocks * 512 to get real on-disk size, which correctly handles
/// sparse files (e.g. OrbStack VM images that report TB in logical size
/// but only use GB on disk).
fn disk_usage(meta: &std::fs::Metadata) -> u64 {
    meta.blocks() * 512
}

/// Get the actual disk size of a directory by walking all files.
pub fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| disk_usage(&m))
        .sum()
}

/// Get sizes of immediate subdirectories, returning (name, path, size) tuples sorted by size desc.
pub fn subdirectory_sizes(path: &Path) -> Vec<(String, String, u64)> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let size = if entry_path.is_dir() {
                dir_size(&entry_path)
            } else {
                entry.metadata().map(|m| disk_usage(&m)).unwrap_or(0)
            };
            if size > 0 {
                results.push((name, entry_path.to_string_lossy().to_string(), size));
            }
        }
    }
    results.sort_by(|a, b| b.2.cmp(&a.2));
    results
}
