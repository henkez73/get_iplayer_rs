# Change history

Changes are grouped by application because the CLI and GUI use independent version numbers. Release entries reflect the repository's version tags; unreleased work is listed separately.

## CLI

### Unreleased (1.1.0)

- Use the BBC iBL API for available archive episode lookup and recursive series PID resolution, with pagination across the full listing.
- Default numbered programme filenames to `Show_Name.S01E02.ext`; subtitle and thumbnail sidecars use the same stem. Custom filename templates remain supported.
- Add episode metadata parsing for series/episode numbers, availability, expiry, duration, channel, artwork, and version IDs.

### 1.0.3

- Add ARM64 Linux release binaries alongside AMD64 releases.
- Align container and binary release versioning and improve release workflow handling.

### 1.0.2

- Fix Clippy findings and correct the displayed package version.
- Improve Docker multi-architecture builds and add automated security checks.
- Align Docker and binary release versions and streamline release builds.

### 1.0.1

- Add automated Linux binary releases and synchronize Docker image versioning.
- Fix release workflow version detection and publishing steps.

### 1.0.0

- Initial versioned release of the Rust CLI for searching and recording BBC TV and radio programmes.
- Include cache management, filtering, download history, saved PVR searches, subtitles, thumbnails, and media tagging.

## GUI

### Unreleased (1.1.0)

- Load all currently available episodes for a TV show from the BBC iBL API directly from its expanded search result, then update the shared cache and search results.
- Display series and episode details for archive results and filter expired programmes from search results.
- Fix negative panel widths that could trigger an egui assertion on narrow layouts.
- Use the new `Show_Name.S01E02` default filename format for numbered recordings and their sidecars.

### 1.0.0

- Introduce the desktop GUI with programme search, BBC Films browsing, downloads, history, and settings.
- Share profile data and caches with the CLI; refresh stale TV and radio caches in the background.
- Publish Linux AMD64 and ARM64 GUI release binaries.
