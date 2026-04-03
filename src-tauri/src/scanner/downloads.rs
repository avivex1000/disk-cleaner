use super::dir_size;
use super::Scanner;
use crate::types::{Category, DiskItem, ItemType};
use std::path::Path;

pub struct DownloadsScanner;

const MIN_SIZE: u64 = 20_000_000; // 20 MB

impl Scanner for DownloadsScanner {
    fn id(&self) -> &str { "downloads" }
    fn name(&self) -> &str { "Downloads" }
    fn icon(&self) -> &str { "⬇️" }

    fn scan(&self, home: &Path) -> Option<Category> {
        let mut items = Vec::new();

        let downloads = home.join("Downloads");
        if !downloads.exists() { return None; }

        if let Ok(entries) = std::fs::read_dir(&downloads) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let size = if path.is_dir() {
                    dir_size(&path)
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };

                if size >= MIN_SIZE {
                    let item_type = if path.is_dir() {
                        ItemType::Directory
                    } else {
                        ItemType::File
                    };

                    items.push(DiskItem {
                        path: path.to_string_lossy().to_string(),
                        size_bytes: size,
                        item_type,
                        description: format!("Download: {name}"),
                    orphaned: false,
                    });
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
