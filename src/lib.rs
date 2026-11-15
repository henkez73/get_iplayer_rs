//! `get_iplayer_rs` core library.
//!
//! Provides the core functionalities for querying BBC iPlayer and Sounds,
//! managing caches, search, downloads, tagging, PVR, and history.

pub mod bbc_api;
pub mod cache;
pub mod channels;
pub mod cli;
pub mod config;
pub mod downloader;
pub mod history;
pub mod indexer;
pub mod mediaselector;
pub mod models;
pub mod postproc;
pub mod pvr;
pub mod search;
pub mod subtitles;
pub mod tagger;
pub mod utils;
