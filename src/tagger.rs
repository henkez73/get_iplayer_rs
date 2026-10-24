use crate::models::{Programme, ProgrammeType};
use crate::utils::exists_in_path;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub struct Tagger {
    pub ap_path: Option<String>,
    pub ffmpeg_path: String,
}

impl Tagger {
    pub fn new(ap_override: Option<&str>, ffmpeg_path: &str) -> Self {
        let ap_path = if let Some(p) = ap_override {
            Some(p.to_string())
        } else if exists_in_path("AtomicParsley") {
            Some("AtomicParsley".to_string())
        } else if exists_in_path("atomicparsley") {
            Some("atomicparsley".to_string())
        } else {
            None
        };

        Self {
            ap_path,
            ffmpeg_path: ffmpeg_path.to_string(),
        }
    }

    /// Tags an MP4/M4A file with metadata and artwork
    pub fn tag_file(
        &self,
        media_file: &Path,
        prog: &Programme,
        thumb_file: Option<&Path>,
        tag_podcast: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref ap) = self.ap_path {
            self.tag_with_atomicparsley(ap, media_file, prog, thumb_file, tag_podcast)
        } else {
            self.tag_with_ffmpeg(media_file, prog, thumb_file)
        }
    }

    fn tag_with_atomicparsley(
        &self,
        ap: &str,
        media_file: &Path,
        prog: &Programme,
        thumb_file: Option<&Path>,
        tag_podcast: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cmd = Command::new(ap);
        cmd.arg(media_file);
        cmd.arg("--overWrite");

        let title = if prog.episode.is_empty() || prog.episode == "-" {
            &prog.name
        } else {
            &prog.episode
        };

        cmd.arg("--title").arg(title);
        cmd.arg("--artist").arg(&prog.channel);
        cmd.arg("--album").arg(&prog.name);
        cmd.arg("--albumArtist").arg(if prog.prog_type == ProgrammeType::Tv { "BBC TV" } else { "BBC Radio" });
        cmd.arg("--comment").arg(&prog.desc);
        cmd.arg("--longdesc").arg(format!("{}\n\nPID: {}\nURL: {}", prog.desc, prog.pid, prog.web));

        if let Some(ep) = prog.episodenum {
            cmd.arg("--tracknum").arg(ep.to_string());
        }
        if let Some(s) = prog.seriesnum {
            cmd.arg("--disk").arg(s.to_string());
        }

        if prog.prog_type == ProgrammeType::Tv {
            cmd.arg("--stik").arg("TV Show");
            cmd.arg("--TVShowName").arg(&prog.name);
            cmd.arg("--TVEpisode").arg(&prog.pid);
            cmd.arg("--TVNetwork").arg(&prog.channel);
            if let Some(s) = prog.seriesnum {
                cmd.arg("--TVSeasonNum").arg(s.to_string());
            }
            if let Some(ep) = prog.episodenum {
                cmd.arg("--TVEpisodeNum").arg(ep.to_string());
            }
        }

        if tag_podcast {
            cmd.arg("--podcastFlag").arg("true");
        }

        if let Some(thumb) = thumb_file {
            if thumb.is_file() {
                cmd.arg("--artwork").arg(thumb);
            }
        }

        let status = cmd.status()?;
        if !status.success() {
            return Err("AtomicParsley tagging failed".into());
        }

        Ok(())
    }

    fn tag_with_ffmpeg(
        &self,
        media_file: &Path,
        prog: &Programme,
        _thumb_file: Option<&Path>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tmp_output = media_file.with_extension("tagged.tmp.mp4");
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.arg("-y");
        cmd.arg("-loglevel").arg("error");
        cmd.arg("-i").arg(media_file);

        let title = if prog.episode.is_empty() || prog.episode == "-" {
            &prog.name
        } else {
            &prog.episode
        };

        cmd.arg("-metadata").arg(format!("title={}", title));
        cmd.arg("-metadata").arg(format!("artist={}", prog.channel));
        cmd.arg("-metadata").arg(format!("album={}", prog.name));
        cmd.arg("-metadata").arg(format!("comment={}", prog.desc));
        cmd.arg("-metadata").arg(format!("description={}", prog.desc));
        cmd.arg("-metadata").arg(format!("synopsis={}", prog.desc));
        if let Some(s) = prog.seriesnum {
            cmd.arg("-metadata").arg(format!("season_number={}", s));
        }
        if let Some(ep) = prog.episodenum {
            cmd.arg("-metadata").arg(format!("episode_sort={}", ep));
        }

        cmd.arg("-c").arg("copy");
        cmd.arg("-movflags").arg("faststart");
        cmd.arg(&tmp_output);

        let status = cmd.status()?;
        if status.success() {
            std::fs::rename(tmp_output, media_file)?;
        } else {
            let _ = std::fs::remove_file(tmp_output);
        }

        Ok(())
    }

    /// Writes sidecar metadata JSON file if requested by user
    pub fn write_metadata_json(
        prog: &Programme,
        dest: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json_str = serde_json::to_string_pretty(prog)?;
        let mut file = File::create(dest)?;
        file.write_all(json_str.as_bytes())?;
        Ok(())
    }
}
