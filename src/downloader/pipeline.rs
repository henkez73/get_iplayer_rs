//! Download pipeline and execution logic for BBC programmes.

use crate::bbc_api::BbcApi;
use crate::downloader::Downloader;
use crate::history::History;
use crate::mediaselector::Mediaselector;
use crate::models::{Programme, ProgrammeType, Quality, StreamConnection};
use crate::postproc::{OutputPathOptions, PostProcessor};
use crate::subtitles::SubtitleConverter;
use crate::tagger::Tagger;
use std::path::PathBuf;
use std::sync::Arc;

/// Progress stages during programme downloading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStage {
    ResolvingMetadata,
    ResolvingStreams,
    DownloadingSubtitles,
    DownloadingThumbnail,
    DownloadingMedia,
    Remuxing,
    Tagging,
    Completed,
    Failed,
}

impl DownloadStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadStage::ResolvingMetadata => "Resolving metadata",
            DownloadStage::ResolvingStreams => "Resolving streams",
            DownloadStage::DownloadingSubtitles => "Downloading subtitles",
            DownloadStage::DownloadingThumbnail => "Downloading thumbnail",
            DownloadStage::DownloadingMedia => "Downloading media",
            DownloadStage::Remuxing => "Remuxing",
            DownloadStage::Tagging => "Tagging metadata",
            DownloadStage::Completed => "Completed",
            DownloadStage::Failed => "Failed",
        }
    }
}

/// Download progress update sent to listeners.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub stage: DownloadStage,
    pub percent: f32, // 0.0 to 100.0
    pub message: String,
}

/// Options configuring programme download and post-processing.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub output_dir: Option<PathBuf>,
    pub output_tv: Option<PathBuf>,
    pub output_radio: Option<PathBuf>,
    pub tv_quality: Option<String>,
    pub radio_quality: Option<String>,
    pub quality: Option<String>,
    pub versions: String,
    pub subtitles: bool,
    pub subs_embed: bool,
    pub subs_raw: bool,
    pub subs_mono: bool,
    pub suboffset: i64,
    pub subtitles_only: bool,
    pub thumb: bool,
    pub thumb_only: bool,
    pub no_tag: bool,
    pub tag_podcast: bool,
    pub whitespace: bool,
    pub subdir: bool,
    pub subdir_format: Option<String>,
    pub file_prefix: Option<String>,
    pub overwrite: bool,
    pub raw: bool,
    pub audio_only: bool,
    pub test: bool,
    pub tv_lower_bitrate: bool,
    pub command: Option<String>,
    pub metadata: Option<String>,
    pub metadata_only: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            output_dir: None,
            output_tv: None,
            output_radio: None,
            tv_quality: None,
            radio_quality: None,
            quality: None,
            versions: "default".to_string(),
            subtitles: false,
            subs_embed: false,
            subs_raw: false,
            subs_mono: false,
            suboffset: 0,
            subtitles_only: false,
            thumb: true,
            thumb_only: false,
            no_tag: false,
            tag_podcast: false,
            whitespace: false,
            subdir: false,
            subdir_format: None,
            file_prefix: None,
            overwrite: false,
            raw: false,
            audio_only: false,
            test: false,
            tv_lower_bitrate: false,
            command: None,
            metadata: None,
            metadata_only: false,
        }
    }
}

/// Services required to download and process programmes.
pub struct DownloadServices<'a> {
    pub bbc_api: &'a BbcApi,
    pub mediaselector: &'a Mediaselector,
    pub downloader: &'a Downloader,
    pub postproc: &'a PostProcessor,
    pub tagger: &'a Tagger,
}

/// Executes the full download and post-processing pipeline for a single programme.
pub async fn download_programme(
    prog: &mut Programme,
    options: &DownloadOptions,
    services: &DownloadServices<'_>,
    history: &mut History,
    progress_callback: Option<Arc<dyn Fn(DownloadProgress) + Send + Sync>>,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let notify = |stage: DownloadStage, percent: f32, message: &str| {
        if let Some(ref cb) = progress_callback {
            cb(DownloadProgress {
                stage,
                percent,
                message: message.to_string(),
            });
        }
    };

    notify(
        DownloadStage::ResolvingMetadata,
        2.0,
        &format!(
            "Processing '{} - {}' ({})",
            prog.name, prog.episode, prog.pid
        ),
    );

    // 1. Resolve version PIDs
    if prog.verpids.is_empty() {
        let info = services.bbc_api.get_programme_info(&prog.pid).await?;
        prog.verpids = info.verpids;
        prog.versions = info.versions;
        if prog.thumbnail.is_empty() {
            prog.thumbnail = info.thumbnail;
        }
        if prog.duration == 0 {
            prog.duration = info.duration;
        }
        if prog.desc.is_empty() {
            prog.desc = info.desc;
        }
    }

    // 2. Select version
    notify(
        DownloadStage::ResolvingStreams,
        10.0,
        "Selecting version and resolving stream URLs...",
    );
    let requested_versions: Vec<&str> = options.versions.split(',').map(|s| s.trim()).collect();
    let mut selected_version = "default".to_string();
    let mut selected_vpid = prog.pid.clone();

    for req in requested_versions {
        if let Some(vpid) = prog.verpids.get(req) {
            selected_version = req.to_string();
            selected_vpid = vpid.clone();
            break;
        }
    }

    if let Some(vpid) = prog.verpids.get(&selected_version) {
        selected_vpid = vpid.clone();
    } else if let Some((first_ver, first_vpid)) = prog.verpids.iter().next() {
        selected_version = first_ver.clone();
        selected_vpid = first_vpid.clone();
    }

    // 3. Resolve stream links from Mediaselector
    let resolved = services
        .mediaselector
        .get_streams(&selected_vpid, prog.prog_type)
        .await?;

    if resolved.is_geoblocked {
        notify(
            DownloadStage::Failed,
            10.0,
            "Programme is geo-blocked (UK IP required)",
        );
        return Err("Programme is geo-blocked (UK IP address required)".into());
    }
    if resolved.is_unavailable {
        notify(DownloadStage::Failed, 10.0, "Stream currently unavailable");
        return Err("Programme stream is currently unavailable".into());
    }

    // 4. Determine output paths
    let dot_path = PathBuf::from(".");
    let default_output_dir = if prog.prog_type == ProgrammeType::Tv {
        options
            .output_tv
            .as_deref()
            .or(options.output_dir.as_deref())
            .unwrap_or(&dot_path)
            .to_path_buf()
    } else {
        options
            .output_radio
            .as_deref()
            .or(options.output_dir.as_deref())
            .unwrap_or(&dot_path)
            .to_path_buf()
    };

    let ext = if prog.prog_type == ProgrammeType::Tv && !options.audio_only {
        "mp4"
    } else {
        "m4a"
    };
    let (target_dir, target_file) = PostProcessor::generate_paths(
        prog,
        &selected_version,
        ext,
        &OutputPathOptions {
            base_output_dir: &default_output_dir,
            use_subdir: options.subdir,
            subdir_format: options.subdir_format.as_deref(),
            file_prefix: options.file_prefix.as_deref(),
            keep_whitespace: options.whitespace,
        },
    );

    if !options.test && !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
    }

    // 5. Handle Subtitles
    let mut subtitle_file: Option<PathBuf> = None;
    if (options.subtitles || options.subtitles_only || options.subs_embed)
        && !resolved.subtitle_urls.is_empty()
    {
        notify(
            DownloadStage::DownloadingSubtitles,
            15.0,
            "Downloading subtitles...",
        );
        let srt_path = target_file.with_extension("srt");
        let sub_url = &resolved.subtitle_urls[0];
        if let Ok(xml) = services.downloader.get_text(sub_url).await {
            if options.subs_raw {
                let raw_path = target_file.with_extension("ttml");
                let _ = std::fs::write(&raw_path, &xml);
            }
            if let Ok(srt_content) =
                SubtitleConverter::ttml_to_srt(&xml, options.subs_mono, options.suboffset)
            {
                if std::fs::write(&srt_path, srt_content).is_ok() {
                    subtitle_file = Some(srt_path);
                }
            }
        }
    }

    if options.subtitles_only {
        notify(
            DownloadStage::Completed,
            100.0,
            "Subtitles downloaded successfully",
        );
        return Ok(target_file);
    }

    // 6. Handle Thumbnail
    let mut thumb_path: Option<PathBuf> = None;
    if (options.thumb || options.thumb_only || !options.no_tag) && !prog.thumbnail.is_empty() {
        notify(
            DownloadStage::DownloadingThumbnail,
            20.0,
            "Downloading thumbnail...",
        );
        let tp = target_file.with_extension("jpg");
        if services
            .downloader
            .download_file(&prog.thumbnail, &tp)
            .await
            .is_ok()
        {
            thumb_path = Some(tp);
        }
    }

    if options.thumb_only {
        notify(
            DownloadStage::Completed,
            100.0,
            "Thumbnail downloaded successfully",
        );
        return Ok(target_file);
    }

    // 7. Handle Metadata sidecar
    if let Some(ref meta_format) = options.metadata {
        if meta_format.eq_ignore_ascii_case("json") {
            let meta_path = target_file.with_extension("json");
            Tagger::write_metadata_json(prog, &meta_path)?;
        }
    }
    if options.metadata_only {
        notify(
            DownloadStage::Completed,
            100.0,
            "Metadata saved successfully",
        );
        return Ok(target_file);
    }

    // Check if target file already exists
    if target_file.exists() && !options.overwrite {
        notify(
            DownloadStage::Completed,
            100.0,
            "Target file already exists (skipped)",
        );
        return Ok(target_file);
    }

    // 8. Select best stream based on quality preferences
    if resolved.streams.is_empty() {
        notify(DownloadStage::Failed, 20.0, "No streams found for version");
        return Err(format!(
            "No streams found for {} (version {})",
            prog.pid, selected_version
        )
        .into());
    }

    let quality_pref = if prog.prog_type == ProgrammeType::Tv {
        options
            .tv_quality
            .as_deref()
            .or(options.quality.as_deref())
            .unwrap_or("hd,sd,web,mobile")
    } else {
        options
            .radio_quality
            .as_deref()
            .or(options.quality.as_deref())
            .unwrap_or("high,std,med,low")
    };

    let target_qualities: Vec<Quality> = quality_pref
        .split(',')
        .filter_map(Quality::from_str_lenient)
        .collect();

    let mut chosen_stream: Option<&StreamConnection> = None;
    for q in &target_qualities {
        if let Some(s) = resolved.streams.iter().find(|s| &s.quality == q) {
            chosen_stream = Some(s);
            break;
        }
    }

    let stream = chosen_stream.unwrap_or(&resolved.streams[0]);

    if options.test {
        notify(
            DownloadStage::Completed,
            100.0,
            "Test mode complete (dry run)",
        );
        return Ok(target_file);
    }

    // 9. Download media
    notify(
        DownloadStage::DownloadingMedia,
        25.0,
        "Starting media stream download...",
    );
    let tmp_ts = target_file.with_extension("tmp.ts");

    if stream.transfer_format == "hls" {
        let cb_clone = progress_callback.clone();
        let progress_fn = move |cur: usize, tot: usize| {
            if let Some(ref cb) = cb_clone {
                let frac = if tot > 0 {
                    cur as f32 / tot as f32
                } else {
                    0.0
                };
                let percent = 25.0 + (frac * 60.0); // 25% -> 85%
                cb(DownloadProgress {
                    stage: DownloadStage::DownloadingMedia,
                    percent,
                    message: format!("Segment {}/{} ({:.1}%)", cur, tot, frac * 100.0),
                });
            }
        };

        services
            .downloader
            .hls
            .download_with_progress(
                &stream.href,
                &tmp_ts,
                Some(&stream.quality),
                options.tv_lower_bitrate,
                false,
                Some(&progress_fn),
            )
            .await?;

        if options.raw {
            std::fs::rename(&tmp_ts, &target_file)?;
        } else {
            notify(
                DownloadStage::Remuxing,
                87.0,
                "Remuxing media with FFmpeg...",
            );
            services.postproc.remux(
                Some(&tmp_ts),
                None,
                subtitle_file.as_deref(),
                &target_file,
                options.subs_embed,
            )?;
            let _ = std::fs::remove_file(&tmp_ts);
        }
    } else {
        // DASH stream
        notify(
            DownloadStage::DownloadingMedia,
            25.0,
            "Fetching DASH manifest...",
        );
        let (v_track, a_track) = services
            .downloader
            .dash
            .parse_manifest(&stream.href)
            .await?;
        let tmp_v = target_file.with_extension("tmp.m4v");
        let tmp_a = target_file.with_extension("tmp.m4a");

        if let Some(ref v) = v_track {
            let cb_clone = progress_callback.clone();
            let v_prog = move |cur: u64, tot: u64| {
                if let Some(ref cb) = cb_clone {
                    let frac = if tot > 0 {
                        cur as f32 / tot as f32
                    } else {
                        0.0
                    };
                    let percent = 25.0 + (frac * 35.0); // 25% -> 60%
                    cb(DownloadProgress {
                        stage: DownloadStage::DownloadingMedia,
                        percent,
                        message: format!("Video chunk {}/{} ({:.1}%)", cur, tot, frac * 100.0),
                    });
                }
            };
            services
                .downloader
                .dash
                .download_track_with_progress(v, &tmp_v, Some(&v_prog))
                .await?;
        }

        if let Some(ref a) = a_track {
            let cb_clone = progress_callback.clone();
            let a_prog = move |cur: u64, tot: u64| {
                if let Some(ref cb) = cb_clone {
                    let frac = if tot > 0 {
                        cur as f32 / tot as f32
                    } else {
                        0.0
                    };
                    let percent = 60.0 + (frac * 25.0); // 60% -> 85%
                    cb(DownloadProgress {
                        stage: DownloadStage::DownloadingMedia,
                        percent,
                        message: format!("Audio chunk {}/{} ({:.1}%)", cur, tot, frac * 100.0),
                    });
                }
            };
            services
                .downloader
                .dash
                .download_track_with_progress(a, &tmp_a, Some(&a_prog))
                .await?;
        }

        if options.raw {
            notify(DownloadStage::Completed, 100.0, "Saved raw DASH tracks");
        } else {
            notify(
                DownloadStage::Remuxing,
                87.0,
                "Remuxing video and audio tracks...",
            );
            services.postproc.remux(
                if a_track.is_some() {
                    Some(&tmp_a)
                } else {
                    None
                },
                if v_track.is_some() {
                    Some(&tmp_v)
                } else {
                    None
                },
                subtitle_file.as_deref(),
                &target_file,
                options.subs_embed,
            )?;
            let _ = std::fs::remove_file(&tmp_v);
            let _ = std::fs::remove_file(&tmp_a);
        }
    }

    // 10. Tag file
    if !options.no_tag && target_file.is_file() {
        notify(
            DownloadStage::Tagging,
            93.0,
            "Tagging file with metadata...",
        );
        let _ = services.tagger.tag_file(
            &target_file,
            prog,
            thumb_path.as_deref(),
            options.tag_podcast,
        );
    }

    // Clean up temporary thumbnail if user did not request thumbnail preservation
    if !options.thumb {
        if let Some(ref tp) = thumb_path {
            let _ = std::fs::remove_file(tp);
        }
    }

    // 11. Run User Command
    if let Some(ref cmd_tmpl) = options.command {
        PostProcessor::run_user_command(cmd_tmpl, prog, &selected_version, &target_file);
    }

    // 12. Add to download history
    history.add(
        prog,
        stream.quality.as_str(),
        &target_file.display().to_string(),
        &selected_version,
    )?;

    notify(
        DownloadStage::Completed,
        100.0,
        &format!("Successfully downloaded to {}", target_file.display()),
    );

    Ok(target_file)
}
