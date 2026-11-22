# Plan: Parallel Downloads (2–3 Slots)

**Scope:** GUI only (`get_iplayer_rs_gui`). The CLI and shared library are unchanged.

---

## Background

Currently `enqueue_download()` calls `spawn_download()` immediately for every
episode added to the queue.  Each `spawn_download()` fires a `tokio` task on
the shared `Runtime`.  Because the runtime has no concurrency cap, all queued
episodes race to start at the same time — in practice only one or two make
progress because the BBC CDN throttles burst connections from a single IP.

The fix introduces a **bounded semaphore** that limits how many downloads run
concurrently, while keeping the existing per-episode async task structure.

---

## Goals

- Run at most N episodes simultaneously (default **2**, user-configurable to 3).
- Queued episodes wait for a slot; they do not spin or poll.
- The Downloads tab shows each in-flight episode's progress bar independently
  (this already works since progress is stored per `DownloadItem` in the shared
  `Vec`).
- A "Max parallel downloads" spinner (1–3) is exposed in Settings.
- No change to the CLI, shared library, or download pipeline logic.

---

## Non-Goals

- Parallelising HLS chunk fetching inside a single episode (requires patching
  the downloader pipeline — out of scope).
- More than 3 parallel downloads (BBC CDN risk).
- Resumable downloads (separate concern).

---

## Architecture

### New shared state

```
Arc<Semaphore>   — tokio::sync::Semaphore with `max_parallel` permits
```

Added to `MyApp` alongside `download_queue`:

```rust
download_semaphore: Arc<tokio::sync::Semaphore>,
max_parallel_downloads: u8,   // 1..=3, persisted in settings
```

### How `spawn_download` changes

```
Before:
  rt.spawn(async { ... download_programme() ... })
           ↑ starts immediately, no cap

After:
  rt.spawn(async {
      let _permit = semaphore.acquire().await;   // blocks until a slot is free
      // ... existing download_programme() logic unchanged ...
      // permit is dropped at end of task → slot freed
  })
```

The `acquire()` call suspends the task (not the thread) while waiting, so
tokio's thread pool is not wasted.

---

## Detailed Changes

### 1. `get_iplayer_rs_gui/src/app.rs`

#### 1a. `MyApp` struct — add two fields

```rust
download_semaphore: Arc<tokio::sync::Semaphore>,
max_parallel_downloads: u8,
```

#### 1b. `MyApp::new()` — initialise them

```rust
let max_parallel = 2u8;
// …
download_semaphore: Arc::new(tokio::sync::Semaphore::new(max_parallel as usize)),
max_parallel_downloads: max_parallel,
```

#### 1c. `spawn_download()` — acquire permit before starting

Wrap the existing `rt.spawn` body:

```rust
fn spawn_download(&self, prog: Programme) {
    let semaphore = self.download_semaphore.clone();
    // … existing clones …

    self.rt.spawn(async move {
        // Wait for a free slot (suspends task, not thread)
        let _permit = semaphore.acquire().await
            .expect("download semaphore closed");

        // Set status to Running now that we have a slot
        { /* queue.iter_mut().find(pid).status = Running */ }

        // … existing download_programme() call and result handling …

        // _permit dropped here → slot freed for next queued task
    });
}
```

> **Note:** The status transition `Queued → Running` moves from *before*
> `spawn` to *after* `acquire()`.  Items show as **⏳ Queued** while waiting
> for a slot and **▶ Running** only once they actually start downloading.

#### 1d. `enqueue_download()` — no structural change needed

The function still calls `spawn_download()` immediately.  The semaphore inside
the spawned task is what gates actual work.

#### 1e. Settings panel — add spinner

In `ui_settings()`, add a row alongside the existing proxy/output-dir fields:

```
Max parallel downloads:  [−]  2  [+]     (range 1–3)
```

When the user changes the value:
1. Store the new value in `self.max_parallel_downloads`.
2. Replace `self.download_semaphore` with a new semaphore of the updated size.

> **Warning:** Replacing the semaphore while downloads are running is safe:
> in-flight tasks hold permits on the *old* semaphore (via `Arc`), which stays
> alive until all those tasks finish.  New tasks pick up the new semaphore.
> There is no race condition.

#### 1f. Downloads tab UI — no structural changes required

`DownloadStatus` already has `Queued` / `Running` states and the tab already
renders them differently (`⏳` vs `▶`).  Multiple `Running` rows will naturally
appear side-by-side once the semaphore allows it.

#### 1g. Persist the setting

`max_parallel_downloads` should round-trip through `Config` (or the app's
existing settings-persistence mechanism) so the preference survives restarts.

---

### 2. `get_iplayer_rs_gui/SPEC.md`

Update section **3.4 Downloads** to describe the concurrency model and the new
settings field.

---

### 3. `CHANGE.md`

Add under GUI **Unreleased**:

```
- Parallel downloads: up to N episodes download simultaneously (default 2,
  configurable 1–3 in Settings) using a tokio semaphore; queued items show
  ⏳ Queued until a slot is free.
```

---

## Implementation Order

```
Step 1  Add fields to MyApp struct and initialise in new()
Step 2  Modify spawn_download() to acquire semaphore permit
Step 3  Move Queued→Running transition to after acquire()
Step 4  Add Settings UI spinner + semaphore replacement on change
Step 5  Persist max_parallel_downloads in Config / settings file
Step 6  Update SPEC.md and CHANGE.md
Step 7  cargo fmt, cargo clippy, cargo test, cargo build --release
```

---

## Testing Plan

| Test | Method |
|---|---|
| Unit: semaphore limits concurrency to N | Mock `spawn_download` calls; assert max `Running` count ≤ N |
| Unit: slot freed on completion | Verify next `Queued` item transitions to `Running` after one finishes |
| Unit: changing N replaces semaphore without panic | Call setter while tasks hold old permits |
| Manual: queue 5 episodes, observe ≤ 2 `▶ Running` rows at once | Live run |
| Manual: change setting to 3, verify 3 run simultaneously | Live run |
| Regression: single-episode download still works | `cargo test` + live run |

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|---|---|---|
| BBC CDN throttles at N=3 | Low–Medium | Default stays at 2; user opts in to 3 |
| `History` write contention (two tasks finish simultaneously) | Low | `History::new()` is per-task; each holds its own file handle; last-write-wins is acceptable for history |
| Semaphore replacement races | None | Old `Arc<Semaphore>` outlives in-flight tasks; new one picked up by subsequent `spawn_download` calls |
