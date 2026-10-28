use crate::models::{Programme, ProgrammeType};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub const CACHE_HEADER: &str = "#index|type|name|episode|seriesnum|episodenum|pid|channel|available|expires|duration|desc|web|thumbnail|timeadded|";

pub struct Cache {
    pub programmes: Vec<Programme>,
    pub by_pid: HashMap<String, usize>,
    pub by_index: HashMap<usize, usize>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            programmes: Vec::new(),
            by_pid: HashMap::new(),
            by_index: HashMap::new(),
        }
    }

    pub fn load_file<P: AsRef<Path>>(path: P, default_type: ProgrammeType) -> Self {
        let path = path.as_ref();
        let mut cache = Self::new();
        if !path.is_file() {
            return cache;
        }

        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            let mut format_cols: Vec<String> = vec![
                "index".into(), "type".into(), "name".into(), "episode".into(),
                "seriesnum".into(), "episodenum".into(), "pid".into(), "channel".into(),
                "available".into(), "expires".into(), "duration".into(), "desc".into(),
                "web".into(), "thumbnail".into(), "timeadded".into(),
            ];

            for line in reader.lines().map_while(Result::ok) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line.starts_with('#') {
                    // Check if custom header
                    let cols: Vec<String> = line.trim_start_matches('#')
                        .split('|')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if cols.len() >= 5 {
                        format_cols = cols;
                    }
                    continue;
                }

                let values: Vec<&str> = line.split('|').collect();
                let mut map: HashMap<&str, &str> = HashMap::new();
                for (i, col) in format_cols.iter().enumerate() {
                    if i < values.len() {
                        map.insert(col.as_str(), values[i]);
                    }
                }

                let pid = map.get("pid").unwrap_or(&"").trim().to_string();
                if pid.is_empty() {
                    continue;
                }

                let index: usize = map.get("index").and_then(|s| s.parse().ok()).unwrap_or(0);
                let prog_type_str = map.get("type").unwrap_or(&"").to_lowercase();
                let prog_type = match prog_type_str.as_str() {
                    "radio" => ProgrammeType::Radio,
                    "tv" => ProgrammeType::Tv,
                    _ => default_type,
                };

                let name = map.get("name").unwrap_or(&"").to_string();
                let episode = map.get("episode").unwrap_or(&"").to_string();
                let seriesnum: Option<u32> = map.get("seriesnum").and_then(|s| s.parse().ok());
                let episodenum: Option<u32> = map.get("episodenum").and_then(|s| s.parse().ok());
                let channel = map.get("channel").unwrap_or(&"").to_string();
                let available = map.get("available").unwrap_or(&"").to_string();
                let expires: i64 = map.get("expires").and_then(|s| s.parse().ok()).unwrap_or(0);
                let duration: u32 = map.get("duration").and_then(|s| s.parse().ok()).unwrap_or(0);
                let desc = map.get("desc").unwrap_or(&"").to_string();
                let web = map.get("web").unwrap_or(&"").to_string();
                let thumbnail = map.get("thumbnail").unwrap_or(&"").to_string();
                let timeadded: i64 = map.get("timeadded").and_then(|s| s.parse().ok()).unwrap_or(0);

                let prog = Programme {
                    index,
                    prog_type,
                    name,
                    episode,
                    seriesnum,
                    episodenum,
                    pid: pid.clone(),
                    channel,
                    available,
                    expires,
                    duration,
                    desc,
                    web,
                    thumbnail,
                    timeadded,
                    ..Default::default()
                };

                let idx = cache.programmes.len();
                cache.by_pid.insert(pid, idx);
                if index > 0 {
                    cache.by_index.insert(index, idx);
                }
                cache.programmes.push(prog);
            }
        }

        cache
    }

    pub fn save_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let path = path.as_ref();
        let tmp_path = path.with_extension("tmp");
        {
            let mut file = File::create(&tmp_path)?;
            writeln!(file, "{}", CACHE_HEADER)?;
            for p in &self.programmes {
                writeln!(
                    file,
                    "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
                    p.index,
                    p.prog_type.as_str(),
                    p.name.replace('|', "-"),
                    p.episode.replace('|', "-"),
                    p.seriesnum.map(|n| n.to_string()).unwrap_or_default(),
                    p.episodenum.map(|n| n.to_string()).unwrap_or_default(),
                    p.pid,
                    p.channel.replace('|', "-"),
                    p.available,
                    p.expires,
                    p.duration,
                    p.desc.replace('|', "-").replace('\n', " ").replace('\r', ""),
                    p.web,
                    p.thumbnail,
                    p.timeadded
                )?;
            }
        }
        fs::rename(tmp_path, path)?;
        Ok(())
    }

    pub fn sort_and_reindex(&mut self, min_index: usize) {
        // Sort by name, then series, then episode, then available
        self.programmes.sort_by(|a, b| {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
                .then_with(|| a.seriesnum.cmp(&b.seriesnum))
                .then_with(|| a.episodenum.cmp(&b.episodenum))
                .then_with(|| a.episode.to_lowercase().cmp(&b.episode.to_lowercase()))
                .then_with(|| a.available.cmp(&b.available))
        });

        self.by_index.clear();
        self.by_pid.clear();
        for (i, p) in self.programmes.iter_mut().enumerate() {
            p.index = min_index + i;
            self.by_index.insert(p.index, i);
            self.by_pid.insert(p.pid.clone(), i);
        }
    }

    pub fn most_recent_addition(&self) -> i64 {
        self.programmes.iter().map(|p| p.timeadded).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cache_roundtrip() {
        let mut cache = Cache::new();
        cache.programmes.push(Programme {
            index: 208,
            prog_type: ProgrammeType::Tv,
            name: "Doctor Who".to_string(),
            episode: "1. The Bells of Saint John".to_string(),
            seriesnum: Some(7),
            episodenum: Some(1),
            pid: "b01rryzz".to_string(),
            channel: "BBC One".to_string(),
            available: "2023-04-01T18:00:00+01:00".to_string(),
            expires: 1682874000,
            duration: 2700,
            desc: "The Doctor encounters Clara Oswald.".to_string(),
            web: "https://www.bbc.co.uk/programmes/b01rryzz".to_string(),
            thumbnail: "https://ichef.bbci.co.uk/test.jpg".to_string(),
            timeadded: 1680368400,
            ..Default::default()
        });

        let tmp = NamedTempFile::new().unwrap();
        cache.save_file(tmp.path()).unwrap();

        let loaded = Cache::load_file(tmp.path(), ProgrammeType::Tv);
        assert_eq!(loaded.programmes.len(), 1);
        let p = &loaded.programmes[0];
        assert_eq!(p.index, 208);
        assert_eq!(p.pid, "b01rryzz");
        assert_eq!(p.name, "Doctor Who");
        assert_eq!(p.episode, "1. The Bells of Saint John");
        assert_eq!(p.seriesnum, Some(7));
        assert_eq!(p.episodenum, Some(1));
    }
}
