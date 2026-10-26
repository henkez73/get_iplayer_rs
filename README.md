# get_iplayer_rs Rust Implementation

A Rust rewrite of the popular BBC iPlayer and BBC Sounds downloader tool with enhanced performance and security.

## Overview

This project is a complete rewrite of the get_iplayer tool in Rust, providing:
- Faster performance due to Rust's compilation and memory safety
- Enhanced security with non-root execution and minimal attack surface
- Multi-architecture support (AMD64 and ARM64)

## Features

- Download BBC iPlayer TV and BBC Sounds radio programmes
- Search and record programmes with advanced filtering options
- Support for PVR (Personal Video Recorder) functionality
- Subtitle and thumbnail download capabilities
- Metadata tagging support
- Cache management for improved performance
- Multi-architecture support

## Building

### Prerequisites
- Rust 1.99 or later
- Cargo (comes with Rust)

### Build Steps
```bash
# Clone the repository
git clone https://github.com/your-username/get_iplayer_rs.git
cd get_iplayer_rs

# Build the project
cargo build --release

# The binary will be located at:
# target/release/get_iplayer_rs
```

## Automated Releases

Releases are automatically created when code is pushed to the main branch. These releases include:
- Semantic versioned binaries for Linux x86_64 and ARM64 architectures
- SHA256 checksums for verification
- GitHub release assets with proper version tags

## Docker Usage

### Build the image
```bash
# Build for current architecture
docker build -t henkez/get_iplayer_rs:test .

# Build for both AMD64 and ARM64 architectures
docker buildx build --platform linux/amd64,linux/arm64 -t get_iplayer_rs:latest --push .
```

### Run the container
```bash
# Basic usage
docker run --rm henkez/get_iplayer_rs:test --help

# Download a programme
docker run -v $(pwd)/cache:/app/cache -v $(pwd)/config:/app/config get_iplayer_rs "programme search term"

# With output directory
docker run -v $(pwd)/cache:/app/cache -v $(pwd)/config:/app/config -v $(pwd)/output:/app/output get_iplayer_rs --output-dir=/app/output "programme search term"
```

### Volume Mounts
- `/app/cache`: Cache directory for storing programme information
- `/app/config`: Configuration directory for user preferences

## Usage Examples

```bash
# Search for programmes
get_iplayer_rs "Doctor Who"

# Download programmes
get_iplayer_rs --get "Doctor Who"

# List available programmes
get_iplayer_rs --list "Doctor Who"

# Show help
get_iplayer_rs --help
```

## Project Structure

```
get_iplayer_rs/
├── src/
│   ├── main.rs          # Entry point and command line interface
│   ├── bbc_api.rs       # BBC API integration
│   ├── cache.rs         # Cache management
│   ├── channels.rs      # Channel information
│   ├── cli.rs           # Command line argument parsing
│   ├── config.rs        # Configuration management
│   ├── downloader/      # Media downloading functionality
│   ├── history.rs       # Download history
│   ├── indexer.rs       # Programme indexer
│   ├── mediaselector.rs # Media stream selection
│   ├── models.rs        # Data models
│   ├── postproc.rs      # Post-processing
│   ├── pVR.rs           # PVR functionality
│   ├── search.rs        # Search functionality
│   ├── subtitles.rs     # Subtitle handling
│   ├── tagger.rs        # Metadata tagging
│   └── utils.rs         # Utility functions
├── Cargo.toml           # Rust dependencies
└── README.md            # This file
```

## License

This project is licensed under the GPL-3.0-or-later License - see the LICENSE.txt file for details.