# Product and Engineering Plan

**Status:** Prioritized backlog of proposed work. Items are ideas for discussion, not commitments or scheduled releases. Effort estimates are relative and will need review before implementation.

This plan complements the retrospective [CLI specification](SPEC.md) and [GUI specification](get_iplayer_rs_gui/SPEC.md). The specifications describe implemented behavior; this file tracks possible improvements.

## Prioritization

Priority reflects user impact, how directly an item addresses a known gap, and likely implementation scope. Effort uses **S** (small), **M** (medium), and **L** (large); these are initial estimates, not delivery promises.

## Near-term candidates

| Priority | Feature | Benefit | Initial effort | Dependencies / notes |
| --- | --- | --- | --- | --- |
| 1 | Make GUI **All** search include both TV and radio | Fixes the documented mismatch where All currently searches TV only. | S–M | Confirm shared search accepts both cache types together; add regression tests for TV, radio, and combined results. |
| 2 | Show cache freshness and refresh state | Makes automatic and manual refresh behavior understandable; users can see when each cache was last updated and whether refresh failed. | S–M | Read cache timestamps and surface per-cache status in the GUI. Consider clear behavior for missing caches and failed refreshes. |
| 3 | Improve download queue controls | Gives users more control over long or mistaken batches. Start with retry and cancellation; consider pause and reordering separately. | M–L | Cancellation may require propagating cancellation through the shared downloader and external FFmpeg/post-processing steps. Define whether cancelled items update history. |
| 4 | Improve download failure explanations | Helps users understand and recover from expired listings, unavailable streams, missing tools, and network errors. | M | Normalize errors from the shared recording pipeline and present actionable detail in the GUI; preserve useful CLI diagnostics. |

## Follow-on candidates

| Priority | Feature | Benefit | Initial effort | Dependencies / notes |
| --- | --- | --- | --- | --- |
| 5 | Expand GUI search filters | Bring GUI search closer to CLI filtering, including availability/expiry ranges and recorded-item options. | M | Reuse shared search filtering where possible; keep controls understandable for common searches. |
| 6 | Save and repeat GUI searches | Makes recurring searches easier to use. | M | Decide whether searches live in the shared profile and whether they are GUI-only or interoperable with CLI PVR searches. |
| 7 | Export search results | Lets users inspect, archive, or process results outside the app. | S–M | Choose supported formats (CSV, JSON, or both) and define which fields are exported. |
| 8 | Improve history filtering | Makes larger histories easier to navigate by date, media type, and recording outcome. | M | History currently records successful downloads and manually marked items; failure history may require a data format change. |
| 9 | Configurable automatic cache refresh | Lets users choose a refresh interval or disable automatic refresh. | S–M | Define settings defaults and shared behavior; currently the GUI uses a fixed three-hour freshness threshold and the CLI refreshes explicitly. |

## Larger or exploratory work

| Priority | Feature | Benefit | Initial effort | Dependencies / notes |
| --- | --- | --- | --- | --- |
| 10 | PVR scheduling | Runs saved searches automatically instead of requiring a manual CLI invocation. | L | Needs a scheduler lifecycle, duplicate handling, logging, and clear behavior when the app is closed. Could begin with an explicit CLI scheduler mode. |
| 11 | Accessibility improvements | Makes the GUI more usable with assistive technology and keyboard navigation. | M–L | Review the current AccessKit configuration, restore or replace screen-reader integration, and validate on supported Linux desktops. |
| 12 | Release update notifications | Helps users discover new versions. | M | Define update source, release channels, and user controls; avoid downloading or installing updates without an explicit user action. |
| 13 | Windows binaries | Expands platform support. | M–L | Requires Windows CI builds, dependency and path checks, packaging decisions, and validation on Windows. Linux remains the current release target. |

## Working conventions

- Revisit priorities before starting an item; the order is a proposal, not a commitment.
- Add or update tests for every behavior change, including regression coverage for known gaps.
- Update the relevant README and retrospective SPEC when implemented behavior changes; keep this plan focused on work that remains proposed or in progress.
