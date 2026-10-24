use crate::cache::Cache;
use crate::channels::{self};
use crate::models::{Programme, ProgrammeType};
use crate::utils::{extract_episode_num, extract_series_num};
use chrono::{Datelike, Duration, Utc};
use futures_util::stream::{self, StreamExt};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Indexer {
    client: reqwest::Client,
    max_conn: usize,
}

impl Indexer {
    pub fn new(max_conn: usize) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(45))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client, max_conn }
    }

    /// Computes schedule dates in "YYYY/wWW" format
    pub fn get_schedule_dates(days: i64, future: bool) -> Vec<String> {
        let now = Utc::now();
        let mut dates = Vec::new();
        let mut seen = HashSet::new();

        if future {
            let start = now;
            let end = now + Duration::days(14);
            let mut curr = start;
            while curr <= end {
                let iso = curr.iso_week();
                let str_date = format!("{:04}/w{:02}", iso.year(), iso.week());
                if seen.insert(str_date.clone()) {
                    dates.push(str_date);
                }
                curr = curr + Duration::days(7);
            }
        } else {
            let start = now - Duration::days(days.min(30));
            let end = now + Duration::days(7);
            let mut curr = start;
            while curr <= end {
                let iso = curr.iso_week();
                let str_date = format!("{:04}/w{:02}", iso.year(), iso.week());
                if seen.insert(str_date.clone()) {
                    dates.push(str_date);
                }
                curr = curr + Duration::days(7);
            }
        }

        dates
    }

    /// Refreshes the cache for TV or Radio
    pub async fn refresh(
        &self,
        cache: &mut Cache,
        prog_type: ProgrammeType,
        limit_days: i64,
        future: bool,
        _verbose: bool,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let schedule_dates = Self::get_schedule_dates(limit_days, future);
        let channel_list = channels::get_channels(prog_type);

        println!(
            "INFO: Indexing {} programme schedules{}...",
            prog_type.as_str(),
            if future { " [future]" } else { "" }
        );

        let mut fetch_tasks = Vec::new();
        for ch in channel_list {
            for date_str in &schedule_dates {
                let url = format!("https://www.bbc.co.uk/schedules/{}/{}", ch.id, date_str);
                fetch_tasks.push((url, ch.name, ch.prog_type));
            }
        }

        let now = Utc::now().timestamp();
        let min_available = now - (30 * 86400);

        let discovered = Arc::new(Mutex::new(HashMap::<String, Programme>::new()));
        let total_urls = fetch_tasks.len();
        let progress_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        stream::iter(fetch_tasks)
            .map(|(url, channel_name, p_type)| {
                let client = self.client.clone();
                let discovered = Arc::clone(&discovered);
                let progress_counter = Arc::clone(&progress_counter);
                async move {
                    let res = client.get(&url).send().await;
                    let count = progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 20 == 0 || count == total_urls {
                        print!(".");
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }

                    if let Ok(resp) = res {
                        if resp.status().is_success() {
                            if let Ok(body) = resp.text().await {
                                let programmes = Self::parse_schedule_page(&body, channel_name, p_type, future, min_available, now);
                                if !programmes.is_empty() {
                                    let mut disc = discovered.lock().await;
                                    for p in programmes {
                                        disc.entry(p.pid.clone())
                                            .and_modify(|existing| {
                                                // Keep earliest availability
                                                if p.available < existing.available {
                                                    existing.available = p.available.clone();
                                                }
                                                if p.expires > existing.expires {
                                                    existing.expires = p.expires;
                                                }
                                            })
                                            .or_insert(p);
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .buffer_unordered(self.max_conn)
            .collect::<Vec<()>>()
            .await;

        println!();

        let disc = Arc::try_unwrap(discovered).unwrap().into_inner();
        let added_count = disc.len();

        // Merge into cache
        for (pid, new_prog) in disc {
            if let Some(&idx) = cache.by_pid.get(&pid) {
                // Update existing
                let existing = &mut cache.programmes[idx];
                existing.expires = new_prog.expires;
                if new_prog.available < existing.available {
                    existing.available = new_prog.available;
                }
            } else {
                let idx = cache.programmes.len();
                cache.by_pid.insert(pid, idx);
                cache.programmes.push(new_prog);
            }
        }

        // Sort and reindex
        cache.sort_and_reindex(prog_type.index_min());
        println!("INFO: Added {} {} programmes to cache", added_count, prog_type.as_str());

        Ok(added_count)
    }

    /// Parses schedule page looking for JSON-LD and HTML elements
    pub fn parse_schedule_page(
        html: &str,
        channel: &str,
        prog_type: ProgrammeType,
        future: bool,
        min_available: i64,
        now: i64,
    ) -> Vec<Programme> {
        let mut results = Vec::new();
        let cache_channel = channels::collapse_channel_name(channel, prog_type);

        // 1. Try parsing JSON-LD script tags
        let re_script = Regex::new(r#"(?s)<script\s+type="application/ld\+json">(\{.*?\})</script>"#).unwrap();
        for cap in re_script.captures_iter(html) {
            if let Some(json_str) = cap.get(1) {
                if let Ok(val) = serde_json::from_str::<Value>(json_str.as_str()) {
                    if let Some(graph) = val.get("@graph").and_then(|g| g.as_array()) {
                        for item in graph {
                            if let Some(p) = Self::parse_json_ld_entry(item, &cache_channel, prog_type, future, min_available, now) {
                                results.push(p);
                            }
                        }
                    }
                }
            }
        }

        if !results.is_empty() {
            return results;
        }

        // 2. Fallback: Parse HTML items
        let re_item = Regex::new(r#"(?s)<li[^>]*class="[^"]*week-guide__table__item[^"]*"[^>]*>(.*?)</li>"#).unwrap();
        let re_pid = Regex::new(r#"data-pid="([a-zA-Z0-9]{8,})""#).unwrap();
        let re_start = Regex::new(r#"class="[^"]*broadcast__time[^"]*"[^>]*content="([^"]+)""#).unwrap();
        let re_end = Regex::new(r#"property="endDate"[^>]*content="([^"]+)""#).unwrap();
        let re_title = Regex::new(r#"(?s)<span[^>]*class="programme__title[^"]*"[^>]*>(.*?)</span>"#).unwrap();
        let re_desc = Regex::new(r#"(?s)<p[^>]*class="programme__synopsis[^"]*"[^>]*>(.*?)</p>"#).unwrap();
        let re_strip_html = Regex::new(r#"<[^>]*>"#).unwrap();

        for cap in re_item.captures_iter(html) {
            let item_html = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let pid = match re_pid.captures(item_html).and_then(|c| c.get(1)) {
                Some(m) => m.as_str().to_string(),
                None => continue,
            };

            let start_str = re_start.captures(item_html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default();
            let end_str = re_end.captures(item_html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_default();

            let start_time = chrono::DateTime::parse_from_rfc3339(&start_str)
                .map(|dt| dt.timestamp())
                .unwrap_or(0);
            let end_time = chrono::DateTime::parse_from_rfc3339(&end_str)
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            if start_time < min_available {
                continue;
            }
            if !future && start_time > now {
                continue;
            }
            if future && start_time <= now {
                continue;
            }

            let duration = if end_time > start_time { (end_time - start_time) as u32 } else { 0 };
            let raw_title = re_title.captures(item_html).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("");
            let raw_desc = re_desc.captures(item_html).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("");

            let clean_title = htmlescape::decode_html(&re_strip_html.replace_all(raw_title, "")).unwrap_or_default().trim().to_string();
            let clean_desc = htmlescape::decode_html(&re_strip_html.replace_all(raw_desc, "")).unwrap_or_default().trim().to_string();

            let (name, episode) = if let Some((n, e)) = clean_title.split_once(':') {
                (n.trim().to_string(), e.trim().to_string())
            } else {
                (clean_title, "-".to_string())
            };

            let episodenum = extract_episode_num(&episode).or_else(|| extract_episode_num(&clean_desc));
            let seriesnum = extract_series_num(&format!("{} {}", name, episode));

            results.push(Programme {
                index: 0,
                prog_type,
                name,
                episode,
                seriesnum,
                episodenum,
                pid: pid.clone(),
                channel: cache_channel.clone(),
                available: start_str,
                expires: start_time + (30 * 86400),
                duration,
                desc: clean_desc,
                web: format!("https://www.bbc.co.uk/programmes/{}", pid),
                thumbnail: "https://ichef.bbci.co.uk/images/ic/1920xn/p01tqv8z.png".to_string(),
                timeadded: now,
                ..Default::default()
            });
        }

        results
    }

    fn parse_json_ld_entry(
        entry: &Value,
        channel: &str,
        prog_type: ProgrammeType,
        future: bool,
        min_available: i64,
        now: i64,
    ) -> Option<Programme> {
        let pid = entry.get("identifier")?.as_str()?.to_string();
        let pub_info = entry.get("publication")?;
        let start_str = pub_info.get("startDate")?.as_str()?.to_string();
        let end_str = pub_info.get("endDate").and_then(|e| e.as_str()).unwrap_or("");

        let start_time = chrono::DateTime::parse_from_rfc3339(&start_str)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);
        let end_time = chrono::DateTime::parse_from_rfc3339(end_str)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        if start_time < min_available {
            return None;
        }
        if !future && start_time > now {
            return None;
        }
        if future && start_time <= now {
            return None;
        }

        let duration = if end_time > start_time { (end_time - start_time) as u32 } else { 0 };

        let raw_name = entry.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let raw_desc = entry.get("description").and_then(|d| d.as_str()).unwrap_or("");

        let clean_name = htmlescape::decode_html(raw_name).unwrap_or_else(|_| raw_name.to_string());
        let clean_desc = htmlescape::decode_html(raw_desc).unwrap_or_else(|_| raw_desc.to_string());

        let (name, episode) = if let Some((n, e)) = clean_name.split_once(':') {
            (n.trim().to_string(), e.trim().to_string())
        } else {
            (clean_name, "-".to_string())
        };

        let episodenum = extract_episode_num(&episode).or_else(|| extract_episode_num(&clean_desc));
        let seriesnum = extract_series_num(&format!("{} {}", name, episode));

        Some(Programme {
            index: 0,
            prog_type,
            name,
            episode,
            seriesnum,
            episodenum,
            pid: pid.clone(),
            channel: channel.to_string(),
            available: start_str,
            expires: start_time + (30 * 86400),
            duration,
            desc: clean_desc,
            web: format!("https://www.bbc.co.uk/programmes/{}", pid),
            thumbnail: "https://ichef.bbci.co.uk/images/ic/1920xn/p01tqv8z.png".to_string(),
            timeadded: now,
            ..Default::default()
        })
    }
}
