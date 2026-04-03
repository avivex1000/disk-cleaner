use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub total_bytes: u64,
    pub items: Vec<DiskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskItem {
    pub path: String,
    pub size_bytes: u64,
    pub item_type: ItemType,
    pub description: String,
    /// True if this looks like leftover data from an uninstalled app.
    #[serde(default)]
    pub orphaned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ItemType {
    File,
    Directory,
    PruneCommand { command: String, args: Vec<String> },
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteResult {
    pub deleted: Vec<String>,
    pub errors: Vec<DeleteError>,
    pub bytes_freed: u64,
    pub command_output: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteError {
    pub path: String,
    pub error: String,
}
