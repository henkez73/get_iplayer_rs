# get-iplayer-rs Docker Image

[![Docker Image Size](https://img.shields.io/docker/image-size/henkez/get_iplayer_rs)](https://hub.docker.com/r/henkez/get_iplayer_rs)
[![Docker Pulls](https://img.shields.io/docker/pulls/henkez/get_iplayer_rs)](https://hub.docker.com/r/henkez/get_iplayer_rs)

A Docker image for the Rust implementation of get-iplayer, a tool to download BBC iPlayer and BBC Sounds programmes.

## What This Image Does

This Docker image provides a secure, portable way to download television programmes and radio shows from BBC iPlayer and BBC Sounds using the Rust-rewritten get-iplayer tool. 

**Key Features:**
- Download BBC iPlayer TV programmes and BBC Sounds radio programmes
- Search and record programmes with advanced filtering options  
- Support for PVR (Personal Video Recorder) functionality
- Subtitle and thumbnail download capabilities
- Metadata tagging support
- Multi-architecture support (AMD64 and ARM64)
- Non-root execution for enhanced security
- Embedded ffmpeg for media processing

## How to Run

### Basic Usage
```bash
# Search for programmes
docker run --rm henkez/get_iplayer_rs:latest "Doctor Who"

# Download a programme
docker run -v $(pwd)/cache:/data/cache -v $(pwd)/config:/data/config -v $(pwd)/output:/data/output henkez/get_iplayer_rs:latest --get "Doctor Who"

# Show help
docker run --rm henkez/get_iplayer_rs:latest --help
```

### Volume Mounts
The image expects three volume mounts:
- `/data/cache` - Cache directory for storing programme information (required)
- `/data/config` - Configuration directory for user preferences (required) 
- `/data/output` - Directory for downloaded programmes (required)

### Example with All Options
```bash
mkdir -p cache config output
docker run -v $(pwd)/cache:/data/cache -v $(pwd)/config:/data/config -v $(pwd)/output:/data/output henkez/get_iplayer_rs:latest --get --tv-quality hd "Doctor Who"
```

## Docker Image Building

### Build for Current Architecture
```bash
docker build -t henkez/get_iplayer_rs:test .
```

### Multi-Architecture Build (AMD64 and ARM64)
```bash
docker buildx build --platform linux/amd64,linux/arm64 -t henkez/get_iplayer_rs:latest --push .
```

## Configuration

The container uses default configuration files stored in `/data/config`. The first run will create necessary config files which can be customized.

## Security

This image:
- Runs as a non-root user (appuser)
- Uses minimal base Alpine Linux image
- Includes only necessary dependencies (ffmpeg, openssl)
- Does not expose unnecessary ports
- Has read-only file system when possible

## Requirements

- Docker 20.10 or later
- At least 5GB of available disk space for cache and downloads

## License

This project is licensed under the GPL-3.0-or-later License - see the LICENSE.txt file for details.