use super::{dir_size, Scanner};
use crate::types::{Category, DiskItem, ItemType};
use std::path::Path;

pub struct TrashScanner;

impl Scanner for TrashScanner {
    fn id(&self) -> &str { "trash" }
    fn name(&self) -> &str { "Trash" }
    fn icon(&self) -> &str { "🗑️" }

    fn scan(&self, home: &Path) -> Option<Category> {
        let trash = home.join(".Trash");
        if !trash.exists() { return None; }

        let size = dir_size(&trash);
        if size < 1_000_000 { return None; }

        let items = vec![DiskItem {
            path: trash.to_string_lossy().to_string(),
            size_bytes: size,
            item_type: ItemType::Directory,
            description: "Empty the Trash".into(),
                    orphaned: false,
        }];

        Some(Category {
            id: self.id().into(),
            name: self.name().into(),
            icon: self.icon().into(),
            total_bytes: size,
            items,
        })
    }
}
