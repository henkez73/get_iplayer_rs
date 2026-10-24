use crate::models::Quality;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub struct HlsDownloader {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HlsSegment {
    pub url: String,
    pub duration: f64,
}

#[derive(Debug, Clone)]
struct VariantStream {
    url: String,
    bandwidth: u64,
    width: u32,
    height: u32,
    fps: u32,
}

impl HlsDownloader {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Resolves master playlist down to the media playlist matching the requested quality
    pub async fn resolve_media_playlist(
        &self,
        master_url: &str,
        target_quality: Option<&Quality>,
        prefer_25fps: bool,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.client.get(master_url).send().await?;
        let final_url = resp.url().clone();
        let content = resp.text().await?;

        if !content.contains("#EXT-X-STREAM-INF") {
            // It is already a media playlist
            return Ok(final_url.to_string());
        }

        let re_bw = Regex::new(r#"BANDWIDTH=(\d+)"#).unwrap();
        let re_res = Regex::new(r#"RESOLUTION=(\d+)x(\d+)"#).unwrap();
        let re_fps = Regex::new(r#"FRAME-RATE=([\d\.]+)"#).unwrap();

        let mut variants: Vec<VariantStream> = Vec::new();
        let mut lines = content.lines();

        while let Some(line) = lines.next() {
            let line = line.trim();
            if line.starts_with("#EXT-X-STREAM-INF") {
                let bandwidth = re_bw.captures(line).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let (width, height) = if let Some(caps) = re_res.captures(line) {
                    let w = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                    let h = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                    (w, h)
                } else {
                    (0, 0)
                };
                let fps = re_fps.captures(line).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(25.0).round() as u32;

                if let Some(next_line) = lines.next() {
                    let next_trimmed = next_line.trim();
                    if !next_trimmed.is_empty() && !next_trimmed.starts_with('#') {
                        variants.push(VariantStream {
                            url: next_trimmed.to_string(),
                            bandwidth,
                            width,
                            height,
                            fps,
                        });
                    }
                }
            }
        }

        if variants.is_empty() {
            return Ok(final_url.to_string());
        }

        // Sort variants by bandwidth desc
        variants.sort_by_key(|v| std::cmp::Reverse(v.bandwidth));

        // Choose variant based on target_quality
        let chosen = if let Some(q) = target_quality {
            let matches_quality = |v: &VariantStream| -> bool {
                match q {
                    Quality::Fhd => v.height >= 1080,
                    Quality::Hd => v.height == 720,
                    Quality::Sd => v.height == 540,
                    Quality::Web => v.height == 396,
                    Quality::Mobile => v.height <= 288 && v.height > 0,
                    Quality::High | Quality::Std | Quality::Med | Quality::Low => true,
                }
            };

            let mut matched_variants: Vec<&VariantStream> = variants.iter().filter(|v| matches_quality(v)).collect();
            if prefer_25fps {
                matched_variants.sort_by_key(|v| if v.fps == 25 { 0 } else { 1 });
            }

            matched_variants.first().copied().unwrap_or(&variants[0])
        } else {
            &variants[0]
        };

        let mut resolved_url = if chosen.url.starts_with("http") {
            chosen.url.clone()
        } else {
            final_url.join(&chosen.url)?.to_string()
        };

        // If the variant url does not have query params, inherit from master url
        if !resolved_url.contains('?') {
            if let Some(q) = final_url.query() {
                resolved_url = format!("{}?{}", resolved_url, q);
            }
        }

        Ok(resolved_url)
    }

    /// Parses media playlist segments
    pub async fn parse_segments(&self, playlist_url: &str) -> Result<Vec<HlsSegment>, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.client.get(playlist_url).send().await?;
        let base_url = resp.url().clone();
        let content = resp.text().await?;

        let mut segments = Vec::new();
        let mut current_duration = 0.0;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("#EXTINF:") {
                let dur_str = line.trim_start_matches("#EXTINF:").split(',').next().unwrap_or("0");
                current_duration = dur_str.parse().unwrap_or(0.0);
            } else if !line.is_empty() && !line.starts_with('#') {
                let mut seg_url = if line.starts_with("http") {
                    line.to_string()
                } else {
                    base_url.join(line)?.to_string()
                };

                // Inherit token/query params if missing
                if !seg_url.contains('?') {
                    if let Some(q) = base_url.query() {
                        seg_url = format!("{}?{}", seg_url, q);
                    }
                }

                segments.push(HlsSegment {
                    url: seg_url,
                    duration: current_duration,
                });
                current_duration = 0.0;
            }
        }

        Ok(segments)
    }

    /// Downloads HLS segments and appends them to a target MPEG-TS file
    pub async fn download(
        &self,
        playlist_url: &str,
        output_ts: &Path,
        target_quality: Option<&Quality>,
        prefer_25fps: bool,
        resume: bool,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let media_url = self.resolve_media_playlist(playlist_url, target_quality, prefer_25fps).await?;
        let segments = self.parse_segments(&media_url).await?;

        if segments.is_empty() {
            return Err("No segments found in HLS playlist".into());
        }

        let total_segments = segments.len();
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(resume)
            .truncate(!resume)
            .open(output_ts)?;

        let pb = ProgressBar::new(total_segments as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} segments ({percent}%) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        let mut downloaded_bytes = 0u64;

        for (i, seg) in segments.iter().enumerate() {
            let mut attempts = 0;
            let mut success = false;

            while attempts < 3 && !success {
                attempts += 1;
                match self.client.get(&seg.url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(bytes) = resp.bytes().await {
                            file.write_all(&bytes)?;
                            downloaded_bytes += bytes.len() as u64;
                            success = true;
                        }
                    }
                    _ => {
                        tokio::time::sleep(std::time::Duration::from_millis(500 * attempts)).await;
                    }
                }
            }

            if !success {
                pb.finish_with_message("Download failed on segment");
                return Err(format!("Failed to download segment {}", i + 1).into());
            }

            pb.inc(1);
        }

        pb.finish_with_message("Done");
        Ok(downloaded_bytes)
    }
}
