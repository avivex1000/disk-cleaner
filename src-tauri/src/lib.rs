mod cleaner;
mod scanner;
mod types;

use scanner::all_scanners;
use std::path::PathBuf;
use tauri::Emitter;
use types::{DeleteResult, DiskItem};

#[tauri::command]
async fn start_scan(app: tauri::AppHandle) -> Result<(), String> {
    let home = home_dir()?;
    let scanners = all_scanners();

    // Notify frontend how many scanners to expect
    let count = scanners.len();
    let _ = app.emit("scan-total-scanners", count);

    // Spawn each scanner in its own thread so results stream to the frontend
    for s in scanners {
        let home = home.clone();
        let app = app.clone();
        std::thread::spawn(move || {
            if let Some(category) = s.scan(&home) {
                let _ = app.emit("scan-category", &category);
            }
            let _ = app.emit("scan-scanner-done", s.id());
        });
    }

    Ok(())
}

/// Rescan specific scanner IDs and emit updated categories
#[tauri::command]
async fn rescan(app: tauri::AppHandle, scanner_ids: Vec<String>) -> Result<(), String> {
    let home = home_dir()?;
    let scanners = all_scanners();

    for s in scanners {
        if scanner_ids.contains(&s.id().to_string()) {
            let home = home.clone();
            let app = app.clone();
            std::thread::spawn(move || {
                if let Some(category) = s.scan(&home) {
                    let _ = app.emit("scan-category", &category);
                }
            });
        }
    }

    Ok(())
}

#[tauri::command]
async fn delete_selected(items: Vec<DiskItem>) -> Result<DeleteResult, String> {
    Ok(cleaner::delete_items(items))
}

fn home_dir() -> Result<PathBuf, String> {
    dirs_next::home_dir().ok_or_else(|| "Could not determine home directory".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_scan, rescan, delete_selected])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
