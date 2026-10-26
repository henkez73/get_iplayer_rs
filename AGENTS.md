# AGENTS.md

This file provides instructions and guidelines for AI agents working on this Rust codebase.

## Project Overview

*   **Language:** Rust (2021 edition)
*   **Architecture:** Modular, idiomatic Rust codebase focusing on safety, concurrency, and performance.
*   **Primary Toolchain:** `cargo` (latest stable toolchain recommended).

---

## Guidelines for AI Agents

### 1. Code Style & Idioms

*   **Idiomatic Rust:** Prefer standard Rust idioms and conventions (e.g., using `Result` and `Option` combinators like `.map()`, `.and_then()`, `?` operator over explicit `match` blocks when appropriate).
*   **Formatting:** All code **must** strictly adhere to `rustfmt`. Run `cargo fmt` before suggesting or committing changes.
*   **Clippy:** Ensure all code passes `cargo clippy` without warnings. Treat compiler and clippy warnings as errors.
*   **Naming Conventions:**
    *   Types, Traits, Enums, Structs: `PascalCase`
    *   Functions, Methods, Variables, Modules, Macros: `snake_case`
    *   Constants, Statics: `SCREAMING_SNAKE_CASE`

### 2. Error Handling

*   Avoid `unwrap()` or `expect()` in production code paths. Use proper error handling with `Result` and custom error types (using `thiserror` or `anyhow` as established in the crate).
*   Use `expect("clear reason why this is invariant")` only in places where an invariant is guaranteed and failure indicates a critical developer error or test logic.
*   Propagate errors using the `?` operator where applicable.

### 3. Memory & Concurrency

*   Prefer safe Rust. Avoid `unsafe` code blocks unless strictly required for hardware interoperability or extreme performance bottlenecks; any `unsafe` block must include a explicit `// SAFETY:` comment justifying why it is sound.
*   Prefer immutable bindings (`let`) by default over mutable ones (`let mut`).
*   Respect ownership and borrowing patterns. Prefer references (`&T`, `&str`, `&[T]`) over cloning unless ownership transfer or lifetime simplicity requires allocations.

### 4. Testing & Verification

Before submitting any code changes, ensure the following commands pass cleanly:

```bash
# Check syntax and type correctness
cargo check --all-targets

# Run rustfmt verification
cargo fmt --check

# Run linter
cargo clippy --all-targets -- -D warnings

# Run all unit and integration tests
cargo test
```

### 5. Documentation

*   Document all `pub` structs, enums, functions, and traits using doc comments (`///`).
*   Provide module-level documentation (`//!`) at the top of main entry points (`lib.rs` / `main.rs`).
*   Include code examples in doc comments where appropriate, ensuring they pass `cargo test --doc`.

---

## Quick Reference Commands

| Action | Command |
| :--- | :--- |
| Build debug | `cargo build` |
| Build release | `cargo build --release` |
| Run tests | `cargo test` |
| Auto-format | `cargo fmt` |
| Lint check | `cargo clippy` |