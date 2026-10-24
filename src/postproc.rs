use crate::models::Programme;
use crate::utils::{sanitize_filename, substitute_template};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PostProcessor {
    pub ffmpeg_path: String,
    pub log_level: String,
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
        base_output_dir: &Path,
        use_subdir: bool,
        subdir_format: Option<&str>,
        file_prefix: Option<&str>,
        keep_whitespace: bool,
    ) -> (PathBuf, PathBuf) {
        let vars = prog.template_vars(version, ext);

        // Subdir
        let mut target_dir = base_output_dir.to_path_buf();
        if use_subdir {
            let sub_fmt = subdir_format.unwrap_or("<name>");
            let sub_name = substitute_template(sub_fmt, &vars);
            let clean_sub = sanitize_filename(&sub_name, keep_whitespace);
            target_dir = target_dir.join(clean_sub);
        }

        // File prefix
        let prefix_fmt = file_prefix.unwrap_or("<name> - <episode> <pid> <version>");
        let prefix = substitute_template(prefix_fmt, &vars);
        let clean_prefix = sanitize_filename(&prefix, keep_whitespace);

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
        let is_ts = audio_file.map(|p| p.extension().map(|e| e == "ts").unwrap_or(false)).unwrap_or(false)
            || video_file.map(|p| p.extension().map(|e| e == "ts").unwrap_or(false)).unwrap_or(false);

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
            let _ = Command::new("sh")
                .arg("-c")
                .arg(&command_str)
                .status();
        }

        #[cfg(target_family = "windows")]
        {
            let _ = Command::new("cmd")
                .arg("/C")
                .arg(&command_str)
                .status();
        }
    }
}
