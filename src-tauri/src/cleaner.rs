use crate::types::{DeleteError, DeleteResult, DiskItem, ItemType};
use std::fs;
use std::process::Command;

pub fn delete_items(items: Vec<DiskItem>) -> DeleteResult {
    let mut deleted = Vec::new();
    let mut errors = Vec::new();
    let mut bytes_freed: u64 = 0;
    let mut command_output: Option<String> = None;

    for item in items {
        match &item.item_type {
            ItemType::File => {
                match fs::remove_file(&item.path) {
                    Ok(_) => {
                        bytes_freed += item.size_bytes;
                        deleted.push(item.path.clone());
                    }
                    Err(e) => {
                        errors.push(DeleteError {
                            path: item.path.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
            ItemType::Directory => {
                let _ = Command::new("chmod")
                    .args(["-R", "u+w", &item.path])
                    .output();

                match fs::remove_dir_all(&item.path) {
                    Ok(_) => {
                        bytes_freed += item.size_bytes;
                        deleted.push(item.path.clone());
                    }
                    Err(e) => {
                        errors.push(DeleteError {
                            path: item.path.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
            ItemType::PruneCommand { command, args } => {
                // Build the full shell command string
                let shell_cmd = format!("{} {}", command, args.join(" "));

                // Run through the user's login shell so PATH is inherited
                // (Tauri apps launched from Finder don't get the user's shell PATH)
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

                match Command::new(&shell)
                    .args(["-l", "-c", &shell_cmd])
                    .output()
                {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let combined = if !stdout.is_empty() && !stderr.is_empty() {
                            format!("{}\n{}", stdout.trim(), stderr.trim())
                        } else if !stdout.is_empty() {
                            stdout.trim().to_string()
                        } else {
                            stderr.trim().to_string()
                        };

                        if output.status.success() {
                            deleted.push(item.path.clone());
                            if !combined.is_empty() {
                                command_output = Some(combined);
                            }
                        } else {
                            errors.push(DeleteError {
                                path: item.path.clone(),
                                error: if combined.is_empty() {
                                    format!("Exit code: {}", output.status.code().unwrap_or(-1))
                                } else {
                                    combined
                                },
                            });
                        }
                    }
                    Err(e) => {
                        errors.push(DeleteError {
                            path: item.path.clone(),
                            error: format!("Failed to launch shell: {e}"),
                        });
                    }
                }
            }
        }
    }

    DeleteResult {
        deleted,
        errors,
        bytes_freed,
        command_output,
    }
}
