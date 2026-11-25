# Change history

Changes are grouped by application because the CLI and GUI use independent version numbers. Release entries reflect the repository's version tags; unreleased work is listed separately.

## CLI

### Unreleased

- Update `cookie` dependency to 0.18.3 to address security advisory RUSTSEC-2026-0594.

### 1.1.1

- Add live BBC iPlayer API search and programme/series resolution in `BbcApi` without requiring web scraping.
- Replace retired iBL `/search` endpoint with A-to-Z programme listing search (`/atoz/{letter}/programmes`); search now matches all programmes whose title contains the query words, handling punctuation differences such as `"bill's"` → `"bills"`.
- Support `/episodes/` plural URLs, partial URL paths, and `pid:` prefixes when parsing PIDs in `BbcApi`.
- Update `url` dependency to 2.5.9 to address security advisory RUSTSEC-2026-0592.
- Update Docker builder image to `rust:1.100-alpine`.

### 1.1.0

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

### 1.1.1

- Parallel downloads: up to N episodes download simultaneously (default 2, configurable 1–3 in Settings) using a tokio semaphore; queued items show ⏳ Queued until a slot is free.
- Add "Max parallel downloads" control (range 1–3) in the Settings tab, persisted across restarts in the shared profile options file.
- Bump `rfd` file-dialog dependency to 0.18.
- Add live BBC iPlayer API search to the Search tab with **Search iPlayer API** and automatic resolution when entering BBC iPlayer episode or series URLs and PIDs.
- Fix iPlayer API search returning HTTP 404: replaced the retired iBL `/search` endpoint with A-to-Z programme listing search so any show visible on BBC iPlayer can be found by title.
- Add quick download by iPlayer URL or PID in the Downloads tab to queue content directly via the API.
- Include future programme schedules during TV and radio cache refreshes so upcoming broadcasts are indexed in the cache.

### 1.1.0

- Load all currently available episodes for a TV show from the BBC iBL API directly from its expanded search result, then update the shared cache and search results.
- Display series and episode details for archive results and filter expired programmes from search results.
- Fix negative panel widths that could trigger an egui assertion on narrow layouts.
- Use the new `Show_Name.S01E02` default filename format for numbered recordings and their sidecars.

### 1.0.0

- Introduce the desktop GUI with programme search, BBC Films browsing, downloads, history, and settings.
- Share profile data and caches with the CLI; refresh stale TV and radio caches in the background.
- Publish Linux AMD64 and ARM64 GUI release binaries.
