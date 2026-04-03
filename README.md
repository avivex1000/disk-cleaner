# Disk Cleaner

A lightweight macOS disk cleanup utility that helps you find and remove space-hogging files, caches, and orphaned app data.

Built with [Tauri 2](https://tauri.app) (Rust + HTML/CSS/JS) — the app binary is ~5MB with no runtime dependencies.

![screenshot](https://img.shields.io/badge/platform-macOS-blue)

## Features

- **Fast parallel scanning** — scans your disk in seconds using Rust's `walkdir` + `rayon`
- **Streaming results** — categories appear in the UI as each scanner completes
- **Smart suggestions** — native cleanup commands for package managers (uv, pip, npm, pnpm, Homebrew, Go, etc.)
- **Orphan detection** — identifies leftover data from uninstalled apps by cross-referencing installed `.app` bundles
- **Two views** — browse by category (grouped) or by size (flat list)
- **Sparse file aware** — uses actual disk blocks, not logical file size (handles Docker/OrbStack VM images correctly)
- **Local deletions** — UI updates instantly after cleanup without rescanning the entire disk
- **Dark & light mode** — follows macOS system appearance

### What it scans

| Category | What's detected |
|----------|----------------|
| Docker & Containers | Docker CLI cache, OrbStack data, prune commands |
| Package Manager Caches | uv, pip, npm, pnpm, Yarn, Bun, Cargo, Go, Gradle, Maven, CocoaPods, Homebrew, Conda, and more |
| System & App Caches | `~/Library/Caches`, `~/.cache` |
| Application Data | `~/Library/Application Support`, Containers, Group Containers |
| node_modules | Auto-discovers across all project directories |
| Logs & Crash Reports | App logs, diagnostic reports, Java heap dumps |
| Downloads | Large files in `~/Downloads` |
| Trash | `~/.Trash` contents |

## Getting Started

### Prerequisites

- macOS 12+
- [Rust](https://rustup.rs/) (stable or nightly)
- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)
- Tauri CLI: `cargo install tauri-cli`

### Development

```bash
pnpm install
pnpm dev
```

### Build

```bash
pnpm build
```

The `.app` bundle and `.dmg` installer will be in `src-tauri/target/release/bundle/`.

## Permissions

The app works without special permissions, but for a complete scan, grant **Full Disk Access** in System Settings → Privacy & Security. Without it, some `~/Library` directories may be inaccessible.

## Project Structure

```
src/                    # Frontend (vanilla HTML/CSS/JS)
src-tauri/
  src/
    lib.rs              # Tauri commands (scan, rescan, delete)
    cleaner.rs          # Deletion logic
    scanner/            # Pluggable scanner modules
      mod.rs            # Scanner trait + dir_size utilities
      installed_apps.rs # App bundle discovery for orphan detection
      apps.rs           # Application data scanner
      caches.rs         # System/app cache scanner
      docker.rs         # Docker/OrbStack scanner
      tools.rs          # Package manager cache scanner
      ...
```

See [CLAUDE.md](CLAUDE.md) for detailed architecture docs, design decisions, and how to add new scanners.

## License

MIT
