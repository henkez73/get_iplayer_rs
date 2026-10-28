#![allow(dead_code, unused_variables, unused_assignments)]

mod bbc_api;
mod cache;
mod channels;
mod cli;
mod config;
mod downloader;
mod history;
mod indexer;
mod mediaselector;
mod models;
mod postproc;
mod pvr;
mod search;
mod subtitles;
mod tagger;
mod utils;

use bbc_api::BbcApi;
use cache::Cache;
use clap::Parser;
use cli::Cli;
use config::Config;
use downloader::Downloader;
use history::History;
use indexer::Indexer;
use mediaselector::Mediaselector;
use models::{Programme, ProgrammeType, Quality, StreamConnection};
use postproc::{OutputPathOptions, PostProcessor};
use pvr::PvrManager;
use search::{SearchFilter, Searcher};
use std::path::PathBuf;
use tagger::Tagger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut cli = Cli::parse();

    // Initialize Config
    let mut config = Config::new(cli.profile_dir.as_deref());

    // Handle Preferences actions
    if cli.prefs_show {
        config.show_prefs();
        return Ok(());
    }
    if cli.prefs_clear {
        config.prefs_clear();
        config.save_options()?;
        println!("INFO: Cleared saved preferences");
        return Ok(());
    }
    if cli.prefs_add {
        if let Some(ref q) = cli.tv_quality {
            config.prefs_add("tvquality", q);
        }
        if let Some(ref q) = cli.radio_quality {
            config.prefs_add("radioquality", q);
        }
        if let Some(ref o) = cli.output_dir {
            config.prefs_add("output", o);
        }
        if cli.subtitles {
            config.prefs_add("subtitles", "1");
        }
        if cli.subs_embed {
            config.prefs_add("subsembed", "1");
        }
        if cli.thumb {
            config.prefs_add("thumb", "1");
        }
        if cli.whitespace {
            config.prefs_add("whitespace", "1");
        }
        config.save_options()?;
        println!("INFO: Saved preferences updated");
        return Ok(());
    }
    if cli.prefs_del {
        for term in &cli.search_terms {
            config.prefs_del(term);
        }
        config.save_options()?;
        println!("INFO: Deleted specified preferences");
        return Ok(());
    }

    // Merge saved options into cli if not set
    if cli.output_dir.is_none() {
        if let Some(opt_out) = config.options.get("output") {
            cli.output_dir = Some(opt_out.clone());
        }
    }
    if cli.tv_quality.is_none() {
        if let Some(opt_tq) = config.options.get("tvquality") {
            cli.tv_quality = Some(opt_tq.clone());
        }
    }
    if cli.radio_quality.is_none() {
        if let Some(opt_rq) = config.options.get("radioquality") {
            cli.radio_quality = Some(opt_rq.clone());
        }
    }
    if !cli.subtitles && config.options.contains_key("subtitles") {
        cli.subtitles = true;
    }
    if !cli.subs_embed && config.options.contains_key("subsembed") {
        cli.subs_embed = true;
    }
    if !cli.whitespace && config.options.contains_key("whitespace") {
        cli.whitespace = true;
    }

    // Setup helpers
    let bbc_api = BbcApi::new();
    let mediaselector = Mediaselector::new();
    let downloader = Downloader::new(if cli.no_proxy { None } else { cli.proxy.as_deref() });
    let mut history = History::new(config.history_file());
    let postproc = PostProcessor::new(cli.ffmpeg.as_deref(), None);
    let tagger = Tagger::new(cli.atomicparsley.as_deref(), &postproc.ffmpeg_path);
    let pvr_mgr = PvrManager::new(config.pvr_dir());
    let download_services = DownloadServices {
        cli: &cli,
        bbc_api: &bbc_api,
        mediaselector: &mediaselector,
        downloader: &downloader,
        postproc: &postproc,
        tagger: &tagger,
    };

    // PVR Management Commands
    if cli.pvr_list {
        pvr_mgr.display_list();
        return Ok(());
    }
    if let Some(ref name) = cli.pvr_add {
        let mut opts = std::collections::HashMap::new();
        if !cli.search_terms.is_empty() {
            opts.insert("search".to_string(), cli.search_terms.join(" "));
        }
        opts.insert("type".to_string(), cli.prog_type.clone());
        if let Some(ref q) = cli.tv_quality {
            opts.insert("tvquality".to_string(), q.clone());
        }
        if cli.subtitles {
            opts.insert("subtitles".to_string(), "1".to_string());
        }
        pvr_mgr.add_job(name, &opts)?;
        return Ok(());
    }
    if let Some(ref name) = cli.pvr_del {
        pvr_mgr.delete_job(name)?;
        return Ok(());
    }
    if let Some(ref name) = cli.pvr_enable {
        pvr_mgr.enable_job(name)?;
        return Ok(());
    }
    if let Some(ref name) = cli.pvr_disable {
        pvr_mgr.disable_job(name)?;
        return Ok(());
    }

    // Cache paths
    let tv_cache_file = config.cache_file("tv");
    let radio_cache_file = config.cache_file("radio");

    if cli.show_cache_age {
        println!("Cache age:");
        if tv_cache_file.exists() {
            if let Ok(meta) = std::fs::metadata(&tv_cache_file) {
                if let Ok(mtime) = meta.modified() {
                    let elapsed = mtime.elapsed().unwrap_or_default().as_secs();
                    println!("  TV: {} seconds ago", elapsed);
                }
            }
        } else {
            println!("  TV: (no cache file)");
        }
        if radio_cache_file.exists() {
            if let Ok(meta) = std::fs::metadata(&radio_cache_file) {
                if let Ok(mtime) = meta.modified() {
                    let elapsed = mtime.elapsed().unwrap_or_default().as_secs();
                    println!("  Radio: {} seconds ago", elapsed);
                }
            }
        } else {
            println!("  Radio: (no cache file)");
        }
        return Ok(());
    }

    // Refresh Cache if requested
    let mut tv_cache = Cache::load_file(&tv_cache_file, ProgrammeType::Tv);
    let mut radio_cache = Cache::load_file(&radio_cache_file, ProgrammeType::Radio);

    if cli.refresh || cli.cache_rebuild {
        let indexer = Indexer::new(7);
        let days = cli.refresh_limit.unwrap_or(if cli.cache_rebuild { 30 } else { 7 });

        let prog_type: ProgrammeType = cli.prog_type.parse().unwrap_or(ProgrammeType::Tv);
        if prog_type == ProgrammeType::Tv || prog_type == ProgrammeType::All {
            if cli.cache_rebuild {
                tv_cache = Cache::new();
            }
            indexer.refresh(&mut tv_cache, ProgrammeType::Tv, days, cli.refresh_future, cli.verbose).await?;
            tv_cache.save_file(&tv_cache_file)?;
        }
        if prog_type == ProgrammeType::Radio || prog_type == ProgrammeType::All {
            if cli.cache_rebuild {
                radio_cache = Cache::new();
            }
            indexer.refresh(&mut radio_cache, ProgrammeType::Radio, days, cli.refresh_future, cli.verbose).await?;
            radio_cache.save_file(&radio_cache_file)?;
        }

        if cli.search_terms.is_empty() && cli.pids.is_empty() && cli.urls.is_empty() && !cli.pvr {
            return Ok(());
        }
    }

    // Handle History display
    if cli.history {
        history.list(&cli.search_terms);
        return Ok(());
    }

    // Direct PID / URL Recording
    let mut target_pids = Vec::new();
    for p in &cli.pids {
        if let Some(pid) = BbcApi::parse_pid(p) {
            target_pids.push(pid);
        }
    }
    for u in &cli.urls {
        if let Some(pid) = BbcApi::parse_pid(u) {
            target_pids.push(pid);
        }
    }

    // Check if first search term is a URL or pid:
    if target_pids.is_empty() && !cli.search_terms.is_empty() {
        if let Some(pid) = BbcApi::parse_pid(&cli.search_terms[0]) {
            target_pids.push(pid);
        }
    }

    if !target_pids.is_empty() {
        let mut final_pids = Vec::new();
        for pid in target_pids {
            if cli.pid_recursive {
                println!("INFO: Recursively resolving episodes for PID {}", pid);
                if let Ok(rec_pids) = bbc_api.get_recursive_pids(&pid).await {
                    if rec_pids.is_empty() {
                        final_pids.push(pid);
                    } else {
                        println!("INFO: Found {} episodes", rec_pids.len());
                        final_pids.extend(rec_pids);
                    }
                } else {
                    final_pids.push(pid);
                }
            } else {
                final_pids.push(pid);
            }
        }

        for pid in final_pids {
            // Check cache first for known programme metadata
            let cached_prog = tv_cache.by_pid.get(&pid).map(|&idx| tv_cache.programmes[idx].clone())
                .or_else(|| radio_cache.by_pid.get(&pid).map(|&idx| radio_cache.programmes[idx].clone()));

            let mut prog = match cached_prog {
                Some(p) => p,
                None => {
                    println!("INFO: Fetching metadata for PID {}", pid);
                    bbc_api.get_programme_info(&pid).await?
                }
            };

            if cli.info {
                Searcher::display_info(&prog);
                continue;
            }

            if cli.mark_downloaded {
                println!("INFO: Marking downloaded: {} - {} ({})", prog.name, prog.episode, prog.pid);
                history.add(&prog, "marked", "", "default")?;
                continue;
            }

            download_programme(&mut prog, &download_services, &mut history).await?;
        }

        return Ok(());
    }

    // Handle PVR run
    if cli.pvr || cli.pvr_single.is_some() {
        let jobs = pvr_mgr.list_jobs();
        let target_job = cli.pvr_single.as_deref();

        for job in jobs {
            if let Some(target) = target_job {
                if job.name != target {
                    continue;
                }
            } else if !job.is_enabled {
                continue;
            }

            println!("INFO: Running PVR search '{}'...", job.name);
            let search_term = job.options.get("search").cloned().unwrap_or_else(|| ".*".to_string());
            let job_type_str = job.options.get("type").cloned().unwrap_or_else(|| "tv".to_string());
            let job_prog_type: ProgrammeType = job_type_str.parse().unwrap_or(ProgrammeType::Tv);

            let filter = SearchFilter {
                search_terms: vec![search_term],
                prog_types: vec![job_prog_type],
                hide_recorded: true,
                force: cli.force,
                ..Default::default()
            };

            let matches = Searcher::find_matches(&filter, &tv_cache, &radio_cache, &history);
            println!("INFO: Found {} new matching programmes for PVR search '{}'", matches.len(), job.name);

            for matched_prog in matches {
                let mut prog = matched_prog.clone();
                if let Err(e) =
                    download_programme(&mut prog, &download_services, &mut history).await
                {
                    eprintln!("ERROR: Failed downloading {}: {}", prog.pid, e);
                }
            }
        }
        return Ok(());
    }

    // Search and/or Record matching programmes from Index
    let prog_type: ProgrammeType = cli.prog_type.parse().unwrap_or(ProgrammeType::Tv);
    let filter = SearchFilter {
        search_terms: cli.search_terms.clone(),
        prog_types: vec![prog_type],
        long_search: cli.long_search,
        channel_filter: cli.channel.clone(),
        exclude_channel_filter: cli.exclude_channel.clone(),
        since_hours: cli.since,
        before_hours: cli.before,
        available_since_hours: cli.available_since,
        available_before_hours: cli.available_before,
        expires_after_hours: cli.expires_after,
        expires_before_hours: cli.expires_before,
        future: cli.future,
        hide_recorded: cli.hide,
        force: cli.force,
        limit_matches: cli.limit_matches,
        sort_field: cli.sort.clone(),
        sort_reverse: cli.sort_reverse,
        ..Default::default()
    };

    let matches = Searcher::find_matches(&filter, &tv_cache, &radio_cache, &history);

    // Distinct list elements (e.g. --list channel)
    if let Some(ref elem) = cli.list_elements {
        if elem.eq_ignore_ascii_case("channel") {
            Searcher::list_unique_channels(&matches);
            return Ok(());
        }
    }

    // PVR Series option
    if cli.pvr_series {
        let mut seen_series = std::collections::HashSet::new();
        for p in &matches {
            if seen_series.insert(p.name.clone()) {
                let job_name = format!("{}_{}", utils::sanitize_filename(&p.name, false), p.prog_type.as_str());
                let mut opts = std::collections::HashMap::new();
                opts.insert("search".to_string(), format!("^{}$", p.name));
                opts.insert("type".to_string(), p.prog_type.as_str().to_string());
                pvr_mgr.add_job(&job_name, &opts)?;
            }
        }
        return Ok(());
    }

    // Mark downloaded
    if cli.mark_downloaded {
        for p in &matches {
            println!("INFO: Mark downloaded {}: '{} - {} ({})'", p.prog_type.as_str(), p.name, p.episode, p.pid);
            if !cli.test {
                history.add(p, "marked", "", "default")?;
            }
        }
        return Ok(());
    }

    // Info only
    if cli.info {
        for p in &matches {
            Searcher::display_info(p);
        }
        return Ok(());
    }

    // If --get was not specified, just list results and exit
    if !cli.get {
        Searcher::display_results(&matches, cli.list_format.as_deref());
        return Ok(());
    }

    // Recording matching programmes
    println!("INFO: Found {} matching programmes to record", matches.len());
    for matched_prog in matches {
        let mut prog = matched_prog.clone();
        if !cli.force && history.check(&prog.pid) {
            println!("INFO: Already in history: {} - {} ({})", prog.name, prog.episode, prog.pid);
            continue;
        }

        if let Err(e) = download_programme(&mut prog, &download_services, &mut history).await {
            eprintln!("ERROR: Failed to download {}: {}", prog.pid, e);
        }
    }

    Ok(())
}

struct DownloadServices<'a> {
    cli: &'a Cli,
    bbc_api: &'a BbcApi,
    mediaselector: &'a Mediaselector,
    downloader: &'a Downloader,
    postproc: &'a PostProcessor,
    tagger: &'a Tagger,
}

/// Download and process single programme
async fn download_programme(
    prog: &mut Programme,
    services: &DownloadServices<'_>,
    history: &mut History,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = services.cli;
    let bbc_api = services.bbc_api;
    let mediaselector = services.mediaselector;
    let downloader = services.downloader;
    let postproc = services.postproc;
    let tagger = services.tagger;
    println!("INFO: Processing '{} - {}' ({})", prog.name, prog.episode, prog.pid);

    // 1. Resolve version PIDs
    if prog.verpids.is_empty() {
        let info = bbc_api.get_programme_info(&prog.pid).await?;
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
    let requested_versions: Vec<&str> = cli.versions.split(',').map(|s| s.trim()).collect();
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

    println!("INFO: Selected version: '{}' (vpid: {})", selected_version, selected_vpid);

    // 3. Resolve stream links from Mediaselector
    let resolved = mediaselector.get_streams(&selected_vpid, prog.prog_type).await?;

    if resolved.is_geoblocked {
        eprintln!("WARNING: Programme is geo-blocked (UK IP address required)");
        return Ok(());
    }
    if resolved.is_unavailable {
        eprintln!("WARNING: Programme stream is currently unavailable");
        return Ok(());
    }

    // 4. Determine output paths
    let default_output_dir = PathBuf::from(if prog.prog_type == ProgrammeType::Tv {
        cli.output_tv.as_deref().or(cli.output_dir.as_deref()).unwrap_or(".")
    } else {
        cli.output_radio.as_deref().or(cli.output_dir.as_deref()).unwrap_or(".")
    });

    let ext = if prog.prog_type == ProgrammeType::Tv && !cli.audio_only { "mp4" } else { "m4a" };
    let (target_dir, target_file) = PostProcessor::generate_paths(
        prog,
        &selected_version,
        ext,
        &OutputPathOptions {
            base_output_dir: &default_output_dir,
            use_subdir: cli.subdir,
            subdir_format: cli.subdir_format.as_deref(),
            file_prefix: cli.file_prefix.as_deref(),
            keep_whitespace: cli.whitespace,
        },
    );

    if !cli.test && !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
    }

    // 5. Handle Subtitles
    let mut subtitle_file: Option<PathBuf> = None;
    if (cli.subtitles || cli.subtitles_only || cli.subs_embed) && !resolved.subtitle_urls.is_empty() {
        let srt_path = target_file.with_extension("srt");
        println!("INFO: Downloading subtitles for {}", prog.pid);
        let sub_url = &resolved.subtitle_urls[0];
        if let Ok(xml) = downloader.get_text(sub_url).await {
            if cli.subs_raw {
                let raw_path = target_file.with_extension("ttml");
                let _ = std::fs::write(&raw_path, &xml);
            }
            if let Ok(srt_content) = subtitles::SubtitleConverter::ttml_to_srt(&xml, cli.subs_mono, cli.suboffset) {
                if std::fs::write(&srt_path, srt_content).is_ok() {
                    println!("INFO: Saved subtitles: {}", srt_path.display());
                    subtitle_file = Some(srt_path);
                }
            }
        }
    }

    if cli.subtitles_only {
        return Ok(());
    }

    // 6. Handle Thumbnail
    let mut thumb_path: Option<PathBuf> = None;
    if (cli.thumb || cli.thumb_only || !cli.no_tag) && !prog.thumbnail.is_empty() {
        let tp = target_file.with_extension("jpg");
        if downloader.download_file(&prog.thumbnail, &tp).await.is_ok() {
            thumb_path = Some(tp);
        }
    }

    if cli.thumb_only {
        println!("INFO: Thumbnail downloaded for {}", prog.pid);
        return Ok(());
    }

    // 7. Handle Metadata sidecar
    if let Some(ref meta_format) = cli.metadata {
        if meta_format.eq_ignore_ascii_case("json") {
            let meta_path = target_file.with_extension("json");
            Tagger::write_metadata_json(prog, &meta_path)?;
            println!("INFO: Wrote metadata: {}", meta_path.display());
        }
    }
    if cli.metadata_only {
        return Ok(());
    }

    // Check if target file already exists
    if target_file.exists() && !cli.overwrite {
        println!("INFO: Already exists: {}. Use --overwrite to re-download", target_file.display());
        return Ok(());
    }

    // 8. Select best stream based on quality preferences
    if resolved.streams.is_empty() {
        eprintln!("ERROR: No streams found for {} (version {})", prog.pid, selected_version);
        return Ok(());
    }

    let quality_pref = if prog.prog_type == ProgrammeType::Tv {
        cli.tv_quality.as_deref().or(cli.quality.as_deref()).unwrap_or("hd,sd,web,mobile")
    } else {
        cli.radio_quality.as_deref().or(cli.quality.as_deref()).unwrap_or("high,std,med,low")
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
    println!(
        "INFO: Selected stream: {} {} ({} kbps, {}/{})",
        stream.transfer_format.to_uppercase(),
        stream.quality.as_str().to_uppercase(),
        stream.bitrate,
        stream.supplier,
        stream.service
    );

    if cli.test {
        println!("INFO: Test mode - would record to: {}", target_file.display());
        return Ok(());
    }

    // 9. Download media
    let tmp_ts = target_file.with_extension("tmp.ts");
    if stream.transfer_format == "hls" {
        println!("INFO: Downloading HLS stream to {}", tmp_ts.display());
        downloader.hls.download(&stream.href, &tmp_ts, Some(&stream.quality), cli.tv_lower_bitrate, false).await?;

        if cli.raw {
            std::fs::rename(&tmp_ts, &target_file)?;
            println!("INFO: Saved raw stream: {}", target_file.display());
        } else {
            println!("INFO: Remuxing to {}", target_file.display());
            postproc.remux(
                Some(&tmp_ts),
                None,
                subtitle_file.as_deref(),
                &target_file,
                cli.subs_embed,
            )?;
            let _ = std::fs::remove_file(&tmp_ts);
        }
    } else {
        // DASH stream
        println!("INFO: Downloading DASH manifest from {}", stream.href);
        let (v_track, a_track) = downloader.dash.parse_manifest(&stream.href).await?;
        let tmp_v = target_file.with_extension("tmp.m4v");
        let tmp_a = target_file.with_extension("tmp.m4a");

        if let Some(ref v) = v_track {
            downloader.dash.download_track(v, &tmp_v).await?;
        }
        if let Some(ref a) = a_track {
            downloader.dash.download_track(a, &tmp_a).await?;
        }

        if cli.raw {
            println!("INFO: Saved raw DASH tracks");
        } else {
            postproc.remux(
                if a_track.is_some() { Some(&tmp_a) } else { None },
                if v_track.is_some() { Some(&tmp_v) } else { None },
                subtitle_file.as_deref(),
                &target_file,
                cli.subs_embed,
            )?;
            let _ = std::fs::remove_file(&tmp_v);
            let _ = std::fs::remove_file(&tmp_a);
        }
    }

    // 10. Tag file
    if !cli.no_tag && target_file.is_file() {
        println!("INFO: Tagging {}", target_file.display());
        let _ = tagger.tag_file(&target_file, prog, thumb_path.as_deref(), cli.tag_podcast);
    }

    // Clean up temporary thumbnail if user did not request --thumb
    if !cli.thumb {
        if let Some(ref tp) = thumb_path {
            let _ = std::fs::remove_file(tp);
        }
    }

    // 11. Run User Command
    if let Some(ref cmd_tmpl) = cli.command {
        PostProcessor::run_user_command(cmd_tmpl, prog, &selected_version, &target_file);
    }

    // 12. Add to download history
    history.add(
        prog,
        stream.quality.as_str(),
        &target_file.display().to_string(),
        &selected_version,
    )?;

    println!("INFO: Successfully recorded '{}' to {}", prog.name, target_file.display());
    Ok(())
}
