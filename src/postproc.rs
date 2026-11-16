use crate::models::Programme;
use crate::utils::{sanitize_filename, substitute_template};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PostProcessor {
    pub ffmpeg_path: String,
    pub log_level: String,
}

/// Options controlling output path generation.
pub struct OutputPathOptions<'a> {
    pub base_output_dir: &'a Path,
    pub use_subdir: bool,
    pub subdir_format: Option<&'a str>,
    pub file_prefix: Option<&'a str>,
    pub keep_whitespace: bool,
}

impl PostProcessor {
    pub fn new(ffmpeg_override: Option<&str>, log_level: Option<&str>) -> Self {
        let ffmpeg_path = ffmpeg_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| "ffmpeg".to_string());
        let log_level = log_level
            .map(|s| s.to_string())
            .unwrap_or_else(|| "error".to_string());

        Self {
            ffmpeg_path,
            log_level,
        }
    }

    /// Determines output directory and filename based on user options and templates
    pub fn generate_paths(
        prog: &Programme,
        version: &str,
        ext: &str,
        options: &OutputPathOptions<'_>,
    ) -> (PathBuf, PathBuf) {
        let vars = prog.template_vars(version, ext);

        // Subdir
        let mut target_dir = options.base_output_dir.to_path_buf();
        if options.use_subdir {
            let sub_fmt = options.subdir_format.unwrap_or("<name>");
            let sub_name = substitute_template(sub_fmt, &vars);
            let clean_sub = sanitize_filename(&sub_name, options.keep_whitespace);
            target_dir = target_dir.join(clean_sub);
        }

        // File prefix
        let prefix = if let Some(file_prefix) = options.file_prefix {
            substitute_template(file_prefix, &vars)
        } else if let (Some(series), Some(episode)) = (prog.seriesnum, prog.episodenum) {
            format!("{}.S{series:02}E{episode:02}", prog.name)
        } else {
            substitute_template("<name> - <episode> <pid> <version>", &vars)
        };
        let clean_prefix = sanitize_filename(&prefix, options.keep_whitespace);

        let final_filename = format!("{}.{}", clean_prefix, ext);
        let final_path = target_dir.join(final_filename);

        (target_dir, final_path)
    }

    /// Remuxes TS or separate DASH files into final MP4/M4A with optional soft subtitles
    pub fn remux(
        &self,
        audio_file: Option<&Path>,
        video_file: Option<&Path>,
        subtitle_file: Option<&Path>,
        output_file: &Path,
        embed_subtitles: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.arg("-y");
        cmd.arg("-loglevel").arg(&self.log_level);

        let mut input_idx = 0;
        let mut audio_idx = None;
        let mut video_idx = None;
        let mut sub_idx = None;

        if let Some(audio) = audio_file {
            cmd.arg("-i").arg(audio);
            audio_idx = Some(input_idx);
            input_idx += 1;
        }

        if let Some(video) = video_file {
            cmd.arg("-i").arg(video);
            video_idx = Some(input_idx);
            input_idx += 1;
        }

        if embed_subtitles {
            if let Some(sub) = subtitle_file {
                if sub.is_file() {
                    cmd.arg("-i").arg(sub);
                    sub_idx = Some(input_idx);
                }
            }
        }

        // Codecs
        cmd.arg("-c:v").arg("copy");
        cmd.arg("-c:a").arg("copy");

        // Map streams if both audio and video provided
        if let (Some(a), Some(v)) = (audio_idx, video_idx) {
            cmd.arg("-map").arg(format!("{}:a:0", a));
            cmd.arg("-map").arg(format!("{}:v:0", v));
        }

        // Subtitles mapping
        if let Some(s) = sub_idx {
            cmd.arg("-map").arg(format!("{}:0", s));
            cmd.arg("-c:s").arg("mov_text");
            cmd.arg("-metadata:s:s:0").arg("language=eng");
        }

        // Bitstream filter for AAC in TS
        let is_ts = audio_file
            .map(|p| p.extension().map(|e| e == "ts").unwrap_or(false))
            .unwrap_or(false)
            || video_file
                .map(|p| p.extension().map(|e| e == "ts").unwrap_or(false))
                .unwrap_or(false);

        if is_ts {
            cmd.arg("-bsf:a").arg("aac_adtstoasc");
        }

        cmd.arg("-movflags").arg("faststart");
        cmd.arg(output_file);

        let status = cmd.status()?;
        if !status.success() {
            return Err(format!("ffmpeg remux failed with status {:?}", status.code()).into());
        }

        Ok(())
    }

    /// Runs custom user command specified by --command
    pub fn run_user_command(
        cmd_template: &str,
        prog: &Programme,
        version: &str,
        output_file: &Path,
    ) {
        let mut vars = prog.template_vars(version, "mp4");
        vars.insert("file", output_file.display().to_string());
        let command_str = substitute_template(cmd_template, &vars);

        #[cfg(target_family = "unix")]
        {
            let _ = Command::new("sh").arg("-c").arg(&command_str).status();
        }

        #[cfg(target_family = "windows")]
        {
            let _ = Command::new("cmd").arg("/C").arg(&command_str).status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Programme, ProgrammeType};

    #[test]
    fn defaults_to_show_and_padded_season_episode_for_numbered_programmes() {
        let programme = Programme {
            name: "Not Going Out".to_string(),
            seriesnum: Some(1),
            episodenum: Some(2),
            ..Default::default()
        };
        let options = OutputPathOptions {
            base_output_dir: Path::new("recordings"),
            use_subdir: false,
            subdir_format: None,
            file_prefix: None,
            keep_whitespace: false,
        };

        let (_, path) = PostProcessor::generate_paths(&programme, "default", "mp4", &options);
        assert_eq!(path, Path::new("recordings/Not_Going_Out.S01E02.mp4"));
        assert_eq!(
            path.with_extension("srt"),
            Path::new("recordings/Not_Going_Out.S01E02.srt")
        );
        assert_eq!(
            path.with_extension("jpg"),
            Path::new("recordings/Not_Going_Out.S01E02.jpg")
        );
    }

    #[test]
    fn keeps_custom_file_prefix_and_legacy_fallback_for_un_numbered_programmes() {
        let programme = Programme {
            prog_type: ProgrammeType::Tv,
            name: "A Show".to_string(),
            episode: "A Special".to_string(),
            pid: "b1234567".to_string(),
            ..Default::default()
        };
        let options = OutputPathOptions {
            base_output_dir: Path::new("recordings"),
            use_subdir: false,
            subdir_format: None,
            file_prefix: None,
            keep_whitespace: false,
        };
        let (_, path) = PostProcessor::generate_paths(&programme, "default", "mp4", &options);
        assert_eq!(
            path,
            Path::new("recordings/A_Show-A_Special_b1234567_default.mp4")
        );

        let custom_options = OutputPathOptions {
            file_prefix: Some("<name>.<pid>"),
            ..options
        };
        let (_, custom_path) =
            PostProcessor::generate_paths(&programme, "default", "mp4", &custom_options);
        assert_eq!(custom_path, Path::new("recordings/A_Show.b1234567.mp4"));
    }
}
