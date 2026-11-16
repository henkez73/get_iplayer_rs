use crate::models::{Programme, ProgrammeType};
use crate::utils::{extract_episode_num, extract_series_num};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

const IBL_BASE: &str = "https://ibl.api.bbci.co.uk/ibl/v1";

pub struct BbcApi {
    client: reqwest::Client,
    ibl_base: String,
}

impl BbcApi {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            ibl_base: IBL_BASE.to_string(),
        }
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
        let re =
            Regex::new(r"/(?:episode|play|programmes|brand)/([b-df-hj-np-tv-z0-9]{8,})").unwrap();
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

        let episode_url = format!(
            "{}/episodes/{}?rights=mobile&availability=available",
            self.ibl_base, pid
        );
        if let Ok(response) = self.client.get(&episode_url).send().await {
            if response.status().is_success() {
                if let Ok(json) = response.json::<Value>().await {
                    if let Some(episode) = json
                        .get("episodes")
                        .and_then(Value::as_array)
                        .and_then(|episodes| episodes.first())
                    {
                        populate_from_ibl_episode(&mut prog, episode);
                    }
                }
            }
        }

        if prog.verpids.is_empty() {
            // Keep the legacy playlist path for content outside the iBL catalogue.
            let playlist_url = format!("https://www.bbc.co.uk/programmes/{}/playlist.json", pid);
            let resp = self.client.get(&playlist_url).send().await;

            if let Ok(res) = resp {
                if res.status().is_success() {
                    if let Ok(body) = res.text().await {
                        if let Ok(json) = serde_json::from_str::<Value>(&body) {
                            self.populate_from_playlist_json(&mut prog, &json);
                        }
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

    /// Loads all currently available episodes for the selected episode's programme from iBL.
    pub async fn get_series_archive(
        &self,
        seed: &Programme,
    ) -> Result<Vec<Programme>, Box<dyn std::error::Error + Send + Sync>> {
        let episode_details: Value = self
            .client
            .get(format!(
                "{}/episodes/{}?rights=mobile&availability=available",
                self.ibl_base, seed.pid
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let brand_pid = ibl_brand_pid(&episode_details).ok_or_else(|| {
            std::io::Error::other(format!(
                "iBL did not return a programme ID for episode {}",
                seed.pid
            ))
        })?;

        let episode_entries = self.fetch_ibl_programme_episodes(brand_pid).await?;
        let mut episodes = HashMap::<String, Programme>::new();
        for episode in &episode_entries {
            if let Some(programme) = programme_from_ibl_episode(episode, seed) {
                episodes.insert(programme.pid.clone(), programme);
            }
        }
        let mut programmes: Vec<Programme> = episodes.into_values().collect();
        programmes.sort_by(|a, b| {
            a.seriesnum
                .cmp(&b.seriesnum)
                .then_with(|| a.episodenum.cmp(&b.episodenum))
                .then_with(|| a.episode.cmp(&b.episode))
        });
        if programmes.is_empty() {
            return Err(std::io::Error::other(format!(
                "iBL returned no available episodes for {}",
                seed.name
            ))
            .into());
        }
        Ok(programmes)
    }

    async fn fetch_ibl_programme_episodes(
        &self,
        programme_pid: &str,
    ) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
        let mut page: usize = 1;
        let mut episodes = Vec::new();
        loop {
            let listing: Value = self
                .client
                .get(format!(
                    "{}/programmes/{programme_pid}/episodes?rights=mobile&availability=available&per_page=100&page={page}",
                    self.ibl_base
                ))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let listing = listing.get("programme_episodes").unwrap_or(&listing);
            let count = listing
                .get("count")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize;
            let page_size = listing
                .get("per_page")
                .and_then(Value::as_u64)
                .unwrap_or(100) as usize;
            let elements = listing
                .get("elements")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if elements.is_empty() {
                break;
            }
            episodes.extend(elements);
            if !has_next_ibl_page(page, count, page_size) {
                break;
            }
            page += 1;
        }
        Ok(episodes)
    }

    fn populate_from_playlist_json(&self, prog: &mut Programme, json: &Value) {
        if let Some(def_ver) = json.get("defaultAvailableVersion") {
            if let Some(smp) = def_ver.get("smpConfig") {
                if let Some(title) = smp.get("title").and_then(|t| t.as_str()) {
                    let decoded =
                        htmlescape::decode_html(title).unwrap_or_else(|_| title.to_string());
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
                    prog.desc =
                        htmlescape::decode_html(summary).unwrap_or_else(|_| summary.to_string());
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
                        let type_strs: Vec<String> = types
                            .iter()
                            .filter_map(|t| t.as_str().map(|s| s.to_lowercase()))
                            .collect();
                        let is_ad = type_strs
                            .iter()
                            .any(|s| s.contains("described") || s.contains("description"));
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
            prog.episodenum =
                extract_episode_num(&prog.episode).or_else(|| extract_episode_num(&prog.desc));
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
                    let clean = t
                        .as_str()
                        .replace("BBC iPlayer - ", "")
                        .replace(" - BBC Sounds", "");
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
                prog.verpids
                    .insert("default".to_string(), vpid.as_str().to_string());
                prog.versions.push("default".to_string());
            }
        }
    }

    /// Recursively finds all episode PIDs for a series, season, or brand PID
    pub async fn get_recursive_pids(
        &self,
        brand_pid: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let episodes = self.fetch_ibl_programme_episodes(brand_pid).await?;
        Ok(episodes
            .iter()
            .filter_map(|episode| episode.get("id").and_then(Value::as_str))
            .map(str::to_string)
            .collect())
    }
}

impl Default for BbcApi {
    fn default() -> Self {
        Self::new()
    }
}

fn ibl_brand_pid(response: &Value) -> Option<&str> {
    response
        .get("episodes")?
        .as_array()?
        .first()?
        .get("tleo_id")?
        .as_str()
}

#[cfg(test)]
fn parse_ibl_episode_page(response: &Value, seed: &Programme) -> (usize, usize, Vec<Programme>) {
    let listing = response.get("programme_episodes").unwrap_or(response);
    let count = listing
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    let page_size = listing
        .get("per_page")
        .and_then(Value::as_u64)
        .unwrap_or(100) as usize;
    let programmes = listing
        .get("elements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|episode| programme_from_ibl_episode(episode, seed))
        .collect();
    (count, page_size, programmes)
}

fn programme_from_ibl_episode(episode: &Value, seed: &Programme) -> Option<Programme> {
    let pid = episode.get("id")?.as_str()?.to_string();
    let subtitle = episode
        .get("subtitle")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let slice_subtitle = episode
        .get("slice_subtitle")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let episode_title = if !slice_subtitle.is_empty() {
        slice_subtitle.to_string()
    } else if extract_series_num(subtitle).is_some() {
        subtitle
            .split_once(':')
            .map(|(_, title)| title.trim().to_string())
            .unwrap_or_else(|| subtitle.to_string())
    } else {
        subtitle.to_string()
    };
    let seriesnum = extract_series_num(subtitle).or_else(|| {
        episode
            .get("slice_id")
            .and_then(Value::as_str)
            .and_then(extract_series_num)
    });
    let editorial_subtitle = episode
        .get("editorial_subtitle")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let episodenum = extract_episode_num(&episode_title)
        .or_else(|| extract_episode_num(editorial_subtitle))
        .or_else(|| leading_episode_number(editorial_subtitle));
    let description = episode
        .get("synopses")
        .and_then(|synopses| {
            synopses
                .get("small")
                .or_else(|| synopses.get("medium"))
                .or_else(|| synopses.get("large"))
        })
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let (available, expires) = episode
        .get("versions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|version| version.get("availability"))
        .fold(
            (String::new(), 0_i64),
            |(mut available, expires), availability| {
                let start = availability
                    .get("start")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !start.is_empty() && (available.is_empty() || start < available.as_str()) {
                    available = start.to_string();
                }
                let end = availability
                    .get("end")
                    .and_then(Value::as_str)
                    .and_then(parse_rfc3339_timestamp)
                    .unwrap_or_default();
                (available, expires.max(end))
            },
        );
    let channel = episode
        .get("master_brand")
        .and_then(|brand| brand.get("titles"))
        .and_then(|titles| titles.get("medium"))
        .and_then(Value::as_str)
        .filter(|title| !title.is_empty())
        .unwrap_or(&seed.channel)
        .to_string();
    let thumbnail = episode
        .get("images")
        .and_then(|images| images.get("standard"))
        .and_then(Value::as_str)
        .filter(|image| !image.is_empty())
        .map(|image| image.replace("{recipe}", "1920xn"))
        .unwrap_or_default();
    let mut programme = Programme {
        prog_type: seed.prog_type,
        name: episode
            .get("title")
            .and_then(Value::as_str)
            .filter(|title| !title.is_empty())
            .unwrap_or(&seed.name)
            .to_string(),
        episode: episode_title,
        seriesnum,
        episodenum,
        pid: pid.clone(),
        channel,
        available,
        expires,
        desc: description,
        web: format!("https://www.bbc.co.uk/iplayer/episode/{pid}"),
        ..Default::default()
    };
    programme.thumbnail = thumbnail;
    if let Some(versions) = episode.get("versions").and_then(Value::as_array) {
        for version in versions {
            let Some(vpid) = version.get("id").and_then(Value::as_str) else {
                continue;
            };
            let kind = version
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase();
            let version_name = match kind.as_str() {
                "audio-described" | "audio_described" => "audiodescribed".to_string(),
                "" | "original" | "editorial" | "technical-replacement" | "other" => {
                    "default".to_string()
                }
                _ => kind,
            };
            if version_name == "default" && !programme.verpids.contains_key("default") {
                programme
                    .verpids
                    .insert("default".to_string(), vpid.to_string());
            }
            programme
                .verpids
                .entry(version_name.clone())
                .or_insert_with(|| vpid.to_string());
            if !programme.versions.contains(&version_name) {
                programme.versions.push(version_name.clone());
            }
            if programme.duration == 0 {
                programme.duration = version
                    .get("duration")
                    .and_then(|duration| duration.get("value"))
                    .and_then(Value::as_str)
                    .and_then(parse_iso_duration_seconds)
                    .unwrap_or_default();
            }
        }
    }

    Some(programme)
}

fn populate_from_ibl_episode(programme: &mut Programme, episode: &Value) {
    if let Some(metadata) = programme_from_ibl_episode(episode, programme) {
        *programme = metadata;
    }
}

fn parse_iso_duration_seconds(value: &str) -> Option<u32> {
    let re = Regex::new(r"^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?$").ok()?;
    let captures = re.captures(value)?;
    let hours = captures
        .get(1)
        .and_then(|value| value.as_str().parse::<u32>().ok())
        .unwrap_or_default();
    let minutes = captures
        .get(2)
        .and_then(|value| value.as_str().parse::<u32>().ok())
        .unwrap_or_default();
    let seconds = captures
        .get(3)
        .and_then(|value| value.as_str().parse::<f32>().ok())
        .unwrap_or_default() as u32;
    Some(hours * 3600 + minutes * 60 + seconds)
}

fn leading_episode_number(text: &str) -> Option<u32> {
    let re = Regex::new(r"^\s*(\d+)(?:/\d+)?(?:[.:\s]|$)").ok()?;
    re.captures(text)?.get(1)?.as_str().parse().ok()
}

fn parse_rfc3339_timestamp(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|date| date.timestamp())
}

fn has_next_ibl_page(page: usize, count: usize, page_size: usize) -> bool {
    count > 0 && page.saturating_mul(page_size.max(1)) < count
}

#[cfg(test)]
mod ibl_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_ibl_brand_and_available_episode_metadata() {
        let detail = json!({
            "episodes": [{ "id": "m0030fbn", "tleo_id": "b006mw70" }]
        });
        assert_eq!(ibl_brand_pid(&detail), Some("b006mw70"));

        let response = json!({
            "programme_episodes": {
                "page": 1,
                "per_page": 100,
                "count": 2,
                "elements": [
                    {
                        "id": "m0030fbn",
                        "title": "Not Going Out",
                        "subtitle": "Series 15: 5. Dream Catcher",
                        "slice_subtitle": "5. Dream Catcher",
                        "editorial_subtitle": "5/6 Dream Catcher",
                        "synopses": { "small": "Lee helps Lucy at a craft fair." },
                        "master_brand": { "titles": { "medium": "BBC One" } },
                        "versions": [
                            {
                                "id": "vpid-default",
                                "kind": "original",
                                "duration": { "value": "PT27M30.400S" },
                                "availability": {
                                    "start": "2026-09-23T05:00:00Z",
                                    "end": "2030-12-30T06:00:00Z"
                                }
                            },
                            {
                                "id": "vpid-ad",
                                "kind": "audio-described",
                                "availability": {
                                    "start": "2026-09-23T05:00:00Z",
                                    "end": "2030-12-30T06:00:00Z"
                                }
                            }
                        ]
                    },
                    {
                        "id": "m002d2yy",
                        "title": "Not Going Out",
                        "subtitle": "Series 14: 5. Dragon Castle",
                        "slice_subtitle": "5. Dragon Castle",
                        "editorial_subtitle": "5/6 Dragon Castle",
                        "versions": []
                    }
                ]
            }
        });
        let seed = Programme {
            prog_type: ProgrammeType::Tv,
            name: "Not Going Out".to_string(),
            channel: "BBC One".to_string(),
            ..Default::default()
        };

        let (count, page_size, episodes) = parse_ibl_episode_page(&response, &seed);
        assert_eq!((count, page_size), (2, 100));
        assert_eq!(episodes.len(), 2);
        let series_14 = episodes
            .iter()
            .find(|episode| episode.pid == "m002d2yy")
            .unwrap();
        assert_eq!(series_14.seriesnum, Some(14));
        assert_eq!(series_14.episodenum, Some(5));
        let series_15 = episodes
            .iter()
            .find(|episode| episode.pid == "m0030fbn")
            .unwrap();
        assert_eq!(series_15.seriesnum, Some(15));
        assert_eq!(series_15.episodenum, Some(5));
        assert_eq!(series_15.episode, "5. Dream Catcher");
        assert_eq!(series_15.desc, "Lee helps Lucy at a craft fair.");
        assert_eq!(series_15.channel, "BBC One");
        assert_eq!(series_15.expires, 1_924_840_800);
        assert_eq!(series_15.duration, 1_650);
        assert_eq!(series_15.verpids["default"], "vpid-default");
        assert_eq!(series_15.verpids["audiodescribed"], "vpid-ad");
    }

    #[test]
    fn parses_editorial_episode_fraction_when_numbered_title_is_missing() {
        assert_eq!(leading_episode_number("6/6 Banksy"), Some(6));
        assert_eq!(leading_episode_number("Series 15: Banksy"), None);
    }

    #[test]
    fn follows_ibl_pagination_until_the_reported_count_is_reached() {
        assert!(has_next_ibl_page(1, 113, 100));
        assert!(!has_next_ibl_page(2, 113, 100));
        assert!(!has_next_ibl_page(1, 0, 100));
        assert!(has_next_ibl_page(1, 2, 0));
    }
}
