use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PvrJob {
    pub name: String,
    pub path: PathBuf,
    pub is_enabled: bool,
    pub options: HashMap<String, String>,
}

pub struct PvrManager {
    pub pvr_dir: PathBuf,
}

impl PvrManager {
    pub fn new<P: AsRef<Path>>(pvr_dir: P) -> Self {
        let pvr_dir = pvr_dir.as_ref().to_path_buf();
        if !pvr_dir.exists() {
            let _ = fs::create_dir_all(&pvr_dir);
        }
        Self { pvr_dir }
    }

    /// Lists all PVR jobs in the pvr directory
    pub fn list_jobs(&self) -> Vec<PvrJob> {
        let mut jobs = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.pvr_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if file_name.starts_with('.') || file_name.ends_with(".tmp") {
                        continue;
                    }
                    let is_disabled = file_name.ends_with(".disabled");
                    let name = file_name.trim_end_matches(".disabled").to_string();

                    let mut options = HashMap::new();
                    if let Ok(f) = File::open(&path) {
                        let reader = BufReader::new(f);
                        for line in reader.lines().flatten() {
                            let line = line.trim();
                            if line.is_empty() || line.starts_with('#') {
                                continue;
                            }
                            if let Some((k, v)) = line.split_once(|c: char| c.is_whitespace() || c == '=') {
                                options.insert(k.trim().to_string(), v.trim().to_string());
                            } else {
                                options.insert(line.to_string(), "1".to_string());
                            }
                        }
                    }

                    jobs.push(PvrJob {
                        name,
                        path,
                        is_enabled: !is_disabled,
                        options,
                    });
                }
            }
        }
        jobs.sort_by(|a, b| a.name.cmp(&b.name));
        jobs
    }

    /// Adds or updates a PVR job
    pub fn add_job(&self, name: &str, options: &HashMap<String, String>) -> std::io::Result<()> {
        let job_path = self.pvr_dir.join(name);
        let mut file = File::create(job_path)?;
        let mut keys: Vec<&String> = options.keys().collect();
        keys.sort();
        for k in keys {
            if let Some(v) = options.get(k) {
                writeln!(file, "{} {}", k, v)?;
            }
        }
        println!("INFO: Added PVR search '{}'", name);
        Ok(())
    }

    /// Deletes a PVR job
    pub fn delete_job(&self, name: &str) -> std::io::Result<()> {
        let job_path = self.pvr_dir.join(name);
        let disabled_path = self.pvr_dir.join(format!("{}.disabled", name));

        let mut deleted = false;
        if job_path.is_file() {
            fs::remove_file(job_path)?;
            deleted = true;
        }
        if disabled_path.is_file() {
            fs::remove_file(disabled_path)?;
            deleted = true;
        }

        if deleted {
            println!("INFO: Deleted PVR search '{}'", name);
        } else {
            println!("WARNING: PVR search '{}' does not exist", name);
        }
        Ok(())
    }

    /// Enables a PVR job
    pub fn enable_job(&self, name: &str) -> std::io::Result<()> {
        let disabled_path = self.pvr_dir.join(format!("{}.disabled", name));
        let job_path = self.pvr_dir.join(name);

        if disabled_path.is_file() {
            fs::rename(disabled_path, job_path)?;
            println!("INFO: Enabled PVR search '{}'", name);
        } else if job_path.is_file() {
            println!("INFO: PVR search '{}' is already enabled", name);
        } else {
            println!("WARNING: PVR search '{}' does not exist", name);
        }
        Ok(())
    }

    /// Disables a PVR job
    pub fn disable_job(&self, name: &str) -> std::io::Result<()> {
        let job_path = self.pvr_dir.join(name);
        let disabled_path = self.pvr_dir.join(format!("{}.disabled", name));

        if job_path.is_file() {
            fs::rename(job_path, disabled_path)?;
            println!("INFO: Disabled PVR search '{}'", name);
        } else if disabled_path.is_file() {
            println!("INFO: PVR search '{}' is already disabled", name);
        } else {
            println!("WARNING: PVR search '{}' does not exist", name);
        }
        Ok(())
    }

    /// Displays all PVR jobs and options
    pub fn display_list(&self) {
        let jobs = self.list_jobs();
        if jobs.is_empty() {
            println!("INFO: No PVR searches saved");
            return;
        }

        println!("All PVR Searches:");
        for job in jobs {
            let status = if job.is_enabled { "ENABLED" } else { "DISABLED" };
            println!("PVR Search: {} [{}]", job.name, status);
            let mut keys: Vec<&String> = job.options.keys().collect();
            keys.sort();
            for k in keys {
                println!("\t{} = {}", k, job.options.get(k).unwrap());
            }
            println!();
        }
    }
}
