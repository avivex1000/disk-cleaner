use super::{subdirectory_sizes, Scanner};
use crate::types::{Category, DiskItem, ItemType};
use std::path::Path;

pub struct LogsScanner;

impl Scanner for LogsScanner {
    fn id(&self) -> &str { "logs" }
    fn name(&self) -> &str { "Logs & Crash Reports" }
    fn icon(&self) -> &str { "📋" }

    fn scan(&self, home: &Path) -> Option<Category> {
        let mut items = Vec::new();

        // ~/Library/Logs
        let logs = home.join("Library/Logs");
        if logs.exists() {
            for (name, path, size) in subdirectory_sizes(&logs) {
                if size > 5_000_000 {
                    items.push(DiskItem {
                        path,
                        size_bytes: size,
                        item_type: ItemType::Directory,
                        description: format!("Logs: {name}"),
                    orphaned: false,
                    });
                }
            }
        }

        // ~/Library/Logs/DiagnosticReports
        let diag = home.join("Library/Logs/DiagnosticReports");
        if diag.exists() {
            let size = super::dir_size(&diag);
            if size > 5_000_000 {
                items.push(DiskItem {
                    path: diag.to_string_lossy().to_string(),
                    size_bytes: size,
                    item_type: ItemType::Directory,
                    description: "macOS diagnostic/crash reports".into(),
                    orphaned: false,
                });
            }
        }

        // Java heap dumps in home directory
        if let Ok(entries) = std::fs::read_dir(home) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".hprof") {
                    if let Ok(meta) = entry.metadata() {
                        let size = meta.len();
                        if size > 10_000_000 {
                            items.push(DiskItem {
                                path: entry.path().to_string_lossy().to_string(),
                                size_bytes: size,
                                item_type: ItemType::File,
                                description: format!("Java heap dump: {name}"),
                    orphaned: false,
                            });
                        }
                    }
                }
            }
        }

        items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

        let total: u64 = items.iter().map(|i| i.size_bytes).sum();
        if items.is_empty() { return None; }

        Some(Category {
            id: self.id().into(),
            name: self.name().into(),
            icon: self.icon().into(),
            total_bytes: total,
            items,
        })
    }
}
