# CLI Application Specification

**Status:** Retrospective specification of the implemented CLI and shared core, as of 2026-11-15. This records current behavior; it is not a proposal for future features.

## 1. Purpose and scope

`get_iplayer_rs` is a Rust command-line application for searching programme listings and recording TV and radio programmes from BBC iPlayer and BBC Sounds. The package also provides the shared Rust library used by the GUI package.

This specification covers the CLI binary, shared search/index/cache/history functions, recording pipeline, and PVR file management. The GUI interface itself is specified in [get_iplayer_rs_gui/SPEC.md](get_iplayer_rs_gui/SPEC.md).

## 2. Users and operating context

- Users search a locally cached BBC programme index, inspect programme metadata, and optionally record matches.
- Users may supply a programme PID or episode URL directly when a listing is not in the local index.
- The application runs as a normal user process. It uses external BBC endpoints and requires network access for index refresh, metadata resolution, and recording.
- BBC stream availability and geographic restrictions are determined by BBC services. A UK IP address may be required for a programme.

## 3. Functional requirements

### 3.1 Programme index and cache

- Maintain separate TV and radio caches under the selected profile directory.
- Refresh one or both indexes from BBC schedule/listing sources. The default refresh window is seven days; `--refresh-limit` sets the window, and `--cache-rebuild` starts from an empty cache with a default thirty-day window.
- `--refresh-future` asks the indexer to include scheduled future programmes. Future items can also be included in a search with `--future` when already indexed.
- Persist programme records in pipe-delimited cache files, with a header describing columns. Each record includes its type, index, PID, programme and episode names, channel, availability and expiry times, duration, description, web and thumbnail URLs, and indexing time.
- `--show-cache-age` reports the filesystem age of the TV and radio cache files. The CLI does not refresh caches automatically based on age.

### 3.2 Search and listing

- Search TV by default; `--type tv`, `radio`, or `all` selects the index or indexes.
- With no search terms, list all records remaining after filters. Search terms are case-insensitive regular expressions matched against programme and episode names. `--long` also searches descriptions. Each supplied text pattern must match.
- Numeric terms can select cache index numbers. `pid:<PID>` selects a PID directly.
- Support include and exclude channel regular expressions, cache-added time windows (`--since`, `--before`), availability windows, expiry windows, future entries, recorded-item hiding, force override, and result limits.
- Sort results by name, episode, channel, availability, duration, or index; reverse the selected ordering with `--sortreverse`.
- Display compact match rows by default; `--info` displays full metadata. `--list channel` displays distinct channel counts. `--listformat` applies the programme template variables to output rows.

### 3.3 Direct PID and URL operations

- Accept PIDs and BBC programme URLs through `--pid` and `--url`, or detect a PID/URL supplied as the first search term.
- Retrieve programme metadata from BBC services if the PID is not in either cache.
- Resolve episode metadata and related available episode PIDs through BBC's iBL API where supported; retain playlist/page metadata fallbacks for content not represented there.
- `--pid-recursive` attempts to resolve a series or brand PID into related episode PIDs using the iBL programme episode listing.
- Direct-PID requests can display metadata (`--info`), mark the programme as downloaded, or proceed to recording.

### 3.4 Recording and media processing

- Resolve programme metadata, choose a requested version (default, audio-described, signed, or another version exposed by BBC metadata), and request stream information from the BBC media selector.
- Select a stream according to the preferred TV or radio quality list, with a general quality preference available as a fallback.
- Download HLS or DASH streams. Remux completed media with FFmpeg unless raw output is requested. TV output is MP4 by default; radio output and TV audio-only output use M4A.
- Allow a dry run with `--test`, overwrite control, separate TV/radio output paths, output directory, filename and subdirectory templates, and whitespace preservation.
- By default, name programmes with both series and episode numbers as `<name>.S<series>E<episode>` with two-digit numbers (for example, `Not_Going_Out.S01E02.mp4`). Subtitle and thumbnail sidecars use the same stem with `.srt` and `.jpg` extensions. Honor `--file-prefix` templates; retain the previous descriptive prefix for entries without both numbers.
- Download subtitles as SRT when available; support monochrome subtitles, timestamp offsets, raw TTML retention, subtitle-only downloads, and soft-subtitle embedding.
- Download thumbnails when requested or needed for tagging; support thumbnail-only operation.
- Write JSON metadata sidecars when requested. Tag media with AtomicParsley or FFmpeg unless tagging is disabled; podcast tagging is optional.
- Run a configured post-download command after successful processing.
- Record successful downloads in history and skip PIDs already recorded unless `--force` is used.

### 3.5 History

- Store download and manually marked records in the profile's `download_history` file.
- Keep PID, programme and episode names, type, date, mode, output filename, version, duration, description, channel, categories, thumbnail, guidance, web URL, and series/episode numbers where available.
- Search history by name, episode, or PID using the supplied terms; `--mark-downloaded` adds matching index records to history without recording media.

### 3.6 Saved preferences

- Read and write key/value preferences in the profile's `options` file.
- Support showing, clearing, adding, and deleting saved preferences. Saved output directory, TV/radio quality, subtitles, subtitle embedding, thumbnail, and whitespace settings are merged into CLI options when the corresponding command-line value is absent.
- `--profile-dir` and `GETIPLAYER_PROFILE` override the default profile directory; otherwise the application uses `~/.get_iplayer_rs` (or the user-profile environment path on Windows).

### 3.7 Personal Video Recorder (PVR)

- Store named PVR searches as option files in the profile's `pvr` directory. Disabled jobs use a `.disabled` suffix.
- Create, list, delete, enable, disable, run one named search, or run all enabled searches.
- PVR runs hide previously recorded results by default and record new matches. `--pvr-series` creates searches from matched programme names.

## 4. Main user flows

1. **Refresh and search:** load the selected profile; optionally refresh cache; select type and filters; search cached programme records; display results.
2. **Inspect:** resolve results to programme metadata and show availability, description, channel, duration, PID, URLs, and related fields.
3. **Record:** resolve version and streams; select quality; fetch subtitles/thumbnail when configured; download HLS/DASH; remux/tag; run post-command; append history.
4. **Automate:** create named PVR option files and run them individually or in a batch.

## 5. Architecture and boundaries

- `src/main.rs` orchestrates CLI configuration, cache operations, search, PVR work, and recording.
- `src/cli.rs` defines the Clap command-line interface.
- `src/indexer.rs`, `src/channels.rs`, `src/bbc_api.rs`, and `src/mediaselector.rs` retrieve and interpret BBC listing, iBL programme/episode, and stream metadata.
- `src/cache.rs`, `src/search.rs`, `src/history.rs`, and `src/pvr.rs` implement local indexing, filtering, history, and PVR persistence.
- `src/downloader/` provides HTTP, HLS, DASH, and the shared download pipeline. `src/subtitles.rs`, `src/postproc.rs`, and `src/tagger.rs` provide subtitle conversion, remuxing, and metadata handling.
- The CLI and GUI share the `get_iplayer_rs` library and profile files. They have separate package versions and release tags.

## 6. Data and dependencies

- Profile files: `tv.cache`, `radio.cache`, `download_history`, `options`, and files in `pvr/`.
- Core external tools: FFmpeg for remuxing and tagging; AtomicParsley for MP4 tags when available.
- HTTP requests support an optional proxy; `--no-proxy` disables proxy use.
- The CLI package version is read from the root `Cargo.toml`. CLI releases use `v<version>` tags and currently publish Linux x86-64 and ARM64 binaries with SHA-256 checksums.

## 7. Current limitations and implementation notes

- Cache refresh is explicit in the CLI; three-hour automatic freshness checks belong to the GUI only.
- The CLI parser currently declares `--pvr-scheduler`, `--subtitles-required`, `--include-format`, `--exclude-format`, `--include-supplier`, `--exclude-supplier`, `--command-tv`, and `--command-radio`, but the current runtime does not wire these flags into scheduling, filtering, or command execution. `--command` is the implemented post-download command option.
- Search and media resolution depend on the structure and availability of BBC endpoints, including the iBL API, listings, and stream metadata.
- Download recording uses BBC stream URLs and may fail for unavailable, expired, or geographically restricted programmes.

## 8. Retrospective acceptance criteria

The current CLI conforms to this specification when it can load or refresh the TV/radio cache, filter and display matches, resolve direct PIDs, record eligible HLS/DASH streams with configured post-processing, persist history/preferences/PVR jobs, and operate within the documented CLI version and profile boundaries.
