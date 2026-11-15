# get_iplayer_rs

A Rust application for searching and recording TV and radio programmes from BBC iPlayer and BBC Sounds. The repository contains a command-line application (CLI) and a separate graphical application (GUI); both use the same core library and profile data.

## Features

- Search BBC iPlayer TV and BBC Sounds radio listings, including programme descriptions and episode details.
- Record programmes, download subtitles and thumbnails, and tag media metadata.
- Manage download history, saved PVR searches, and programme caches.
- Use either the CLI or the GUI.
- Download Linux x86-64 (AMD64) and ARM64 release binaries for both applications.

## Requirements

- Stable Rust and Cargo for building from source.
- FFmpeg for media processing and AtomicParsley for MP4 metadata tagging.

## Build and run the CLI

```sh
cargo build --release --package get_iplayer_rs --bin get_iplayer_rs
cargo run --package get_iplayer_rs -- --help
```

The release binary is `target/release/get_iplayer_rs`.

Examples:

```sh
# Search the programme cache
get_iplayer_rs "Doctor Who"

# Show matching programme details
get_iplayer_rs --info "Doctor Who"

# Record matching programmes
get_iplayer_rs --get "Doctor Who"

# Refresh the TV and radio programme caches
get_iplayer_rs --refresh --type all
```

See `get_iplayer_rs --help` for all search, recording, PVR, cache, and output options.

## GUI

See [get_iplayer_rs_gui/README.md](get_iplayer_rs_gui/README.md) for GUI build instructions and features.

## Releases and versioning

The CLI and GUI have independent versions:

- The CLI version is in the root `Cargo.toml`. Its GitHub releases use tags in the form `v<version>`.
- The GUI version is in `get_iplayer_rs_gui/Cargo.toml`. Its GitHub releases use tags in the form `gui-v<version>`.

Each release includes Linux x86-64 and ARM64 binaries and SHA-256 checksum files. Releases are built from pushes to `main`; both release workflows can also be started manually from GitHub Actions.

## Docker

Build an image for the current architecture:

```sh
docker build -t henkez/get_iplayer_rs:latest .
```

Build and publish a multi-architecture image:

```sh
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag henkez/get_iplayer_rs:latest \
  --push .
```

Run the CLI with persistent profile data and recordings:

```sh
mkdir -p config output
docker run --rm \
  -v "$PWD/config:/data/config" \
  -v "$PWD/output:/data/output" \
  henkez/get_iplayer_rs:latest "Doctor Who"
```

The container stores its shared configuration and cache under `/data/config` and recordings under `/data/output`.

## Security checks

GitHub Actions scans the repository for verified secrets, audits the Cargo dependency lockfile, and runs Clippy. The GUI has an additional Clippy job with the Linux GUI build dependencies installed.

## Licence

The code is licensed under [GPL-3.0-or-later](LICENSE.txt).
