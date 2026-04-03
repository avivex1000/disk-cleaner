use super::{subdirectory_sizes, Scanner};
use super::installed_apps::{InstalledApps, is_system_dir};
use crate::types::{Category, DiskItem, ItemType};
use std::path::Path;

pub struct AppsScanner;

const MIN_SIZE: u64 = 50_000_000; // 50 MB

impl Scanner for AppsScanner {
    fn id(&self) -> &str { "apps" }
    fn name(&self) -> &str { "Application Data" }
    fn icon(&self) -> &str { "📱" }

    fn scan(&self, home: &Path) -> Option<Category> {
        let installed = InstalledApps::discover(home);
        let mut items = Vec::new();

        // ~/Library/Application Support
        let app_support = home.join("Library/Application Support");
        if app_support.exists() {
            for (name, path, size) in subdirectory_sizes(&app_support) {
                if size > MIN_SIZE {
                    let orphaned = !is_system_dir(&name) && !installed.is_installed(&name);
                    let desc = if orphaned {
                        format!("Orphaned app data: {name}")
                    } else {
                        format!("App data: {name}")
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

        // ~/Library/Containers
        let containers = home.join("Library/Containers");
        if containers.exists() {
            for (name, path_str, size) in subdirectory_sizes(&containers) {
                if size > MIN_SIZE {
                    let data_path = Path::new(&path_str).join("Data");
                    let target = if data_path.exists() {
                        data_path.to_string_lossy().to_string()
                    } else {
                        path_str
                    };
                    let orphaned = !is_system_dir(&name) && !installed.is_installed(&name);
                    let desc = if orphaned {
                        format!("Orphaned container: {name}")
                    } else {
                        format!("Container: {name}")
                    };
                    items.push(DiskItem {
                        path: target,
                        size_bytes: size,
                        item_type: ItemType::Directory,
                        description: desc,
                        orphaned,
                    });
                }
            }
        }

        // ~/Library/Group Containers (skip docker/orbstack — handled by docker scanner)
        let group_containers = home.join("Library/Group Containers");
        if group_containers.exists() {
            for (name, path, size) in subdirectory_sizes(&group_containers) {
                if size > MIN_SIZE {
                    let lname = name.to_lowercase();
                    if lname.contains("docker") || lname.contains("orbstack") {
                        continue;
                    }
                    let orphaned = !is_system_dir(&name) && !installed.is_installed(&name);
                    let desc = if orphaned {
                        format!("Orphaned group container: {name}")
                    } else {
                        format!("Group container: {name}")
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

        items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        items.dedup_by(|a, b| a.path.starts_with(&b.path));

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
