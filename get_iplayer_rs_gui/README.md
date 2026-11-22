# get_iplayer_rs GUI

The GUI is the graphical application for `get_iplayer_rs`. It shares the CLI's core code, profile directory, programme caches, download history, and settings.

## Features

- Search TV and radio programmes, with each result showing its available date and description when supplied by the listing.
- Query the live BBC iPlayer API directly from Search with **Search iPlayer API**, or enter an episode/series URL or PID to discover and load shows not yet in the local schedule cache.
- Queue downloads directly by iPlayer URL or PID from the **Downloads** tab without searching the cache first.
- Sort search results by availability, newest first or oldest first.
- Browse available episodes by show and season, select episodes for a batch, and queue downloads.
- For a TV show, use **Load all available episodes** in its expanded Search result to retrieve the currently available archive from BBC iBL. Results are added to the shared TV cache and grouped by series; this can reveal older series not present in the current cache.
- Browse the current BBC Films catalogue grouped into BBC collections.
- Track download progress, download up to 3 episodes concurrently with configurable parallelism (default 2) in Settings, view download history, and edit output and recording settings.
- Name numbered recordings by default as `Show_Name.S01E02.mp4`; subtitle and thumbnail files share that stem. The CLI's `--file-prefix` option can customize filenames.
- Refresh TV or radio caches manually. At startup, the app refreshes both caches if either is missing or more than three hours old; it also checks freshness every three hours while open.
- View the GUI version, licence, BBC attribution, BBC Terms of Use, and TV Licence guidance on the **About** page.

## Build and run

From the repository root:

```sh
cargo build --release --package get_iplayer_rs_gui --bin get_iplayer_rs_gui
cargo run --package get_iplayer_rs_gui --bin get_iplayer_rs_gui
```

The release binary is `target/release/get_iplayer_rs_gui`.

On Linux, building from source requires GTK 3 development files, `pkg-config`, and the X11/Wayland development libraries used by eframe. For Ubuntu, the release workflow in `.github/workflows/gui-release.yml` lists the packages it installs.

## Releases and version

The GUI version is maintained independently in this package's `Cargo.toml`. GUI releases use tags in the form `gui-v<version>` and include Linux x86-64 (AMD64) and ARM64 binaries with SHA-256 checksums. The CLI has its own version and release tags; see the [main README](../README.md#releases-and-versioning).

## Shared profile and settings

The GUI uses the same profile, caches, download history, and options as the CLI. Changes made through **Settings** are written to the shared options file when **Save settings** is selected.

## BBC services and TV Licence

BBC iPlayer and BBC Sounds are BBC services; this independent application is not affiliated with or endorsed by the BBC. Use of BBC services remains subject to the [BBC Terms of Use](https://www.bbc.co.uk/terms).

In the UK, you must be covered by a TV Licence to watch or download BBC programmes on BBC iPlayer. See [TV Licensing's BBC iPlayer guidance](https://www.tvlicensing.co.uk/check-if-you-need-one/topics/bbc-iplayer-and-the-tv-licence2) for details.

## Licence

The code is licensed under [GPL-3.0-or-later](../LICENSE.txt). BBC programme content and trademarks belong to their respective rights holders.
