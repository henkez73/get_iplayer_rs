use crate::cache::Cache;
use crate::history::History;
use crate::models::{Programme, ProgrammeType};
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub search_terms: Vec<String>,
    pub prog_types: Vec<ProgrammeType>,
    pub long_search: bool,
    pub channel_filter: Option<String>,
    pub exclude_channel_filter: Option<String>,
    pub category_filter: Option<String>,
    pub exclude_category_filter: Option<String>,
    pub since_hours: Option<i64>,
    pub before_hours: Option<i64>,
    pub available_since_hours: Option<i64>,
    pub available_before_hours: Option<i64>,
    pub expires_after_hours: Option<i64>,
    pub expires_before_hours: Option<i64>,
    pub future: bool,
    pub hide_recorded: bool,
    pub force: bool,
    pub limit_matches: Option<usize>,
    pub sort_field: Option<String>,
    pub sort_reverse: bool,
}

pub struct Searcher;

impl Searcher {
    pub fn find_matches<'a>(
        filter: &SearchFilter,
        tv_cache: &'a Cache,
        radio_cache: &'a Cache,
        history: &History,
    ) -> Vec<&'a Programme> {
        let mut candidates: Vec<&'a Programme> = Vec::new();

        let include_tv = filter.prog_types.is_empty()
            || filter.prog_types.contains(&ProgrammeType::Tv)
            || filter.prog_types.contains(&ProgrammeType::All);

        let include_radio = filter.prog_types.contains(&ProgrammeType::Radio)
            || filter.prog_types.contains(&ProgrammeType::All);

        if include_tv {
            candidates.extend(tv_cache.programmes.iter());
        }
        if include_radio {
            candidates.extend(radio_cache.programmes.iter());
        }

        let now = chrono::Utc::now().timestamp();

        // 1. Check if user passed specific index numbers or pid:<pid>
        let mut index_searches = Vec::new();
        let mut pid_searches = Vec::new();
        let mut regex_searches = Vec::new();

        for term in &filter.search_terms {
            let t = term.trim();
            if let Ok(idx) = t.parse::<usize>() {
                index_searches.push(idx);
            } else if let Some(stripped) = t.strip_prefix("pid:").or_else(|| t.strip_prefix("PID:")) {
                pid_searches.push(stripped.trim().to_string());
            } else if !t.is_empty() {
                // If the term is "*", convert to ".*"
                let pat = if t == "*" { ".*" } else { t };
                if let Ok(r) = Regex::new(&format!("(?i){}", pat)) {
                    regex_searches.push(r);
                }
            }
        }

        // Channel regexes
        let channel_re = filter.channel_filter.as_ref().and_then(|c| Regex::new(&format!("(?i){}", c)).ok());
        let exclude_channel_re = filter.exclude_channel_filter.as_ref().and_then(|c| Regex::new(&format!("(?i){}", c)).ok());

        let mut matched: Vec<&'a Programme> = candidates.into_iter().filter(|p| {
            // Index number match
            if !index_searches.is_empty() && index_searches.contains(&p.index) {
                return true;
            }

            // PID match
            if !pid_searches.is_empty() && pid_searches.iter().any(|pid| pid.eq_ignore_ascii_case(&p.pid)) {
                return true;
            }

            // If we have index or PID searches and NO regex searches, skip non-matches
            if (!index_searches.is_empty() || !pid_searches.is_empty()) && regex_searches.is_empty() {
                return false;
            }

            // Regex text matching
            if !regex_searches.is_empty() {
                let text_to_match = if filter.long_search {
                    format!("{} {} {}", p.name, p.episode, p.desc)
                } else {
                    format!("{} {}", p.name, p.episode)
                };
                if !regex_searches.iter().all(|r| r.is_match(&text_to_match)) {
                    return false;
                }
            } else if filter.search_terms.is_empty() {
                // If no search terms, default matches all unless specific filters apply
            }

            // Channel filter
            if let Some(ref re) = channel_re {
                if !re.is_match(&p.channel) {
                    return false;
                }
            }
            if let Some(ref re) = exclude_channel_re {
                if re.is_match(&p.channel) {
                    return false;
                }
            }

            // Time filters
            if !filter.future {
                let start_time = chrono::DateTime::parse_from_rfc3339(&p.available)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);
                if start_time > now {
                    return false;
                }
            }

            if let Some(since_h) = filter.since_hours {
                if p.timeadded < now - (since_h * 3600) {
                    return false;
                }
            }
            if let Some(before_h) = filter.before_hours {
                if p.timeadded >= now - (before_h * 3600) {
                    return false;
                }
            }

            if let Some(avail_since_h) = filter.available_since_hours {
                let start = chrono::DateTime::parse_from_rfc3339(&p.available).map(|dt| dt.timestamp()).unwrap_or(0);
                if start < now - (avail_since_h * 3600) {
                    return false;
                }
            }

            if let Some(avail_before_h) = filter.available_before_hours {
                let start = chrono::DateTime::parse_from_rfc3339(&p.available).map(|dt| dt.timestamp()).unwrap_or(0);
                if start >= now - (avail_before_h * 3600) {
                    return false;
                }
            }

            if let Some(exp_after_h) = filter.expires_after_hours {
                if p.expires <= now + (exp_after_h * 3600) {
                    return false;
                }
            }

            if let Some(exp_before_h) = filter.expires_before_hours {
                if p.expires > now + (exp_before_h * 3600) {
                    return false;
                }
            }

            // Hide already recorded
            if filter.hide_recorded && !filter.force && history.check(&p.pid) {
                return false;
            }

            true
        }).collect();

        // Sort
        if let Some(ref field) = filter.sort_field {
            match field.to_lowercase().as_str() {
                "name" => matched.sort_by(|a, b| a.name.cmp(&b.name)),
                "episode" => matched.sort_by(|a, b| a.episode.cmp(&b.episode)),
                "channel" => matched.sort_by(|a, b| a.channel.cmp(&b.channel)),
                "available" => matched.sort_by(|a, b| a.available.cmp(&b.available)),
                "duration" => matched.sort_by_key(|a| a.duration),
                _ => matched.sort_by_key(|p| p.index),
            }
        } else {
            matched.sort_by_key(|p| p.index);
        }

        if filter.sort_reverse {
            matched.reverse();
        }

        if let Some(limit) = filter.limit_matches {
            matched.truncate(limit);
        }

        matched
    }

    pub fn display_results(results: &[&Programme], custom_format: Option<&str>) {
        if results.is_empty() {
            println!("INFO: 0 Matching Programmes");
            return;
        }

        println!("Matches:");
        for p in results {
            if let Some(fmt) = custom_format {
                let vars = p.template_vars("default", "mp4");
                println!("{}", crate::utils::substitute_template(fmt, &vars));
            } else {
                let ep_part = if p.episode.is_empty() || p.episode == "-" {
                    "".to_string()
                } else {
                    format!(" - {}", p.episode)
                };
                println!("{}:  {}{}, {}, {}", p.index, p.name, ep_part, p.channel, p.pid);
            }
        }
        println!("INFO: {} Matching Programmes", results.len());
    }

    pub fn display_info(p: &Programme) {
        println!("--------------------------------------------------");
        println!("Matches:");
        let ep_part = if p.episode.is_empty() || p.episode == "-" {
            "".to_string()
        } else {
            format!(" - {}", p.episode)
        };
        println!("{}:  {}{}, {}, {}", p.index, p.name, ep_part, p.channel, p.pid);
        println!();
        println!("INFO: Metadata for {}", p.pid);
        println!("available:\t{}", p.available);
        println!("channel:\t{}", p.channel);
        println!("desc:\t\t{}", p.desc);
        println!("duration:\t{} ({})", p.duration, crate::utils::format_duration(p.duration));
        println!("episode:\t{}", p.episode);
        if let Some(ep_num) = p.episodenum {
            println!("episodenum:\t{}", ep_num);
        }
        if let Some(s_num) = p.seriesnum {
            println!("seriesnum:\t{}", s_num);
        }
        println!("name:\t\t{}", p.name);
        println!("pid:\t\t{}", p.pid);
        println!("thumbnail:\t{}", p.thumbnail);
        println!("type:\t\t{}", p.prog_type.as_str());
        println!("web:\t\t{}", p.web);
        println!("--------------------------------------------------");
    }

    pub fn list_unique_channels(results: &[&Programme]) {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for p in results {
            *counts.entry(&p.channel).or_insert(0) += 1;
        }
        let mut list: Vec<(&&str, &usize)> = counts.iter().collect();
        list.sort_by_key(|(ch, _)| **ch);
        println!("{:40} Count", "Channel");
        println!("{}", "-".repeat(50));
        for (ch, count) in list {
            println!("{:40} {}", ch, count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_search_filtering() {
        let mut tv_cache = Cache::new();
        tv_cache.programmes.push(Programme {
            index: 1,
            prog_type: ProgrammeType::Tv,
            name: "Doctor Who".to_string(),
            episode: "The Bells of Saint John".to_string(),
            pid: "b01rryzz".to_string(),
            channel: "BBC One".to_string(),
            available: "2023-01-01T00:00:00+00:00".to_string(),
            ..Default::default()
        });
        tv_cache.programmes.push(Programme {
            index: 2,
            prog_type: ProgrammeType::Tv,
            name: "EastEnders".to_string(),
            episode: "01/01/2023".to_string(),
            pid: "b01abcde".to_string(),
            channel: "BBC One".to_string(),
            available: "2023-01-01T00:00:00+00:00".to_string(),
            ..Default::default()
        });

        let radio_cache = Cache::new();
        let hist_file = NamedTempFile::new().unwrap();
        let history = History::new(hist_file.path());

        // Search for doctor who
        let filter = SearchFilter {
            search_terms: vec!["doctor who".to_string()],
            ..Default::default()
        };
        let matches = Searcher::find_matches(&filter, &tv_cache, &radio_cache, &history);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Doctor Who");

        // Search by channel
        let filter_chan = SearchFilter {
            channel_filter: Some("BBC One".to_string()),
            ..Default::default()
        };
        let matches_chan = Searcher::find_matches(&filter_chan, &tv_cache, &radio_cache, &history);
        assert_eq!(matches_chan.len(), 2);
    }
}
