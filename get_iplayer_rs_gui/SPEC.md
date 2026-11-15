# GUI Application Specification

**Status:** Retrospective specification of the implemented GUI, as of 2026-11-15. This records current behavior; it is not a proposal for future features.

## 1. Purpose and scope

`get_iplayer_rs_gui` is a desktop interface for searching BBC programme listings, browsing the current BBC Films catalogue, and recording selected programmes. It uses the `get_iplayer_rs` shared library and the same profile, cache, history, and saved options as the CLI.

## 2. Users and operating context

- Users interact through the Search, Browse, Downloads, History, Settings, and About tabs.
- The application reads local programme caches and history at startup. It contacts BBC services when refreshing caches, loading the film catalogue, resolving download metadata, and retrieving media.
- BBC stream availability and geographic restrictions are determined by BBC services. A UK IP address may be required for some programmes.

## 3. Functional requirements

### 3.1 Startup and shared profile

- Load the shared profile, TV and radio cache files, and download history before presenting the main interface.
- Use the configured profile override/environment variable or the default `~/.get_iplayer_rs` profile location.
- Display the number of cached TV and radio programmes in the toolbar.
- Preserve loaded cache data in memory if an automatic refresh fails.

### 3.2 Cache refresh

- Keep manual **Refresh TV** and **Refresh Radio** actions in the toolbar. Manual refreshes request a seven-day programme window and run outside the UI thread.
- On startup, inspect the cache files' modification times. If either cache file is missing or at least three hours old, refresh both TV and radio caches.
- While the application remains open, check cache freshness every three hours and refresh both caches when either has become stale.
- Report refresh progress/failure through the toolbar spinner and status message; update the displayed cache counts after a successful refresh.

### 3.3 Search tab

- Search the in-memory TV/radio cache using programme and episode names, with a channel filter and a **Hide recorded** option.
- Allow TV, Radio, and All type selections. The current implementation maps All to an empty type list; the shared searcher interprets an empty type list as TV-only. Consequently, All currently behaves like TV and does not combine the radio cache.
- Include scheduled future programmes in GUI searches.
- Group matched programmes by show and season. Repeated episode subtitles can form a separate spin-off group.
- Show each episode's index, title, episode number when known, available date, description, and channel. If an episode title is missing, use its description or episode number as a fallback title.
- Sort episodes by availability, newest first by default or oldest first. Order show groups by the newest matching episode in the selected direction.
- Selecting an episode presents its programme details and available related episodes. Users can queue one episode, a season, a show, or a custom batch of selected episodes.

### 3.4 Browse tab

- Load the current BBC Films catalogue on request from BBC catalogue pages.
- Group film listings into BBC collection shelves, allow collection filtering, and support individual or multi-selected film downloads.
- Show loading state and catalogue errors within the Browse tab.

### 3.5 Download queue

- Queue an individual programme, selected episodes, a season/show batch, or selected films.
- Resolve programme metadata and stream URLs, then run the shared recording pipeline.
- Show per-item queued/running/completed/failed state, progress stage, percentage, and errors. Display queue totals for each status.
- Prevent the same PID from being queued more than once while it is queued or running; a failed PID may be retried.
- Run downloads asynchronously so the UI remains responsive.
- Use the configured output directory, quality, proxy, subtitle, thumbnail, tagging, and subdirectory settings. Completed downloads update the shared history file.

### 3.6 History tab

- Read and display shared download history in newest-first display order.
- Filter visible entries by name, episode, or PID.
- Show programme name, episode, channel, date, PID, and mode.

### 3.7 Settings tab

- Edit the output directory, TV and radio quality, HTTP proxy, subtitle download and embedding, thumbnail retention, metadata tagging, and per-series subdirectory behavior.
- Mark edits as unsaved until **Save settings** is selected.
- Persist settings in the shared `options` file so the CLI can use them too.

### 3.8 About tab

- Display the GUI package version, GPL licence, project attribution, source-code link, and application description.
- Attribute BBC iPlayer and BBC Sounds to the BBC and identify this project as independent and not endorsed by the BBC.
- Link to BBC Terms of Use and TV Licensing's BBC iPlayer guidance. State that in the UK a TV Licence is required to watch or download BBC programmes on BBC iPlayer.

## 4. Main user flows

1. **Start:** load local profile/cache/history, then refresh both caches if a cache is missing or stale.
2. **Search and select:** enter search/channel filters; inspect grouped episodes, availability dates, and descriptions; select one or more entries.
3. **Queue and record:** queue a programme or batch; monitor asynchronous item status in Downloads; successful recordings are added to shared history.
4. **Browse films:** load the BBC Films catalogue, optionally filter to a collection, and queue individual or selected listings.
5. **Configure:** edit shared options and save them for both GUI and CLI use.

## 5. Architecture and boundaries

- `get_iplayer_rs_gui/src/main.rs` creates the native eframe window and application state.
- `get_iplayer_rs_gui/src/app.rs` owns tab state, query state, grouping, queue UI, cache freshness checks, async task polling, settings, and About content.
- The GUI calls the `get_iplayer_common` library alias for cache, configuration, BBC metadata, indexing, search, history, stream selection, download, and post-processing services.
- Long-running cache and catalogue requests run on background threads; downloads run on the app's Tokio runtime and report progress through shared queue state.
- The GUI package version is maintained independently in `get_iplayer_rs_gui/Cargo.toml`. GUI releases use `gui-v<version>` tags and publish Linux x86-64 and ARM64 binaries with SHA-256 checksums.

## 6. Data and platform requirements

- Shared profile files: `tv.cache`, `radio.cache`, `download_history`, and `options`.
- On Linux, the folder picker uses GTK 3. Building from source requires GTK 3 development files, `pkg-config`, and the X11/Wayland development libraries used by eframe; the release workflow installs these packages.
- The optional AccessKit screen-reader integration is disabled in the current dependency configuration.
- Programme data and media access depend on BBC endpoints and can change or be unavailable independently of the application.

## 7. Retrospective acceptance criteria

The current GUI conforms to this specification when it loads the shared profile and caches, searches and groups cached programmes, displays availability and descriptions, queues and tracks recordings, browses film collections, edits shared settings, and applies the documented automatic/manual cache refresh behavior. The GUI All-type behavior is an acknowledged current gap from the intended combined TV/radio behavior.
