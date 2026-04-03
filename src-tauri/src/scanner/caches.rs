use super::{subdirectory_sizes, Scanner};
use super::installed_apps::{InstalledApps, is_system_dir};
use crate::types::{Category, DiskItem, ItemType};
use std::path::Path;

pub struct CacheScanner;

const MIN_SIZE: u64 = 5_000_000; // 5 MB threshold

impl Scanner for CacheScanner {
    fn id(&self) -> &str { "caches" }
    fn name(&self) -> &str { "System & App Caches" }
    fn icon(&self) -> &str { "🗂️" }

    fn scan(&self, home: &Path) -> Option<Category> {
        let installed = InstalledApps::discover(home);
        let tool_prefixes = super::tools::tool_managed_prefixes(home);
        let mut items = Vec::new();

        // ~/Library/Caches
        let lib_caches = home.join("Library/Caches");
        if lib_caches.exists() {
            for (name, path, size) in subdirectory_sizes(&lib_caches) {
                if size >= MIN_SIZE && !is_tool_managed(&path, &tool_prefixes) {
                    let orphaned = !is_system_dir(&name) && !installed.is_installed(&name);
                    let desc = if orphaned {
                        format!("Orphaned cache: {name}")
                    } else {
                        format!("App cache: {name}")
                    };
                    items.push(DiskItem {
                        path,
                        size_bytes: size,
                        item_type: ItemType::Directory,
                        description: desc,
                        orphaned,
                    });
                }
            }
        }

        // ~/.cache
        let user_cache = home.join(".cache");
        if user_cache.exists() {
            for (name, path, size) in subdirectory_sizes(&user_cache) {
                if size >= MIN_SIZE && !is_tool_managed(&path, &tool_prefixes) {
                    items.push(DiskItem {
                        path,
                        size_bytes: size,
                        item_type: ItemType::Directory,
                        description: format!("User cache: {name}"),
                        orphaned: false, // ~/.cache entries are typically CLI tools, not apps
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

fn is_tool_managed(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| path.starts_with(p.as_str()))
}
