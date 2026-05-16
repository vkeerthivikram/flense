# Flense — Metadata Cleaner

A production-ready cross-platform desktop application for scanning, reviewing, and safely removing privacy-sensitive metadata from files.

Built with **Tauri v2**, **Rust**, **React + TypeScript**. Targets **Windows 10/11** and **major Linux distributions**.

## Features

- **Drag-and-drop** single files, multiple files, or entire directory trees
- **Native metadata scanning** for 17+ file formats — no external tools required
- **Dry Run mode** previews what would be removed before modifying anything
- **Automatic backups** with unique naming — originals are never destroyed
- **Atomic file replacement** — files are never left in a corrupted state
- **Selective removal** — pick exactly which metadata to strip per file
- **Batch processing** — handles 1000+ files without freezing the UI
- **Undo/History** — SQLite-backed operation log with one-click restore from backup
- **Dark-mode dashboard** — modern, responsive, accessible UI

## Supported Formats

| Format | Scan | Clean | Method |
|--------|------|-------|--------|
| JPEG / TIFF | ✅ | ✅ | `kamadak-exif` native / re-encode without EXIF |
| PNG | ✅ | ✅ | Native tEXt/iTXt parser / `png` crate re-encode |
| WebP | ✅ | ✅ | Native EXIF/XMP/ICCP chunk parser / chunk removal |
| BMP | ✅ | ✅ | No metadata exists |
| PDF | ✅ | ✅ | `lopdf` Info dict + XMP parser / Info dict clear + XMP delete |
| MP3 | ✅ | ✅ | `id3` crate native / `Tag::remove_from_path` |
| FLAC | ✅ | ✅ | `metaflac` VorbisComment parser / block clear + rewrite |
| OGG / Opus | ✅ | ✅ | Native OGG page + Vorbis comment parser / exiftool fallback |
| WAV | ✅ | ✅ | Native RIFF LIST INFO parser / LIST INFO chunk removal |
| M4A / AAC | ✅ | ✅ | Native MP4 atom scanner / exiftool fallback |
| MP4 / M4V / MOV | ✅ | ✅ | Native MP4 atom scanner / exiftool fallback |
| AVI | ✅ | ✅ | Native RIFF LIST INFO parser / LIST INFO chunk removal |
| MKV / WebM | ✅ | ✅ | Native EBML Tags parser / EBML Tags element removal |
| WMV | ✅ | ✅ | exiftool / ffmpeg fallback |
| DOCX / XLSX / PPTX | ✅ | ✅ | ZIP + core.xml parser / rewrite with empty core.xml |
| ODT / ODS / ODP | ✅ | ✅ | ZIP + meta.xml parser / rewrite with empty meta.xml |

## Getting Started

### Prerequisites

- **Rust** 1.77+ — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Node.js** 18+ and **bun** — `curl -fsSL https://bun.sh/install | bash`

#### Linux System Dependencies

```bash
# Ubuntu / Debian
sudo apt install libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf

# Fedora
sudo dnf install dbus-devel gtk3-devel webkit2gtk4.1-devel \
  libayatana-appindicator-gtk3-devel librsvg2-devel patchelf

# Arch
sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg patchelf
```

### Development

```bash
cd flense
bun install
cargo tauri dev
```

### Build

```bash
cargo tauri build
```

Output installers are in `src-tauri/target/release/bundle/`:
- **Windows**: `.msi` or `.exe`
- **Linux**: `.deb`, `.rpm`, or `.AppImage`

## Architecture

### Rust Backend (`src-tauri/src/`)

| Module | Responsibility |
|--------|---------------|
| `types.rs` | Shared structs and enums — all serde-serializable |
| `errors.rs` | 17-variant `thiserror` enum with user-friendly messages |
| `metadata_scanner.rs` | File type detection, native metadata parsing for all formats |
| `cleaner.rs` | Backup creation, atomic writes, per-format cleaning logic |
| `history.rs` | SQLite-backed operation log, restore support, migrations |
| `external_tools.rs` | Bundled tool resolution + safe exiftool/ffmpeg invocation |
| `commands.rs` | Tauri v2 command handlers with progress event emission |
| `main.rs` | App entry point, state management, plugin setup |

### React Frontend (`src/`)

| File | Responsibility |
|------|---------------|
| `App.tsx` | Full dashboard — scan, review, clean, history tabs |
| `types/index.ts` | TypeScript types matching Rust JSON response shape |
| `utils/tauri.ts` | Tauri command wrappers + event listeners |
| `styles/global.css` | Dark-mode CSS variables, resets, typography |

### Tauri Configuration

| File | Purpose |
|------|---------|
| `src-tauri/tauri.conf.json` | App window, CSP, bundle resources |
| `src-tauri/capabilities/default.json` | Least-privilege filesystem + dialog access |
| `src-tauri/Cargo.toml` | Rust dependencies |

## Security & Privacy

- **Atomic file replacement** — writes to `.tmp`, verifies, then renames. Originals are never corrupted.
- **Automatic backups** — every modification creates a `.bak` with unique naming. No silent overwrites.
- **Restore from backup on failure** — if cleaning fails, the original is restored before reporting.
- **System directory protection** — blocks modification of `/etc/`, `/usr/`, `\Windows\`, `\Program Files\`.
- **No shell string concatenation** — all external tools invoked via `Command::new().args()`.
- **SHA-256 file hashing** — integrity verification before/after cleaning, stored in SQLite.
- **No metadata values in history** — only category names and counts stored by default. Full values only with explicit `audit_logging: true`.
- **Cross-platform safe paths** — all paths handled via `std::path::Path`, `PathBuf`, `canonicalize()`.
- **Least-privilege Tauri capabilities** — scoped `fs` access, no shell execution permissions.

## Bundled Tools

Flense includes optional external tools for formats that benefit from them:

- **exiftool** — broad metadata scanning/cleaning for formats without native parsers
- **ffmpeg** — video container metadata scanning/cleaning

To bundle tools, place platform-specific binaries in `src-tauri/bin/`:

```
src-tauri/bin/
├── exiftool          # Linux (Perl script with execute permission)
├── exiftool.exe      # Windows standalone
├── ffmpeg            # Linux static build
└── ffmpeg.exe        # Windows static build
```

The app checks bundled paths first, then falls back to system PATH. Detection status is shown in the sidebar.

## License

MIT
