use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub profile_dir: PathBuf,
    pub options: HashMap<String, String>,
}

impl Config {
    /// Discovers profile directory ($GETIPLAYER_PROFILE or ~/.get_iplayer_rs)
    pub fn new(profile_override: Option<&str>) -> Self {
        let profile_dir = if let Some(p) = profile_override {
            PathBuf::from(p)
        } else if let Ok(env_profile) = std::env::var("GETIPLAYER_PROFILE") {
            PathBuf::from(env_profile)
        } else {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".get_iplayer_rs")
        };

        // Ensure profile directory exists
        if !profile_dir.exists() {
            let _ = fs::create_dir_all(&profile_dir);
        }

        let mut config = Self {
            profile_dir,
            options: HashMap::new(),
        };

        config.load_options();
        config
    }

    pub fn options_file(&self) -> PathBuf {
        self.profile_dir.join("options")
    }

    pub fn presets_dir(&self) -> PathBuf {
        self.profile_dir.join("presets")
    }

    pub fn history_file(&self) -> PathBuf {
        self.profile_dir.join("download_history")
    }

    pub fn cache_file(&self, prog_type: &str) -> PathBuf {
        self.profile_dir.join(format!("{}.cache", prog_type))
    }

    pub fn pvr_dir(&self) -> PathBuf {
        self.profile_dir.join("pvr")
    }

    pub fn load_options(&mut self) {
        let opt_path = self.options_file();
        if !opt_path.is_file() {
            return;
        }

        if let Ok(file) = File::open(&opt_path) {
            let reader = BufReader::new(file);
            for line in reader.lines().flatten() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let mut parts = trimmed.splitn(2, |c: char| c.is_whitespace() || c == '=');
                let key = parts.next().unwrap_or("").trim().to_string();
                let val = parts.next().map(|v| v.trim().to_string()).unwrap_or_else(|| "1".to_string());
                if !key.is_empty() {
                    self.options.insert(key, val);
                }
            }
        }
    }

    pub fn save_options(&self) -> std::io::Result<()> {
        let opt_path = self.options_file();
        let mut file = File::create(opt_path)?;
        let mut sorted_keys: Vec<&String> = self.options.keys().collect();
        sorted_keys.sort();
        for k in sorted_keys {
            if let Some(v) = self.options.get(k) {
                writeln!(file, "{} {}", k, v)?;
            }
        }
        Ok(())
    }

    pub fn prefs_add(&mut self, key: &str, val: &str) {
        self.options.insert(key.to_string(), val.to_string());
    }

    pub fn prefs_del(&mut self, key: &str) {
        self.options.remove(key);
    }

    pub fn prefs_clear(&mut self) {
        self.options.clear();
    }

    pub fn show_prefs(&self) {
        println!("Options in {}:", self.options_file().display());
        let mut keys: Vec<&String> = self.options.keys().collect();
        keys.sort();
        if keys.is_empty() {
            println!("  (No saved options)");
        } else {
            for k in keys {
                println!("\t{} = {}", k, self.options.get(k).unwrap());
            }
        }
    }
}
