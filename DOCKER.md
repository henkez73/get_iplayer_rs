# get-iplayer-rs Docker Usage

This document provides instructions on how to use the get-iplayer-rs Docker image with practical examples.

## Building the Image

```bash
docker build -t get-iplayer-rs:latest .
```

## Running the Container

The container expects three volume mounts:
- `/data/cache`: For storing programme cache data
- `/data/config`: For storing configuration files  
- `/data/output`: For storing downloaded programmes

## Basic Usage Examples

### Update Cache
```bash
docker run -v $(pwd)/cache:/data/cache -v $(pwd)/config:/data/config -v $(pwd)/output:/data/output get-iplayer-rs:latest --refresh
```

### Search for Programmes
```bash
docker run -v $(pwd)/cache:/data/cache -v $(pwd)/config:/data/config -v $(pwd)/output:/data/output get-iplayer-rs:latest "Doctor Who"
```

### Download Programme with Specific Quality
```bash
docker run -v $(pwd)/cache:/data/cache -v $(pwd)/config:/data/config -v $(pwd)/output:/data/output get-iplayer-rs:latest --tv-quality fhd --get "Doctor Who"
```

### List Available Programmes
```bash
docker run -v $(pwd)/cache:/data/cache -v $(pwd)/config:/data/config -v $(pwd)/output:/data/output get-iplayer-rs:latest --list "Doctor Who"
```

## Cache Persistence

The cache directory will persist between container runs when mounted as a volume. The first run will populate the cache with programme data, which will be used in subsequent runs to speed up searches.

## Volume Mounts

- `-v $(pwd)/cache:/data/cache` - Cache storage directory
- `-v $(pwd)/config:/data/config` - Configuration directory  
- `-v $(pwd)/output:/data/output` - Download output directory

## Quality Options

For TV programmes, you can specify quality preferences:
- `fhd` (Full High Definition)
- `hd` (High Definition)
- `sd` (Standard Definition)
- `web` (Web Quality)
- `mobile` (Mobile Quality)

Multiple qualities can be specified in order of preference:
```bash
docker run -v $(pwd)/cache:/data/cache -v $(pwd)/config:/data/config -v $(pwd)/output:/data/output get-iplayer-rs:latest --tv-quality hd,sd,web,mobile "programme search term"
```