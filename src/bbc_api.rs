use crate::models::{Programme, ProgrammeType};
use crate::utils::{extract_episode_num, extract_series_num};
use regex::Regex;
use serde_json::Value;

pub struct BbcApi {
    client: reqwest::Client,
}

impl BbcApi {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client }
    }

    /// Checks if a string looks like a BBC PID (e.g. b01rryzz, p015pksy, m001abcd)
    pub fn is_valid_pid(pid: &str) -> bool {
        let re = Regex::new(r"^[b-df-hj-np-tv-z0-9]{8,}$").unwrap();
        re.is_match(pid.trim())
    }

    /// Extracts PID from URL or raw string
    pub fn parse_pid(input: &str) -> Option<String> {
        let input = input.trim();
        if Self::is_valid_pid(input) {
            return Some(input.to_string());
        }

        if let Ok(parsed_url) = url::Url::parse(input) {
            let segments: Vec<&str> = parsed_url.path_segments()?.collect();
            // Look for segments matching PID pattern from right to left
            for seg in segments.iter().rev() {
                if Self::is_valid_pid(seg) {
                    return Some(seg.to_string());
                }
            }
        }

        // Regex fallback
        let re = Regex::new(r"/(?:episode|play|programmes|brand)/([b-df-hj-np-tv-z0-9]{8,})").unwrap();
        if let Some(caps) = re.captures(input) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }

        None
    }

    /// Fetches programme metadata and version PIDs (vpid) given a PID
    pub async fn get_programme_info(
        &self,
        pid: &str,
    ) -> Result<Programme, Box<dyn std::error::Error + Send + Sync>> {
        let mut prog = Programme {
            pid: pid.to_string(),
            web: format!("https://www.bbc.co.uk/programmes/{}", pid),
            ..Default::default()
        };

        // Try JSON playlist first: https://www.bbc.co.uk/programmes/{pid}/playlist.json
        let playlist_url = format!("https://www.bbc.co.uk/programmes/{}/playlist.json", pid);
        let resp = self.client.get(&playlist_url).send().await;

        let mut got_json = false;
        if let Ok(res) = resp {
            if res.status().is_success() {
                if let Ok(body) = res.text().await {
                    if let Ok(json) = serde_json::from_str::<Value>(&body) {
                        got_json = true;
                        self.populate_from_playlist_json(&mut prog, &json);
                    }
                }
            }
        }

        // If versions or title are still empty, try scraping the episode HTML page
        if prog.verpids.is_empty() || prog.name.is_empty() {
            let iplayer_url = format!("https://www.bbc.co.uk/iplayer/episode/{}", pid);
            if let Ok(res) = self.client.get(&iplayer_url).send().await {
                if res.status().is_success() {
                    if let Ok(html) = res.text().await {
                        prog.prog_type = ProgrammeType::Tv;
                        self.scrape_episode_html(&mut prog, &html);
                    }
                }
            }
        }

        // If still empty and possibly radio, try BBC Sounds play page
        if prog.verpids.is_empty() || prog.name.is_empty() {
            let sounds_url = format!("https://www.bbc.co.uk/sounds/play/{}", pid);
            if let Ok(res) = self.client.get(&sounds_url).send().await {
                if res.status().is_success() {
                    if let Ok(html) = res.text().await {
                        prog.prog_type = ProgrammeType::Radio;
                        self.scrape_sounds_html(&mut prog, &html);
                    }
                }
            }
        }

        if prog.verpids.is_empty() {
            // As a last fallback, the episode PID itself might be the version PID (common in box sets or clips)
            prog.verpids.insert("default".to_string(), pid.to_string());
            prog.versions.push("default".to_string());
        }

        if prog.name.is_empty() {
            prog.name = pid.to_string();
        }

        Ok(prog)
    }

    fn populate_from_playlist_json(&self, prog: &mut Programme, json: &Value) {
        if let Some(def_ver) = json.get("defaultAvailableVersion") {
            if let Some(smp) = def_ver.get("smpConfig") {
                if let Some(title) = smp.get("title").and_then(|t| t.as_str()) {
                    let decoded = htmlescape::decode_html(title).unwrap_or_else(|_| title.to_string());
                    if let Some((n, e)) = decoded.split_once(':') {
                        prog.name = n.trim().to_string();
                        prog.episode = e.trim().to_string();
                    } else {
                        prog.name = decoded;
                    }
                }
                if let Some(channel) = smp.get("masterBrandName").and_then(|c| c.as_str()) {
                    prog.channel = channel.to_string();
                }
                if let Some(summary) = smp.get("summary").and_then(|s| s.as_str()) {
                    prog.desc = htmlescape::decode_html(summary).unwrap_or_else(|_| summary.to_string());
                }
                if let Some(thumb) = smp.get("holdingImageURL").and_then(|t| t.as_str()) {
                    let full_thumb = thumb.replace("$recipe", "1920xn");
                    prog.thumbnail = if full_thumb.starts_with("http") {
                        full_thumb
                    } else {
                        format!("https:{}", full_thumb)
                    };
                }
                if let Some(items) = smp.get("items").and_then(|i| i.as_array()) {
                    if let Some(first_item) = items.first() {
                        if let Some(kind) = first_item.get("kind").and_then(|k| k.as_str()) {
                            if kind == "radioProgramme" {
                                prog.prog_type = ProgrammeType::Radio;
                            } else {
                                prog.prog_type = ProgrammeType::Tv;
                            }
                        }
                        if let Some(vpid) = first_item.get("vpid").and_then(|v| v.as_str()) {
                            prog.verpids.insert("default".to_string(), vpid.to_string());
                            prog.versions.push("default".to_string());
                        }
                    }
                }
            }
        }

        // Parse all available versions
        if let Some(all_vers) = json.get("allAvailableVersions").and_then(|a| a.as_array()) {
            for ver in all_vers {
                if let Some(vpid) = ver.get("pid").and_then(|p| p.as_str()) {
                    let duration = ver.get("duration").and_then(|d| d.as_u64()).unwrap_or(0) as u32;
                    let mut version_name = "default".to_string();

                    if let Some(types) = ver.get("types").and_then(|t| t.as_array()) {
                        let type_strs: Vec<String> = types.iter().filter_map(|t| t.as_str().map(|s| s.to_lowercase())).collect();
                        let is_ad = type_strs.iter().any(|s| s.contains("described") || s.contains("description"));
                        let is_signed = type_strs.iter().any(|s| s.contains("sign"));

                        if is_ad && is_signed {
                            version_name = "combined".to_string();
                        } else if is_ad {
                            version_name = "audiodescribed".to_string();
                        } else if is_signed {
                            version_name = "signed".to_string();
                        } else if let Some(first) = type_strs.first() {
                            let clean = first.split_whitespace().next().unwrap_or("default");
                            version_name = clean.to_string();
                        }
                    }

                    prog.durations.insert(version_name.clone(), duration);
                    if prog.duration == 0 && duration > 0 {
                        prog.duration = duration;
                    }
                    if !prog.versions.contains(&version_name) {
                        prog.versions.push(version_name.clone());
                    }
                    prog.verpids.insert(version_name, vpid.to_string());
                }
            }
        }

        // Try extracting series and episode numbers
        if prog.episodenum.is_none() {
            prog.episodenum = extract_episode_num(&prog.episode).or_else(|| extract_episode_num(&prog.desc));
        }
        if prog.seriesnum.is_none() {
            prog.seriesnum = extract_series_num(&format!("{} {}", prog.name, prog.episode));
        }
    }

    fn scrape_episode_html(&self, prog: &mut Programme, html: &str) {
        // Try extracting __IPLAYER_REDUX_STATE__ or data-playable or script json
        let re_data = Regex::new(r#"data-playable='([^']+)'"#).unwrap();
        if let Some(caps) = re_data.captures(html) {
            if let Some(m) = caps.get(1) {
                if let Ok(val) = serde_json::from_str::<Value>(m.as_str()) {
                    if let Some(vpid) = val.get("versionId").and_then(|v| v.as_str()) {
                        prog.verpids.insert("default".to_string(), vpid.to_string());
                        prog.versions.push("default".to_string());
                    }
                    if let Some(title) = val.get("title").and_then(|t| t.as_str()) {
                        prog.name = title.to_string();
                    }
                    if let Some(sub) = val.get("subtitle").and_then(|s| s.as_str()) {
                        prog.episode = sub.to_string();
                    }
                    if let Some(desc) = val.get("description").and_then(|d| d.as_str()) {
                        prog.desc = desc.to_string();
                    }
                }
            }
        }

        // General title extraction from <title>
        if prog.name.is_empty() {
            let re_title = Regex::new(r"<title>(.*?)</title>").unwrap();
            if let Some(caps) = re_title.captures(html) {
                if let Some(t) = caps.get(1) {
                    let clean = t.as_str().replace("BBC iPlayer - ", "").replace(" - BBC Sounds", "");
                    if let Some((n, e)) = clean.split_once(':') {
                        prog.name = n.trim().to_string();
                        prog.episode = e.trim().to_string();
                    } else {
                        prog.name = clean.trim().to_string();
                    }
                }
            }
        }
    }

    fn scrape_sounds_html(&self, prog: &mut Programme, html: &str) {
        let re_title = Regex::new(r"<title>(.*?)</title>").unwrap();
        if let Some(caps) = re_title.captures(html) {
            if let Some(t) = caps.get(1) {
                let clean = t.as_str().replace(" - BBC Sounds", "");
                if let Some((n, e)) = clean.split_once(':') {
                    prog.name = n.trim().to_string();
                    prog.episode = e.trim().to_string();
                } else {
                    prog.name = clean.trim().to_string();
                }
            }
        }

        // Try extracting PID or playable metadata
        let re_vpid = Regex::new(r#""vpid"\s*:\s*"([b-df-hj-np-tv-z0-9]{8,})""#).unwrap();
        if let Some(caps) = re_vpid.captures(html) {
            if let Some(vpid) = caps.get(1) {
                prog.verpids.insert("default".to_string(), vpid.as_str().to_string());
                prog.versions.push("default".to_string());
            }
        }
    }

    /// Recursively finds all episode PIDs for a series, season, or brand PID
    pub async fn get_recursive_pids(
        &self,
        brand_pid: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut pids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let re = Regex::new(r#"data-pid="([b-df-hj-np-tv-z0-9]{8,})""#).unwrap();

        // Check https://www.bbc.co.uk/programmes/{pid}/episodes/player
        for page in 1..=10 {
            let url = format!("https://www.bbc.co.uk/programmes/{}/episodes/player?page={}", brand_pid, page);
            let resp = self.client.get(&url).send().await?;
            if !resp.status().is_success() {
                break;
            }
            let html = resp.text().await?;

            let mut page_pids = 0;
            for cap in re.captures_iter(&html) {
                if let Some(m) = cap.get(1) {
                    let ep_pid = m.as_str().to_string();
                    if ep_pid != brand_pid && seen.insert(ep_pid.clone()) {
                        pids.push(ep_pid);
                        page_pids += 1;
                    }
                }
            }

            if page_pids == 0 {
                break;
            }
        }

        Ok(pids)
    }
}
