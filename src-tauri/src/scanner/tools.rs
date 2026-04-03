use super::{dir_size, Scanner};
use crate::types::{Category, DiskItem, ItemType};
use std::path::Path;

pub struct ToolCacheScanner;

fn has_cmd(cmd: &str) -> bool {
    // Use login shell to find commands on the user's PATH
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    std::process::Command::new(&shell)
        .args(["-l", "-c", &format!("which {} 2>/dev/null", cmd)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn measure(home: &Path, rel: &str) -> (String, u64) {
    let p = home.join(rel);
    let s = if p.exists() { dir_size(&p) } else { 0 };
    (p.to_string_lossy().to_string(), s)
}

struct ToolDef {
    name: &'static str,
    /// CLI binary to check for (empty = skip CLI detection, rely on cache dirs)
    cli: &'static str,
    /// Cache directories relative to $HOME
    cache_dirs: &'static [&'static str],
    /// Native prune command. None = use directory deletion instead.
    prune: Option<(&'static str, &'static [&'static str], &'static str)>,
}

/// Well-known package managers and dev tools with their standard cache locations.
/// These paths are defined by each tool's documentation — they're not machine-specific,
/// they're where these tools store caches on every macOS install.
/// Only tools whose cache directories actually exist on this machine will be shown.
const TOOLS: &[ToolDef] = &[
    // -- Python --
    ToolDef {
        name: "uv (Python)",
        cli: "uv",
        cache_dirs: &[".cache/uv"],
        prune: Some(("uv", &["cache", "clean"], "Clean all uv cached packages")),
    },
    ToolDef {
        name: "pip",
        cli: "pip3",
        cache_dirs: &[".cache/pip", "Library/Caches/pip"],
        prune: Some(("pip3", &["cache", "purge"], "Purge pip download cache")),
    },
    ToolDef {
        name: "Conda",
        cli: "conda",
        cache_dirs: &[".conda/pkgs"],
        prune: Some(("conda", &["clean", "--all", "-y"], "Clean Conda package cache and tarballs")),
    },
    ToolDef {
        name: "Poetry (Python)",
        cli: "poetry",
        cache_dirs: &["Library/Caches/pypoetry", ".cache/pypoetry"],
        prune: Some(("poetry", &["cache", "clear", "--all", "."], "Clean Poetry package cache")),
    },
    // -- JavaScript / Node --
    ToolDef {
        name: "npm",
        cli: "npm",
        cache_dirs: &[".npm/_cacache"],
        prune: Some(("npm", &["cache", "clean", "--force"], "Clean npm download cache")),
    },
    ToolDef {
        name: "pnpm",
        cli: "pnpm",
        cache_dirs: &[".local/share/pnpm/store"],
        prune: Some(("pnpm", &["store", "prune"], "Remove unreferenced packages from pnpm store")),
    },
    ToolDef {
        name: "Yarn",
        cli: "yarn",
        cache_dirs: &[".yarn/cache", "Library/Caches/Yarn"],
        prune: Some(("yarn", &["cache", "clean"], "Clean Yarn package cache")),
    },
    ToolDef {
        name: "Bun",
        cli: "bun",
        cache_dirs: &[".bun/install/cache"],
        prune: None, // bun has no working cache clean command
    },
    ToolDef {
        name: "Deno",
        cli: "deno",
        cache_dirs: &["Library/Caches/deno", ".cache/deno"],
        prune: None,
    },
    // -- Rust --
    ToolDef {
        name: "Cargo (Rust)",
        cli: "cargo",
        cache_dirs: &[".cargo/registry"],
        prune: None, // cargo cache is not a built-in command
    },
    // -- Go --
    ToolDef {
        name: "Go modules",
        cli: "go",
        cache_dirs: &["go/pkg/mod/cache"],
        prune: Some(("go", &["clean", "-modcache"], "Clean Go module download cache")),
    },
    // -- JVM --
    ToolDef {
        name: "Gradle",
        cli: "",
        cache_dirs: &[".gradle/caches", ".gradle/wrapper/dists"],
        prune: None,
    },
    ToolDef {
        name: "Maven",
        cli: "",
        cache_dirs: &[".m2/repository"],
        prune: None,
    },
    // -- iOS / macOS --
    ToolDef {
        name: "CocoaPods",
        cli: "pod",
        cache_dirs: &[".cocoapods/repos", "Library/Caches/CocoaPods"],
        prune: Some(("pod", &["cache", "clean", "--all"], "Clean CocoaPods cache")),
    },
    ToolDef {
        name: "Carthage",
        cli: "",
        cache_dirs: &["Library/Caches/org.carthage.CarthageKit"],
        prune: None,
    },
    // -- System --
    ToolDef {
        name: "Homebrew",
        cli: "brew",
        cache_dirs: &["Library/Caches/Homebrew"],
        prune: Some(("brew", &["cleanup", "--prune=all", "-s"], "Remove old Homebrew downloads and versions")),
    },
    // -- Ruby --
    ToolDef {
        name: "Gem (Ruby)",
        cli: "",
        cache_dirs: &[".gem"],
        prune: None,
    },
    // -- PHP --
    ToolDef {
        name: "Composer (PHP)",
        cli: "composer",
        cache_dirs: &[".composer/cache"],
        prune: Some(("composer", &["clear-cache"], "Clean Composer package cache")),
    },
];

/// Well-known dev tool data directories (not caches, but can be large and deletable).
/// These are detected purely by existence — no CLI check needed.
const KNOWN_DEV_DIRS: &[(&str, &str)] = &[
    (".rustup/toolchains", "Rust toolchains (rustup)"),
    (".pyenv/versions", "Python versions (pyenv)"),
    (".nvm/versions", "Node versions (nvm)"),
    (".volta", "Volta (Node toolchain manager)"),
    (".sdkman", "SDKMAN (JVM toolchain manager)"),
    ("Library/Developer/Xcode/DerivedData", "Xcode DerivedData"),
    ("Library/Developer/CoreSimulator", "iOS Simulators"),
    ("Library/Android/sdk", "Android SDK"),
    (".android/avd", "Android Virtual Devices"),
    (".arduino15", "Arduino IDE data"),
    (".espressif", "ESP-IDF toolchain"),
    (".platformio", "PlatformIO"),
    (".terraform.d", "Terraform plugins"),
    (".kube/cache", "Kubernetes cache"),
    (".minikube", "Minikube VM data"),
];

const MIN_SIZE: u64 = 10_000_000; // 10 MB

impl Scanner for ToolCacheScanner {
    fn id(&self) -> &str { "tools" }
    fn name(&self) -> &str { "Package Manager Caches" }
    fn icon(&self) -> &str { "📦" }

    fn scan(&self, home: &Path) -> Option<Category> {
        let mut items = Vec::new();

        // Scan known tool cache locations — only show tools whose dirs exist
        for tool in TOOLS {
            let mut total_size: u64 = 0;
            let mut found_paths = Vec::new();

            for rel in tool.cache_dirs {
                let (path, size) = measure(home, rel);
                if size > 0 {
                    total_size += size;
                    found_paths.push(path);
                }
            }

            if total_size < MIN_SIZE {
                continue;
            }

            // Offer native prune if CLI exists, otherwise directory deletion
            let cli_available = !tool.cli.is_empty() && has_cmd(tool.cli);

            match &tool.prune {
                Some((cmd, args, desc)) if cli_available => {
                    items.push(DiskItem {
                        path: format!("{} {}", cmd, args.join(" ")),
                        size_bytes: total_size,
                        item_type: ItemType::PruneCommand {
                            command: cmd.to_string(),
                            args: args.iter().map(|a| a.to_string()).collect(),
                        },
                        description: format!("{} — {}", tool.name, desc),
                    orphaned: false,
                    });
                }
                _ => {
                    for path in found_paths {
                        items.push(DiskItem {
                            path,
                            size_bytes: total_size,
                            item_type: ItemType::Directory,
                            description: format!("{} cache", tool.name),
                    orphaned: false,
                        });
                    }
                }
            }
        }

        // Scan known dev tool data directories
        for (rel, desc) in KNOWN_DEV_DIRS {
            let full = home.join(rel);
            if full.exists() {
                let size = dir_size(&full);
                if size >= MIN_SIZE {
                    items.push(DiskItem {
                        path: full.to_string_lossy().to_string(),
                        size_bytes: size,
                        item_type: ItemType::Directory,
                        description: desc.to_string(),
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

/// Returns cache directory prefixes managed by tools, so the generic cache scanner skips them.
pub fn tool_managed_prefixes(home: &Path) -> Vec<String> {
    let mut prefixes = Vec::new();
    for tool in TOOLS {
        for rel in tool.cache_dirs {
            prefixes.push(home.join(rel).to_string_lossy().to_string());
        }
    }
    prefixes
}
