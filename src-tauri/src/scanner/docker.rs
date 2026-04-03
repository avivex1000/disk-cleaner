use super::{dir_size, subdirectory_sizes, Scanner};
use crate::types::{Category, DiskItem, ItemType};
use std::path::Path;

pub struct DockerScanner;

fn has_cmd(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn parse_docker_size(s: &str) -> u64 {
    let s = s.trim();
    if s == "0B" || s == "0" {
        return 0;
    }
    let (num_str, mult) = if let Some(n) = s.strip_suffix("GB") {
        (n, 1_000_000_000u64)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n, 1_000_000)
    } else if let Some(n) = s.strip_suffix("kB") {
        (n, 1_000)
    } else if let Some(n) = s.strip_suffix("TB") {
        (n, 1_000_000_000_000)
    } else if let Some(n) = s.strip_suffix('B') {
        (n, 1)
    } else {
        return 0;
    };
    num_str
        .trim()
        .parse::<f64>()
        .map(|v| (v * mult as f64) as u64)
        .unwrap_or(0)
}

impl Scanner for DockerScanner {
    fn id(&self) -> &str { "docker" }
    fn name(&self) -> &str { "Docker & Containers" }
    fn icon(&self) -> &str { "🐳" }

    fn scan(&self, home: &Path) -> Option<Category> {
        let mut items = Vec::new();
        let has_docker = has_cmd("docker");

        // Docker CLI config/cache
        let docker_dir = home.join(".docker");
        if docker_dir.exists() {
            let size = dir_size(&docker_dir);
            if size > 1_000_000 {
                items.push(DiskItem {
                    path: docker_dir.to_string_lossy().to_string(),
                    size_bytes: size,
                    item_type: ItemType::Directory,
                    description: "Docker CLI config and buildx cache".into(),
                    orphaned: false,
                });
            }
        }

        // Dynamically find container runtime data in Group Containers
        // (OrbStack, Docker Desktop, etc. all store data here)
        let group_containers = home.join("Library/Group Containers");
        if group_containers.exists() {
            for (name, path, size) in subdirectory_sizes(&group_containers) {
                let lname = name.to_lowercase();
                if (lname.contains("docker") || lname.contains("orbstack"))
                    && size > 1_000_000
                {
                    let label = if lname.contains("orbstack") {
                        "OrbStack"
                    } else {
                        "Docker Desktop"
                    };
                    items.push(DiskItem {
                        path,
                        size_bytes: size,
                        item_type: ItemType::Directory,
                        description: format!("{label} VM and container data"),
                    orphaned: false,
                    });
                }
            }
        }

        // Docker Desktop app support data
        let docker_desktop = home.join("Library/Containers/com.docker.docker/Data");
        if docker_desktop.exists() {
            let size = dir_size(&docker_desktop);
            if size > 10_000_000 {
                items.push(DiskItem {
                    path: docker_desktop.to_string_lossy().to_string(),
                    size_bytes: size,
                    item_type: ItemType::Directory,
                    description: "Docker Desktop container data".into(),
                    orphaned: false,
                });
            }
        }

        if has_docker {
            // Docker disk usage from CLI
            if let Some(usage) = docker_disk_usage() {
                for (kind, size) in &usage {
                    if *size > 0 {
                        items.push(DiskItem {
                            path: format!("docker {}", kind.to_lowercase()),
                            size_bytes: *size,
                            item_type: ItemType::Directory,
                            description: format!("Docker {} (reported by docker system df)", kind),
                    orphaned: false,
                        });
                    }
                }
            }

            // Prune commands
            items.push(DiskItem {
                path: "docker system prune -af --volumes".into(),
                size_bytes: 0,
                item_type: ItemType::PruneCommand {
                    command: "docker".into(),
                    args: vec!["system".into(), "prune".into(), "-af".into(), "--volumes".into()],
                },
                description: "Prune all unused images, containers, networks, and volumes".into(),
                    orphaned: false,
            });

            items.push(DiskItem {
                path: "docker builder prune -af".into(),
                size_bytes: 0,
                item_type: ItemType::PruneCommand {
                    command: "docker".into(),
                    args: vec!["builder".into(), "prune".into(), "-af".into()],
                },
                description: "Prune Docker build cache".into(),
                    orphaned: false,
            });

            items.push(DiskItem {
                path: "docker image prune -af".into(),
                size_bytes: 0,
                item_type: ItemType::PruneCommand {
                    command: "docker".into(),
                    args: vec!["image".into(), "prune".into(), "-af".into()],
                },
                description: "Remove all unused Docker images".into(),
                    orphaned: false,
            });

            items.push(DiskItem {
                path: "docker volume prune -af".into(),
                size_bytes: 0,
                item_type: ItemType::PruneCommand {
                    command: "docker".into(),
                    args: vec!["volume".into(), "prune".into(), "-af".into()],
                },
                description: "Remove all unused Docker volumes".into(),
                    orphaned: false,
            });
        }

        // OrbStack CLI prune (detected dynamically)
        if has_cmd("orb") {
            items.push(DiskItem {
                path: "orb prune".into(),
                size_bytes: 0,
                item_type: ItemType::PruneCommand {
                    command: "orb".into(),
                    args: vec!["prune".into()],
                },
                description: "Prune unused OrbStack machines and data".into(),
                    orphaned: false,
            });
        }

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

fn docker_disk_usage() -> Option<Vec<(String, u64)>> {
    let output = std::process::Command::new("docker")
        .args(["system", "df", "--format", "{{.Type}}\t{{.Size}}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let size = parse_docker_size(parts[1]);
            results.push((name, size));
        }
    }
    Some(results)
}
