use crate::models::{ProgrammeType, Quality, StreamConnection};
use quick_xml::events::Event;
use quick_xml::reader::Reader;

pub struct Mediaselector {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct ResolvedStreams {
    pub streams: Vec<StreamConnection>,
    pub subtitle_urls: Vec<String>,
    pub is_geoblocked: bool,
    pub is_unavailable: bool,
}

impl Mediaselector {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client }
    }

    /// Fetches and parses stream manifests from Mediaselector API
    pub async fn get_streams(
        &self,
        vpid: &str,
        prog_type: ProgrammeType,
    ) -> Result<ResolvedStreams, Box<dyn std::error::Error + Send + Sync>> {
        let mut resolved = ResolvedStreams {
            streams: Vec::new(),
            subtitle_urls: Vec::new(),
            is_geoblocked: false,
            is_unavailable: false,
        };

        let mediasets = match prog_type {
            ProgrammeType::Tv => vec!["iptv-all", "pc"],
            ProgrammeType::Radio => vec!["iptv-all", "pc", "mobile-cellular-main"],
            ProgrammeType::All => vec!["iptv-all", "pc", "mobile-cellular-main"],
        };

        for &ms_ver in &[6, 5] {
            for &mediaset in &mediasets {
                let random_cb = chrono::Utc::now().timestamp_subsec_micros() % 90000 + 10000;
                let url = format!(
                    "https://open.live.bbc.co.uk/mediaselector/{}/select/version/2.0/mediaset/{}/vpid/{}/format/xml?cb={}",
                    ms_ver, mediaset, vpid, random_cb
                );

                if let Ok(resp) = self.client.get(&url).send().await {
                    if let Ok(xml) = resp.text().await {
                        if xml.contains("geolocation") || xml.contains("notukerror") {
                            resolved.is_geoblocked = true;
                            continue;
                        }
                        if xml.contains("selectionunavailable") {
                            resolved.is_unavailable = true;
                            continue;
                        }

                        let (streams, subs) = Self::parse_mediaselector_xml(&xml, prog_type);
                        if !streams.is_empty() {
                            resolved.streams.extend(streams);
                        }
                        if !subs.is_empty() {
                            resolved.subtitle_urls.extend(subs);
                        }
                    }
                }
            }

            if !resolved.streams.is_empty() {
                break;
            }
        }

        // Sort streams by quality desc, then priority asc, then bitrate desc
        resolved.streams.sort_by(|a, b| {
            b.quality.cmp(&a.quality)
                .then_with(|| a.priority.cmp(&b.priority))
                .then_with(|| b.bitrate.cmp(&a.bitrate))
        });

        // Deduplicate streams by href
        let mut seen_hrefs = std::collections::HashSet::new();
        resolved.streams.retain(|s| seen_hrefs.insert(s.href.clone()));
        let mut seen_subs = std::collections::HashSet::new();
        resolved.subtitle_urls.retain(|u| seen_subs.insert(u.clone()));

        Ok(resolved)
    }

    /// Parses Mediaselector XML response
    pub fn parse_mediaselector_xml(
        xml: &str,
        prog_type: ProgrammeType,
    ) -> (Vec<StreamConnection>, Vec<String>) {
        let mut streams = Vec::new();
        let mut subtitles = Vec::new();

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut current_media_kind = String::new();
        let mut current_media_bitrate: u32 = 0;
        let mut current_media_width: Option<u32> = None;
        let mut current_media_height: Option<u32> = None;
        let mut current_media_fps: Option<u32> = None;
        let mut current_media_service = String::new();
        let mut current_media_type = String::new();

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let name = e.name();
                    if name.as_ref() == "media" {
                        current_media_kind.clear();
                        current_media_bitrate = 0;
                        current_media_width = None;
                        current_media_height = None;
                        current_media_fps = None;
                        current_media_service.clear();
                        current_media_type.clear();

                        for attr in e.attributes().flatten() {
                            let key = attr.key.as_ref();
                            let val = attr.value.as_ref();
                            match key {
                                "kind" => current_media_kind = val.to_string(),
                                "bitrate" => current_media_bitrate = val.parse().unwrap_or(0),
                                "width" => current_media_width = val.parse().ok(),
                                "height" => current_media_height = val.parse().ok(),
                                "fps" => current_media_fps = val.parse().ok(),
                                "service" => current_media_service = val.to_string(),
                                "type" => current_media_type = val.to_string(),
                                _ => {}
                            }
                        }
                    } else if name.as_ref() == "connection" {
                        let mut href = String::new();
                        let mut transfer_format = String::new();
                        let mut supplier = String::new();
                        let mut protocol = String::new();
                        let mut priority: u32 = 10;

                        for attr in e.attributes().flatten() {
                            let key = attr.key.as_ref();
                            let val = attr.value.as_ref();
                            match key {
                                "href" => href = val.to_string(),
                                "transferFormat" => transfer_format = val.to_string(),
                                "supplier" => supplier = val.to_string(),
                                "protocol" => protocol = val.to_string(),
                                "priority" => priority = val.parse().unwrap_or(10),
                                _ => {}
                            }
                        }

                        if current_media_kind == "captions" || current_media_type == "application/ttaf+xml" {
                            if !href.is_empty() {
                                subtitles.push(href);
                            }
                        } else if !href.is_empty() && (transfer_format == "hls" || transfer_format == "dash") {
                            let quality = Self::determine_quality(
                                prog_type,
                                &current_media_kind,
                                current_media_height,
                                current_media_bitrate,
                            );

                            streams.push(StreamConnection {
                                href,
                                transfer_format,
                                supplier,
                                protocol,
                                priority,
                                kind: current_media_kind.clone(),
                                bitrate: current_media_bitrate,
                                width: current_media_width,
                                height: current_media_height,
                                fps: current_media_fps,
                                quality,
                                service: current_media_service.clone(),
                            });
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == "media" {
                        current_media_kind.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        (streams, subtitles)
    }

    fn determine_quality(
        prog_type: ProgrammeType,
        kind: &str,
        height: Option<u32>,
        bitrate: u32,
    ) -> Quality {
        if prog_type == ProgrammeType::Tv || kind == "video" {
            let h = height.unwrap_or(0);
            if h >= 1080 {
                Quality::Fhd
            } else if h >= 720 {
                Quality::Hd
            } else if h >= 500 {
                Quality::Sd
            } else if h >= 350 {
                Quality::Web
            } else {
                Quality::Mobile
            }
        } else {
            // Radio
            if bitrate >= 192 {
                Quality::High
            } else if bitrate >= 120 {
                Quality::Std
            } else if bitrate >= 80 {
                Quality::Med
            } else {
                Quality::Low
            }
        }
    }
}
