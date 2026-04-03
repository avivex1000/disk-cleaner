use std::collections::HashSet;
use std::path::Path;

/// A set of normalized app names and bundle IDs for all installed apps.
pub struct InstalledApps {
    /// Lowercased app names (e.g. "slack", "visual studio code")
    names: HashSet<String>,
    /// Lowercased bundle IDs (e.g. "com.tinyspeck.slackmacgap")
    bundle_ids: HashSet<String>,
    /// Lowercased bundle ID prefixes — first two segments (e.g. "com.google", "com.apple")
    bundle_prefixes: HashSet<String>,
}

impl InstalledApps {
    /// Discover all installed apps by scanning standard macOS app locations.
    pub fn discover(home: &Path) -> Self {
        let mut names = HashSet::new();
        let mut bundle_ids = HashSet::new();
        let mut bundle_prefixes = HashSet::new();

        // Scan all standard app locations
        let app_dirs = [
            Path::new("/Applications"),
            Path::new("/System/Applications"),
            &home.join("Applications"),
        ];

        for dir in &app_dirs {
            if dir.exists() {
                scan_app_dir(dir, &mut names, &mut bundle_ids, &mut bundle_prefixes);
            }
        }

        // Also check Homebrew cask app links
        for cask_dir in &[
            Path::new("/opt/homebrew/Caskroom"),
            Path::new("/usr/local/Caskroom"),
        ] {
            if cask_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(cask_dir) {
                    for entry in entries.flatten() {
                        names.insert(entry.file_name().to_string_lossy().to_lowercase());
                    }
                }
            }
        }

        InstalledApps { names, bundle_ids, bundle_prefixes }
    }

    /// Check if a directory name (from Library/Application Support, etc.) belongs to an installed app.
    pub fn is_installed(&self, dir_name: &str) -> bool {
        let lower = dir_name.to_lowercase();

        // Direct bundle ID match (e.g. "com.tinyspeck.slackmacgap")
        if self.bundle_ids.contains(&lower) {
            return true;
        }

        // Direct name match (e.g. "Slack" matches "slack.app")
        if self.names.contains(&lower) {
            return true;
        }

        // Bundle ID looks like a reverse-domain and we know the prefix
        // e.g. "com.google.Chrome" — check if "com.google" is a known prefix
        if lower.contains('.') {
            let parts: Vec<&str> = lower.split('.').collect();
            if parts.len() >= 2 {
                let prefix = format!("{}.{}", parts[0], parts[1]);
                if self.bundle_prefixes.contains(&prefix) {
                    return true;
                }
            }
            // Also check if the last segment matches an app name
            if let Some(last) = parts.last() {
                if self.names.contains(*last) {
                    return true;
                }
            }
        }

        // Fuzzy: check if any installed app name is contained in or contains this name
        for app_name in &self.names {
            if lower.contains(app_name.as_str()) || app_name.contains(lower.as_str()) {
                return true;
            }
        }

        false
    }
}

fn scan_app_dir(
    dir: &Path,
    names: &mut HashSet<String>,
    bundle_ids: &mut HashSet<String>,
    bundle_prefixes: &mut HashSet<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if file_name.ends_with(".app") {
            // Add the app name without .app extension
            let app_name = file_name.trim_end_matches(".app").to_lowercase();
            names.insert(app_name);

            // Try to read bundle ID from Info.plist
            if let Some(bid) = read_bundle_id(&path) {
                let lower_bid = bid.to_lowercase();
                // Store the prefix (first two segments)
                let parts: Vec<&str> = lower_bid.split('.').collect();
                if parts.len() >= 2 {
                    bundle_prefixes.insert(format!("{}.{}", parts[0], parts[1]));
                }
                bundle_ids.insert(lower_bid);
            }
        }

        // Recurse one level into subdirectories (for /Applications/Utilities, etc.)
        if path.is_dir() && !file_name.ends_with(".app") {
            scan_app_dir(&path, names, bundle_ids, bundle_prefixes);
        }
    }
}

/// Read the CFBundleIdentifier from an app's Info.plist.
fn read_bundle_id(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    if !plist_path.exists() {
        return None;
    }

    // Use /usr/libexec/PlistBuddy to read the value (always available on macOS)
    let output = std::process::Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier", &plist_path.to_string_lossy()])
        .output()
        .ok()?;

    if output.status.success() {
        let bid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !bid.is_empty() {
            return Some(bid);
        }
    }

    None
}

/// Names that are macOS system components, not real apps — should never be flagged as orphaned.
const SYSTEM_NAMES: &[&str] = &[
    "apple", "com.apple", "caches", "crashreporter", "webkit",
    "addressbook", "accounts", "knowledge", "callhistory",
    "cloudkit", "icloud", "mobilemeaccounts",
];

/// Check if a directory name looks like a macOS system component.
pub fn is_system_dir(name: &str) -> bool {
    let lower = name.to_lowercase();
    SYSTEM_NAMES.iter().any(|s| lower.contains(s))
}
