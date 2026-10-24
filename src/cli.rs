use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "get_iplayer",
    version = "3.36.0",
    about = "BBC iPlayer/BBC Sounds Indexing Tool and PVR (Rust rewrite)",
    long_about = "Lists, searches and records TV and radio programmes from BBC iPlayer and BBC Sounds."
)]
pub struct Cli {
    /// Search terms (regex or substring or programme index numbers)
    #[arg(value_name = "SEARCH_TERMS")]
    pub search_terms: Vec<String>,

    // ==================== Action Flags ====================
    /// Start recording matching programmes
    #[arg(short = 'g', long = "get", alias = "record")]
    pub get: bool,

    /// Record arbitrary PIDs that do not necessarily appear in the index
    #[arg(long = "pid", value_delimiter = ',')]
    pub pids: Vec<String>,

    /// Record the PIDs contained in specified episode URLs (alias for --pid)
    #[arg(long = "url", value_delimiter = ',')]
    pub urls: Vec<String>,

    /// Record all related episodes if value of --pid is a series or brand PID
    #[arg(long = "pid-recursive")]
    pub pid_recursive: bool,

    /// Show full programme metadata and stream availability
    #[arg(short = 'i', long = "info")]
    pub info: bool,

    /// Search recordings history
    #[arg(long = "history")]
    pub history: bool,

    /// Mark matching programmes as downloaded in history
    #[arg(long = "mark-downloaded")]
    pub mark_downloaded: bool,

    /// Refresh programme index cache
    #[arg(short = 'f', long = "refresh", alias = "flush")]
    pub refresh: bool,

    /// Rebuild cache with full 30-day programme index
    #[arg(long = "cache-rebuild", alias = "rebuild-cache")]
    pub cache_rebuild: bool,

    /// Obtain future programme schedule when refreshing cache
    #[arg(long = "refresh-future")]
    pub refresh_future: bool,

    /// Minimum number of days of programmes to cache (1..30, default 7)
    #[arg(long = "refresh-limit")]
    pub refresh_limit: Option<i64>,

    // ==================== PVR Options ====================
    /// Run all saved PVR searches
    #[arg(long = "pvr")]
    pub pvr: bool,

    /// Run single named PVR search
    #[arg(long = "pvr-single", value_name = "NAME")]
    pub pvr_single: Option<String>,

    /// Add new PVR search
    #[arg(long = "pvr-add", value_name = "NAME")]
    pub pvr_add: Option<String>,

    /// Delete named PVR search
    #[arg(long = "pvr-del", value_name = "NAME")]
    pub pvr_del: Option<String>,

    /// List all saved PVR searches
    #[arg(long = "pvr-list")]
    pub pvr_list: bool,

    /// Enable named PVR search
    #[arg(long = "pvr-enable", value_name = "NAME")]
    pub pvr_enable: Option<String>,

    /// Disable named PVR search
    #[arg(long = "pvr-disable", value_name = "NAME")]
    pub pvr_disable: Option<String>,

    /// Automatically create PVR search for matched programme series
    #[arg(long = "pvr-series")]
    pub pvr_series: bool,

    /// Run PVR scheduler daemon loop with given interval in seconds (>= 1800)
    #[arg(long = "pvr-scheduler", value_name = "SECS")]
    pub pvr_scheduler: Option<u64>,

    // ==================== Search & Filtering ====================
    /// Programme types to search: tv, radio, all (default: tv)
    #[arg(long = "type", default_value = "tv")]
    pub prog_type: String,

    /// Search additionally in programme descriptions and episode names
    #[arg(short = 'l', long = "long")]
    pub long_search: bool,

    /// Narrow search to matched channel(s) (regex or comma-separated list)
    #[arg(long = "channel")]
    pub channel: Option<String>,

    /// Exclude matched channel(s)
    #[arg(long = "exclude-channel", alias = "xchan")]
    pub exclude_channel: Option<String>,

    /// Limit search to programmes added to cache in last <hours>
    #[arg(long = "since")]
    pub since: Option<i64>,

    /// Limit search to programmes added to cache before <hours> ago
    #[arg(long = "before")]
    pub before: Option<i64>,

    /// Limit search to programmes that became available in last <hours>
    #[arg(long = "available-since")]
    pub available_since: Option<i64>,

    /// Limit search to programmes that became available before <hours> ago
    #[arg(long = "available-before")]
    pub available_before: Option<i64>,

    /// Limit search to programmes that will expire after <hours> from now
    #[arg(long = "expires-after")]
    pub expires_after: Option<i64>,

    /// Limit search to programmes that will expire before <hours> from now
    #[arg(long = "expires-before")]
    pub expires_before: Option<i64>,

    /// Search future programme schedule if indexed
    #[arg(long = "future")]
    pub future: bool,

    /// Limit number of matching results
    #[arg(long = "limit-matches")]
    pub limit_matches: Option<usize>,

    /// Field to use to sort displayed matches
    #[arg(long = "sort")]
    pub sort: Option<String>,

    /// Reverse order of sorted matches
    #[arg(long = "sortreverse")]
    pub sort_reverse: bool,

    /// Show a list of distinct element values (e.g. channel) and exit
    #[arg(long = "list")]
    pub list_elements: Option<String>,

    /// Custom display format with substitution parameters
    #[arg(long = "listformat")]
    pub list_format: Option<String>,

    /// Display cache age and exit
    #[arg(long = "show-cache-age")]
    pub show_cache_age: bool,

    // ==================== Recording & Stream Options ====================
    /// General recording quality preference
    #[arg(long = "quality")]
    pub quality: Option<String>,

    /// TV recording quality preference: fhd,hd,sd,web,mobile (default: hd,sd,web,mobile)
    #[arg(long = "tv-quality", alias = "tvquality")]
    pub tv_quality: Option<String>,

    /// Radio recording quality preference: high,std,med,low (default: high,std,med,low)
    #[arg(long = "radio-quality", alias = "radioquality")]
    pub radio_quality: Option<String>,

    /// Prefer 25fps (or lower-bitrate 50fps) streams for TV programmes
    #[arg(long = "tv-lower-bitrate", alias = "tvlbr")]
    pub tv_lower_bitrate: bool,

    /// Ignore programme history (re-record if previously downloaded)
    #[arg(long = "force")]
    pub force: bool,

    /// Hide previously recorded programmes from search results
    #[arg(long = "hide")]
    pub hide: bool,

    /// Download subtitles into SRT format
    #[arg(long = "subtitles", alias = "subs")]
    pub subtitles: bool,

    /// Embed soft subtitles in MP4 output file
    #[arg(long = "subs-embed")]
    pub subs_embed: bool,

    /// Create monochrome subtitles
    #[arg(long = "subs-mono")]
    pub subs_mono: bool,

    /// Only download subtitles, not the programme
    #[arg(long = "subtitles-only", alias = "subs-only")]
    pub subtitles_only: bool,

    /// Do not download programme if subtitles are not available
    #[arg(long = "subtitles-required", alias = "subs-required")]
    pub subs_required: bool,

    /// Offset subtitle timestamps by milliseconds
    #[arg(long = "suboffset", default_value = "0")]
    pub suboffset: i64,

    /// Additionally save raw TTML subtitles file
    #[arg(long = "subs-raw")]
    pub subs_raw: bool,

    /// Download thumbnail image
    #[arg(long = "thumb", alias = "thumbnail")]
    pub thumb: bool,

    /// Only download thumbnail image, not the programme
    #[arg(long = "thumb-only", alias = "thumbnail-only")]
    pub thumb_only: bool,

    /// Thumbnail width (e.g. 1920, 1280, 704)
    #[arg(long = "thumbnail-size", default_value = "1920")]
    pub thumbnail_size: u32,

    /// Don't remux, save in native container format
    #[arg(long = "raw")]
    pub raw: bool,

    /// Only download audio stream for TV programme
    #[arg(long = "audio-only")]
    pub audio_only: bool,

    /// Version preference list (default,audiodescribed,signed)
    #[arg(long = "versions", default_value = "default")]
    pub versions: String,

    /// Comma-separated media stream formats to include (hls, dash)
    #[arg(long = "include-format")]
    pub include_format: Option<String>,

    /// Comma-separated media stream formats to exclude (hls, dash)
    #[arg(long = "exclude-format")]
    pub exclude_format: Option<String>,

    /// Comma-separated CDN suppliers to include (akamai, limelight, bidi, cloudfront)
    #[arg(long = "include-supplier")]
    pub include_supplier: Option<String>,

    /// Comma-separated CDN suppliers to exclude
    #[arg(long = "exclude-supplier")]
    pub exclude_supplier: Option<String>,

    /// Overwrite existing recordings
    #[arg(long = "overwrite")]
    pub overwrite: bool,

    /// Test only - no recording
    #[arg(short = 't', long = "test")]
    pub test: bool,

    // ==================== Output & Tagging Options ====================
    /// Output directory for recordings
    #[arg(short = 'o', long = "output")]
    pub output_dir: Option<String>,

    /// Output directory for TV recordings (overrides --output)
    #[arg(long = "output-tv")]
    pub output_tv: Option<String>,

    /// Output directory for radio recordings (overrides --output)
    #[arg(long = "output-radio")]
    pub output_radio: Option<String>,

    /// Filename prefix template (default: <name> - <episode> <pid> <version>)
    #[arg(long = "file-prefix")]
    pub file_prefix: Option<String>,

    /// Save recorded files into subdirectories
    #[arg(short = 's', long = "subdir", alias = "subdirs")]
    pub subdir: bool,

    /// Subdirectory naming template (default: <name>)
    #[arg(long = "subdir-format")]
    pub subdir_format: Option<String>,

    /// Keep whitespace in file and directory names (default replaces with underscores)
    #[arg(short = 'w', long = "whitespace")]
    pub whitespace: bool,

    /// Command to run after successful recording
    #[arg(short = 'c', long = "command")]
    pub command: Option<String>,

    /// Command to run after TV recording
    #[arg(long = "command-tv")]
    pub command_tv: Option<String>,

    /// Command to run after radio recording
    #[arg(long = "command-radio")]
    pub command_radio: Option<String>,

    /// Create metadata info file (json, generic)
    #[arg(long = "metadata")]
    pub metadata: Option<String>,

    /// Create metadata info file without recording
    #[arg(long = "metadata-only")]
    pub metadata_only: bool,

    /// Do not tag downloaded programmes
    #[arg(long = "no-tag", alias = "notag")]
    pub no_tag: bool,

    /// Tag downloaded programmes as podcasts
    #[arg(long = "tag-podcast")]
    pub tag_podcast: bool,

    // ==================== Preferences ====================
    /// Add/change specified saved user options
    #[arg(long = "prefs-add")]
    pub prefs_add: bool,

    /// Remove specified saved user options
    #[arg(long = "prefs-del")]
    pub prefs_del: bool,

    /// Show saved user options
    #[arg(long = "prefs-show")]
    pub prefs_show: bool,

    /// Remove ALL saved user options
    #[arg(long = "prefs-clear")]
    pub prefs_clear: bool,

    // ==================== External & System ====================
    /// Location of ffmpeg binary
    #[arg(long = "ffmpeg")]
    pub ffmpeg: Option<String>,

    /// Location of AtomicParsley binary
    #[arg(long = "atomicparsley")]
    pub atomicparsley: Option<String>,

    /// Override profile directory
    #[arg(long = "profile-dir")]
    pub profile_dir: Option<String>,

    /// Web proxy URL
    #[arg(short = 'p', long = "proxy")]
    pub proxy: Option<String>,

    /// Ignore proxy settings
    #[arg(long = "no-proxy")]
    pub no_proxy: bool,

    /// Show verbose output
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Reduce logging output
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}
