
# get_iplayer_rs_gui

A GUI application built on top of get_iplayer_rs functionality.

## Features

- Graphical user interface for browsing and downloading BBC iPlayer content
- Interactive programme search and filtering
- Live BBC iPlayer film catalogue grouped by BBC collections
- Real-time download management
- Download history and settings editor
- Background cache refresh with progress status

## Building and Running

```bash
# Build the GUI application
cargo build --release --bin get_iplayer_rs_gui

# Run the application
cargo run --bin get_iplayer_rs_gui
```

## Configuration

The GUI shares the profile, cache, history, and options files used by `get_iplayer_rs`. Settings are written to the shared options file when **Save settings** is selected. Downloads use the configured output directory and quality, subtitle, thumbnail, tagging, and subdirectory preferences.
