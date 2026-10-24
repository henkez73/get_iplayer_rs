use crate::models::{HistoryEntry, Programme};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub struct History {
    pub file_path: PathBuf,
    pub records: HashMap<String, HistoryEntry>,
}

impl History {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let mut hist = Self {
            file_path: path.as_ref().to_path_buf(),
            records: HashMap::new(),
        };
        hist.load();
        hist
    }

    pub fn load(&mut self) {
        if !self.file_path.is_file() {
            return;
        }

        if let Ok(file) = File::open(&self.file_path) {
            let reader = BufReader::new(file);
            let cols = [
                "pid", "name", "episode", "type", "timeadded", "mode", "filename",
                "versions", "duration", "desc", "channel", "categories", "thumbnail",
                "guidance", "web", "episodenum", "seriesnum",
            ];

            for line in reader.lines().flatten() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                let parts: Vec<&str> = line.split('|').collect();
                let mut map = HashMap::new();
                for (i, col) in cols.iter().enumerate() {
                    if i < parts.len() {
                        map.insert(*col, parts[i]);
                    }
                }

                let pid = map.get("pid").unwrap_or(&"").to_string();
                if pid.is_empty() {
                    continue;
                }

                let entry = HistoryEntry {
                    pid: pid.clone(),
                    name: map.get("name").unwrap_or(&"").to_string(),
                    episode: map.get("episode").unwrap_or(&"").to_string(),
                    prog_type: map.get("type").unwrap_or(&"").to_string(),
                    timeadded: map.get("timeadded").and_then(|s| s.parse().ok()).unwrap_or(0),
                    mode: map.get("mode").unwrap_or(&"").to_string(),
                    filename: map.get("filename").unwrap_or(&"").to_string(),
                    versions: map.get("versions").unwrap_or(&"").to_string(),
                    duration: map.get("duration").and_then(|s| s.parse().ok()).unwrap_or(0),
                    desc: map.get("desc").unwrap_or(&"").to_string(),
                    channel: map.get("channel").unwrap_or(&"").to_string(),
                    categories: map.get("categories").unwrap_or(&"").to_string(),
                    thumbnail: map.get("thumbnail").unwrap_or(&"").to_string(),
                    guidance: map.get("guidance").unwrap_or(&"").to_string(),
                    web: map.get("web").unwrap_or(&"").to_string(),
                    episodenum: map.get("episodenum").and_then(|s| s.parse().ok()),
                    seriesnum: map.get("seriesnum").and_then(|s| s.parse().ok()),
                };

                self.records.insert(pid, entry);
            }
        }
    }

    pub fn check(&self, pid: &str) -> bool {
        self.records.contains_key(pid)
    }

    pub fn add(
        &mut self,
        prog: &Programme,
        mode: &str,
        filename: &str,
        version: &str,
    ) -> std::io::Result<()> {
        let now = chrono::Utc::now().timestamp();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;

        writeln!(
            file,
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
            prog.pid,
            prog.name.replace('|', "-"),
            prog.episode.replace('|', "-"),
            prog.prog_type.as_str(),
            now,
            mode,
            filename.replace('|', "-"),
            version,
            prog.duration,
            prog.desc.replace('|', "-").replace('\n', " ").replace('\r', ""),
            prog.channel.replace('|', "-"),
            prog.categories.replace('|', "-"),
            prog.thumbnail,
            prog.guidance.as_deref().unwrap_or(""),
            prog.web,
            prog.episodenum.map(|n| n.to_string()).unwrap_or_default(),
            prog.seriesnum.map(|n| n.to_string()).unwrap_or_default()
        )?;

        let entry = HistoryEntry {
            pid: prog.pid.clone(),
            name: prog.name.clone(),
            episode: prog.episode.clone(),
            prog_type: prog.prog_type.as_str().to_string(),
            timeadded: now,
            mode: mode.to_string(),
            filename: filename.to_string(),
            versions: version.to_string(),
            duration: prog.duration,
            desc: prog.desc.clone(),
            channel: prog.channel.clone(),
            categories: prog.categories.clone(),
            thumbnail: prog.thumbnail.clone(),
            guidance: prog.guidance.clone().unwrap_or_default(),
            web: prog.web.clone(),
            episodenum: prog.episodenum,
            seriesnum: prog.seriesnum,
        };
        self.records.insert(prog.pid.clone(), entry);

        Ok(())
    }

    pub fn search(&self, terms: &[String]) -> Vec<&HistoryEntry> {
        let regexes: Vec<regex::Regex> = terms
            .iter()
            .filter_map(|t| regex::RegexBuilder::new(t).case_insensitive(true).build().ok())
            .collect();

        let mut results: Vec<&HistoryEntry> = self.records.values().filter(|e| {
            if regexes.is_empty() {
                return true;
            }
            regexes.iter().all(|r| r.is_match(&e.name) || r.is_match(&e.episode) || r.is_match(&e.pid))
        }).collect();

        results.sort_by_key(|e| e.timeadded);
        results
    }

    pub fn list(&self, terms: &[String]) {
        let matches = self.search(terms);
        if matches.is_empty() {
            println!("INFO: 0 Matching Programmes in History");
            return;
        }

        println!("Matches in download history:");
        for (i, entry) in matches.iter().enumerate() {
            let dt = chrono::DateTime::from_timestamp(entry.timeadded, 0)
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "".to_string());

            println!(
                "{}:  {} - {}, {}, {}, {}",
                i + 1,
                entry.name,
                entry.episode,
                entry.channel,
                entry.pid,
                dt
            );
        }
        println!("INFO: {} Matching Programmes", matches.len());
    }
}
