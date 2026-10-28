use indicatif::{ProgressBar, ProgressStyle};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct DashDownloader {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct DashTrack {
    pub mime_type: String,
    pub id: String,
    pub bandwidth: u64,
    pub init_url_template: String,
    pub media_url_template: String,
    pub start_number: u64,
    pub total_segments: u64,
    pub segment_duration: u64,
    pub timescale: u64,
}

impl DashDownloader {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Fetches and parses DASH MPD manifest
    pub async fn parse_manifest(&self, mpd_url: &str) -> Result<(Option<DashTrack>, Option<DashTrack>), Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.client.get(mpd_url).send().await?;
        let base_url = resp.url().clone();
        let content = resp.text().await?;

        let mut reader = Reader::from_str(&content);
        reader.config_mut().trim_text(true);

        let mut best_video: Option<DashTrack> = None;
        let mut best_audio: Option<DashTrack> = None;

        let mut in_adaptation = false;
        let mut current_mime = String::new();
        let mut template_init = String::new();
        let mut template_media = String::new();
        let mut start_number = 1u64;
        let mut timescale = 1000u64;
        let mut segment_duration = 0u64;
        let mut media_presentation_duration = 0f64;

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let name = e.name();
                    if name.as_ref() == "MPD" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == "mediaPresentationDuration" {
                                media_presentation_duration = Self::parse_iso_duration(attr.value.as_ref());
                            }
                        }
                    } else if name.as_ref() == "AdaptationSet" {
                        in_adaptation = true;
                        current_mime.clear();
                        template_init.clear();
                        template_media.clear();

                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == "mimeType" || attr.key.as_ref() == "contentType" {
                                current_mime = attr.value.as_ref().to_string();
                            }
                        }
                    } else if name.as_ref() == "SegmentTemplate" {
                        for attr in e.attributes().flatten() {
                            let k = attr.key.as_ref();
                            let v = attr.value.as_ref();
                            match k {
                                "initialization" => template_init = v.to_string(),
                                "media" => template_media = v.to_string(),
                                "startNumber" => start_number = v.parse().unwrap_or(1),
                                "timescale" => timescale = v.parse().unwrap_or(1000),
                                "duration" => segment_duration = v.parse().unwrap_or(0),
                                _ => {}
                            }
                        }
                    } else if name.as_ref() == "Representation" {
                        let mut rep_id = String::new();
                        let mut bandwidth = 0u64;
                        let mut rep_mime = current_mime.clone();

                        for attr in e.attributes().flatten() {
                            let k = attr.key.as_ref();
                            let v = attr.value.as_ref();
                            match k {
                                "id" => rep_id = v.to_string(),
                                "bandwidth" => bandwidth = v.parse().unwrap_or(0),
                                "mimeType" => rep_mime = v.to_string(),
                                _ => {}
                            }
                        }

                        let total_segments = if segment_duration > 0 && timescale > 0 && media_presentation_duration > 0.0 {
                            let seg_len_secs = segment_duration as f64 / timescale as f64;
                            (media_presentation_duration / seg_len_secs).ceil() as u64
                        } else {
                            50
                        };

                        let init_resolved = if template_init.starts_with("http") {
                            template_init.clone()
                        } else {
                            base_url.join(&template_init)?.to_string()
                        };

                        let media_resolved = if template_media.starts_with("http") {
                            template_media.clone()
                        } else {
                            base_url.join(&template_media)?.to_string()
                        };

                        let track = DashTrack {
                            mime_type: rep_mime.clone(),
                            id: rep_id,
                            bandwidth,
                            init_url_template: init_resolved,
                            media_url_template: media_resolved,
                            start_number,
                            total_segments,
                            segment_duration,
                            timescale,
                        };

                        if rep_mime.contains("video") {
                            if best_video.as_ref().map(|v| bandwidth > v.bandwidth).unwrap_or(true) {
                                best_video = Some(track);
                            }
                        } else if rep_mime.contains("audio")
                            && best_audio.as_ref().map(|a| bandwidth > a.bandwidth).unwrap_or(true)
                        {
                            best_audio = Some(track);
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == "AdaptationSet" {
                        in_adaptation = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        Ok((best_video, best_audio))
    }

    /// Downloads a single DASH track (init + segments) to a file
    pub async fn download_track(
        &self,
        track: &DashTrack,
        output_path: &Path,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let mut file = File::create(output_path)?;
        let mut downloaded_bytes = 0u64;

        // Download init segment
        let init_url = track.init_url_template.replace("$RepresentationID$", &track.id);
        if let Ok(resp) = self.client.get(&init_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    file.write_all(&bytes)?;
                    downloaded_bytes += bytes.len() as u64;
                }
            }
        }

        let pb = ProgressBar::new(track.total_segments);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} chunks ({percent}%)")
                .unwrap()
                .progress_chars("#>-"),
        );

        for seq in track.start_number..(track.start_number + track.total_segments) {
            let seg_url = track.media_url_template
                .replace("$RepresentationID$", &track.id)
                .replace("$Number$", &seq.to_string());

            let mut attempts = 0;
            let mut success = false;
            while attempts < 3 && !success {
                attempts += 1;
                match self.client.get(&seg_url).send().await {
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
                break;
            }
            pb.inc(1);
        }

        pb.finish_with_message("Done");
        Ok(downloaded_bytes)
    }

    /// Parse ISO8601 duration string like "PT1H23M45.67S"
    fn parse_iso_duration(s: &str) -> f64 {
        let mut total = 0.0;
        let mut num = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() || c == '.' {
                num.push(c);
            } else {
                let val: f64 = num.parse().unwrap_or(0.0);
                num.clear();
                match c {
                    'H' => total += val * 3600.0,
                    'M' => total += val * 60.0,
                    'S' => total += val,
                    _ => {}
                }
            }
        }
        total
    }
}
