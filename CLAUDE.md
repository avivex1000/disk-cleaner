# Disk Cleaner

macOS disk cleanup utility built with Tauri 2 (Rust backend + vanilla HTML/CSS/JS frontend).

## Quick Start

```bash
pnpm install
pnpm dev          # development with hot-reload
pnpm build        # production build → src-tauri/target/release/bundle/
```

## Architecture

```
src/                          # Frontend (vanilla JS, no framework)
  index.html                  # Layout: titlebar, widgets, tree table, modals
  styles.css                  # Dark/light mode, all component styles
  app.js                      # All app logic, Tauri IPC, rendering

src-tauri/                    # Rust backend
  src/
    main.rs                   # Entry point
    lib.rs                    # Tauri commands: start_scan, rescan, delete_selected
    types.rs                  # Shared types: Category, DiskItem, ItemType, DeleteResult
    cleaner.rs                # Deletion logic (files, dirs, shell commands)
    scanner/
      mod.rs                  # Scanner trait, dir_size (uses blocks*512 for sparse files), all_scanners()
      installed_apps.rs       # Discovers installed .app bundles + bundle IDs for orphan detection
      apps.rs                 # ~/Library/Application Support, Containers, Group Containers
      caches.rs               # ~/Library/Caches, ~/.cache (excludes tool-managed paths)
      docker.rs               # Docker/OrbStack data + prune commands
      downloads.rs            # ~/Downloads large files
      logs.rs                 # ~/Library/Logs, crash reports, .hprof heap dumps
      node_modules.rs         # Finds node_modules in all home subdirs dynamically
      tools.rs                # Package manager caches with native CLI prune commands
      trash.rs                # ~/.Trash
```

## Key Design Decisions

### Scanning
- Scanners run in parallel, each in its own thread. Results stream to the frontend via Tauri events (`scan-category`, `scan-scanner-done`).
- `dir_size()` uses `metadata.blocks() * 512` (not `metadata.len()`) to handle sparse files correctly (e.g. OrbStack VM images report TB logical size but only use GB on disk).
- The `rescan` command re-runs specific scanners by ID after cleanup actions, avoiding a full rescan.

### Tool Detection
- Tools are detected by cache directory existence AND CLI availability (not hardcoded to any machine).
- CLI commands run through `$SHELL -l -c "..."` (login shell) so tools in non-standard PATH locations are found.
- `tools.rs` defines which tools have working native prune commands vs which need directory deletion. Some CLIs (e.g. `bun pm cache rm`) are no-ops — these have `prune: None`.

### Orphan Detection
- `installed_apps.rs` scans `/Applications`, `~/Applications`, `/System/Applications` and reads `Info.plist` for bundle IDs.
- Matches cache/data directories against installed apps by name, bundle ID, prefix, and substring.
- System directories (`com.apple.*`) are never flagged as orphaned.

### Frontend State
- Suggestions are stateless — they always reflect current disk size, no "done" tracking.
- Only transient state tracked: `_running` (spinner while command executes), `deletingPaths` (spinner on tree items).
- Two tree views: grouped by category (with expand/collapse) and flat sorted by size.
- Select mode is opt-in — checkboxes hidden by default, activated via "Select" button.

### Deletion
- File/directory deletion is direct (`rm -rf` with `chmod -R u+w` first for read-only caches like Go modules).
- PruneCommand items run through the user's login shell.
- After deletion, the frontend updates locally (removes items, recalculates totals) without a full rescan. Suggestion actions trigger a targeted rescan of affected scanners.

## Build & Distribution

- `pnpm build` creates a `.dmg` and `.app` in `src-tauri/target/release/bundle/dmg/`.
- Without Apple Developer signing, recipients need to right-click > Open to bypass Gatekeeper.
- The app benefits from Full Disk Access (System Settings > Privacy & Security) for scanning protected `~/Library` paths.

## Adding a New Scanner

1. Create `src-tauri/src/scanner/my_scanner.rs` implementing the `Scanner` trait.
2. Add `pub mod my_scanner;` to `scanner/mod.rs`.
3. Add `Box::new(my_scanner::MyScanner)` to the `all_scanners()` vec.
4. Set `orphaned: false` on all `DiskItem` constructors (or use `installed_apps` for detection).

## Adding a New Package Manager Tool

Add a `ToolDef` entry to the `TOOLS` array in `tools.rs`:
- Set `prune: Some(...)` only if the CLI command actually works (test it manually first).
- Set `prune: None` to fall back to directory deletion.
- The tool only appears if its `cache_dirs` exist on disk.

## Gotchas

- `cargo cache --autoclean` and `bun pm cache rm` are no-ops — don't add them as prune commands.
- macOS `~/Library/Containers` dirs are protected by `containermanagerd` — delete the `/Data` subdirectory instead.
- The titlebar uses `data-tauri-drag-region` which requires `core:window:allow-start-dragging` in capabilities.
- Some cargo registry files may be owned by root (from past `sudo cargo`). The cleaner's `chmod -R u+w` handles user-owned read-only files but can't fix root-owned ones.
