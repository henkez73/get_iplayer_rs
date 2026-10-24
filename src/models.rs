use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProgrammeType {
    Tv,
    Radio,
    All,
}

impl ProgrammeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProgrammeType::Tv => "tv",
            ProgrammeType::Radio => "radio",
            ProgrammeType::All => "all",
        }
    }

    pub fn index_min(&self) -> usize {
        match self {
            ProgrammeType::Tv => 1,
            ProgrammeType::Radio => 30001,
            ProgrammeType::All => 1,
        }
    }

    pub fn index_max(&self) -> usize {
        match self {
            ProgrammeType::Tv => 29999,
            ProgrammeType::Radio => 99999,
            ProgrammeType::All => 99999,
        }
    }
}

impl std::str::FromStr for ProgrammeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tv" => Ok(ProgrammeType::Tv),
            "radio" => Ok(ProgrammeType::Radio),
            "all" => Ok(ProgrammeType::All),
            other => Err(format!("Unknown programme type '{}'", other)),
        }
    }
}

impl fmt::Display for ProgrammeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Quality {
    // TV
    Fhd,    // 1080p
    Hd,     // 720p
    Sd,     // 540p
    Web,    // 396p
    Mobile, // 288p
    // Radio
    High,   // 320k
    Std,    // 128k
    Med,    // 96k
    Low,    // 48k
}

impl Quality {
    pub fn from_str_lenient(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "fhd" | "1080" | "1080p" => Some(Quality::Fhd),
            "hd" | "720" | "720p" => Some(Quality::Hd),
            "sd" | "540" | "540p" => Some(Quality::Sd),
            "web" | "396" | "396p" => Some(Quality::Web),
            "mobile" | "288" | "288p" => Some(Quality::Mobile),
            "high" | "320" | "320k" => Some(Quality::High),
            "std" | "128" | "128k" => Some(Quality::Std),
            "med" | "96" | "96k" => Some(Quality::Med),
            "low" | "48" | "48k" => Some(Quality::Low),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Quality::Fhd => "fhd",
            Quality::Hd => "hd",
            Quality::Sd => "sd",
            Quality::Web => "web",
            Quality::Mobile => "mobile",
            Quality::High => "high",
            Quality::Std => "std",
            Quality::Med => "med",
            Quality::Low => "low",
        }
    }
}

/// Represents a BBC Programme
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Programme {
    pub index: usize,
    pub prog_type: ProgrammeType,
    pub name: String,
    pub episode: String,
    pub seriesnum: Option<u32>,
    pub episodenum: Option<u32>,
    pub pid: String,
    pub channel: String,
    pub available: String,      // ISO string or formatted date
    pub expires: i64,          // epoch seconds
    pub duration: u32,         // seconds
    pub desc: String,
    pub web: String,
    pub thumbnail: String,
    pub timeadded: i64,        // epoch seconds
    // Additional runtime metadata
    pub categories: String,
    pub verpids: HashMap<String, String>, // version -> verpid
    pub durations: HashMap<String, u32>,
    pub releaseyear: Option<u32>,
    pub guidance: Option<String>,
    pub player: Option<String>,
    pub versions: Vec<String>,
}

impl Default for Programme {
    fn default() -> Self {
        Self {
            index: 0,
            prog_type: ProgrammeType::Tv,
            name: String::new(),
            episode: String::new(),
            seriesnum: None,
            episodenum: None,
            pid: String::new(),
            channel: String::new(),
            available: String::new(),
            expires: 0,
            duration: 0,
            desc: String::new(),
            web: String::new(),
            thumbnail: "https://ichef.bbci.co.uk/images/ic/1920xn/p01tqv8z.png".to_string(),
            timeadded: 0,
            categories: String::new(),
            verpids: HashMap::new(),
            durations: HashMap::new(),
            releaseyear: None,
            guidance: None,
            player: None,
            versions: Vec::new(),
        }
    }
}

impl Programme {
    /// Generate substitution dictionary for filename / subdir templates
    pub fn template_vars(&self, version: &str, ext: &str) -> HashMap<&'static str, String> {
        let mut vars = HashMap::new();
        vars.insert("name", self.name.clone());
        vars.insert("episode", if self.episode.is_empty() || self.episode == "-" { self.name.clone() } else { self.episode.clone() });
        vars.insert("episodeshort", if self.episode.is_empty() || self.episode == "-" { self.name.clone() } else { self.episode.clone() });
        vars.insert("pid", self.pid.clone());
        vars.insert("version", version.to_string());
        vars.insert("channel", self.channel.clone());
        vars.insert("type", self.prog_type.as_str().to_string());
        vars.insert("ext", ext.to_string());
        vars.insert("desc", self.desc.clone());
        vars.insert("duration", self.duration.to_string());
        vars.insert("durations", crate::utils::format_duration(self.duration));
        if let Some(s) = self.seriesnum {
            vars.insert("seriesnum", s.to_string());
            vars.insert("series", format!("Series {}", s));
        } else {
            vars.insert("seriesnum", "".to_string());
            vars.insert("series", "".to_string());
        }
        if let Some(e) = self.episodenum {
            vars.insert("episodenum", e.to_string());
            vars.insert("senum", format!("s{:02}e{:02}", self.seriesnum.unwrap_or(1), e));
        } else {
            vars.insert("episodenum", "".to_string());
            vars.insert("senum", "".to_string());
        }
        vars.insert("firstbcast", self.available.clone());
        vars.insert("dldate", chrono::Utc::now().format("%Y-%m-%d").to_string());
        vars
    }
}

/// Represents an entry in `download_history`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub pid: String,
    pub name: String,
    pub episode: String,
    pub prog_type: String,
    pub timeadded: i64,
    pub mode: String,
    pub filename: String,
    pub versions: String,
    pub duration: u32,
    pub desc: String,
    pub channel: String,
    pub categories: String,
    pub thumbnail: String,
    pub guidance: String,
    pub web: String,
    pub episodenum: Option<u32>,
    pub seriesnum: Option<u32>,
}

/// Stream connection info resolved from Mediaselector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConnection {
    pub href: String,
    pub transfer_format: String, // "hls", "dash", etc.
    pub supplier: String,        // "akamai", "limelight", "cloudfront", "bidi"
    pub protocol: String,        // "https", "http"
    pub priority: u32,
    pub kind: String,            // "video", "audio", "captions"
    pub bitrate: u32,            // kbps
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub quality: Quality,
    pub service: String,
}
