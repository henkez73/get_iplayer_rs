use eframe::egui;
use eframe::egui::{Color32, FontId, RichText, Ui};
use get_iplayer_common::{
    bbc_api::BbcApi,
    cache::Cache,
    config::Config,
    downloader::{
        pipeline::{DownloadOptions, DownloadProgress, DownloadServices, DownloadStage},
        Downloader,
    },
    history::History,
    indexer::Indexer,
    mediaselector::Mediaselector,
    models::{Programme, ProgrammeType},
    postproc::PostProcessor,
    search::{SearchFilter, Searcher},
    tagger::Tagger,
};
use regex::Regex;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};
use tokio::{runtime::Runtime, sync::Semaphore};

// ─── Tab selection ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tab {
    Search,
    Browse,
    Downloads,
    History,
    Settings,
    About,
}

// ─── Download queue item ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DownloadItem {
    prog: Programme,
    progress: f32,
    stage: String,
    status: DownloadStatus,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DownloadStatus {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Debug)]
struct SearchShowGroup {
    title: String,
    seasons: Vec<SearchSeasonGroup>,
}

#[derive(Debug)]
struct SearchSeasonGroup {
    label: String,
    episodes: Vec<Programme>,
}

#[derive(Debug)]
struct SearchShowBuilder {
    title: String,
    seasons: BTreeMap<Option<u32>, Vec<Programme>>,
}

#[derive(Debug, Clone)]
struct FilmShelf {
    title: String,
    films: Vec<Programme>,
}

const CACHE_MAX_AGE: Duration = Duration::from_secs(3 * 60 * 60);
const BBC_TERMS_URL: &str = "https://www.bbc.co.uk/terms";
const TV_LICENCE_URL: &str =
    "https://www.tvlicensing.co.uk/check-if-you-need-one/topics/bbc-iplayer-and-the-tv-licence2";
const BBC_ATTRIBUTION: &str = "BBC iPlayer and BBC Sounds are services of the BBC. This independent application is not affiliated with or endorsed by the BBC.";
const TV_LICENCE_NOTICE: &str = "In the UK, you must be covered by a TV Licence to watch or download BBC programmes on BBC iPlayer. See the official guidance for full details.";

// ─── App state ────────────────────────────────────────────────────────────────

pub struct MyApp {
    // Runtime
    rt: Runtime,

    // Core services (all wrapped for thread-safe sharing when needed)
    config: Config,

    // Cache state
    tv_cache: Cache,
    radio_cache: Cache,
    cache_loading: bool,
    cache_status: String,
    cache_refresh_rx: Option<mpsc::Receiver<Result<(Cache, Cache), String>>>,
    next_cache_refresh_check: Instant,
    archive_loading: bool,
    archive_refresh_rx: Option<mpsc::Receiver<Result<Vec<Programme>, String>>>,
    api_search_loading: bool,
    api_search_rx: Option<mpsc::Receiver<Result<Vec<Programme>, String>>>,

    // Quick download by URL or PID in Downloads tab
    quick_download_input: String,
    quick_download_loading: bool,
    quick_download_rx: Option<mpsc::Receiver<Result<Vec<Programme>, String>>>,

    // History
    history: History,

    // Search state
    search_query: String,
    search_prog_type: ProgrammeType,
    search_channel: String,
    search_hide_recorded: bool,
    search_newest_first: bool,
    search_results: Vec<Programme>,
    selected_prog: Option<Programme>,
    selected_episode_pids: HashSet<String>,

    // Category catalogue
    film_shelves: Vec<FilmShelf>,
    film_loading: bool,
    film_status: String,
    film_refresh_rx: Option<mpsc::Receiver<Result<Vec<FilmShelf>, String>>>,
    film_collection: String,
    selected_film_pids: HashSet<String>,

    // Downloads
    download_queue: Arc<Mutex<Vec<DownloadItem>>>,
    download_options: DownloadOptions,
    download_semaphore: Arc<Semaphore>,
    max_parallel_downloads: u8,

    // Settings
    output_dir: String,
    proxy: String,
    tv_quality: String,
    radio_quality: String,
    settings_dirty: bool,

    // UI state
    active_tab: Tab,
    history_search: String,
    status_message: String,
    _show_info_panel: bool,
}

impl MyApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let rt = Runtime::new().expect("Failed to create Tokio runtime");
        let config = Config::new(None);
        let history = History::new(config.history_file());

        let tv_cache_path = config.cache_file("tv");
        let radio_cache_path = config.cache_file("radio");
        let tv_cache = Cache::load_file(&tv_cache_path, ProgrammeType::Tv);
        let radio_cache = Cache::load_file(&radio_cache_path, ProgrammeType::Radio);

        let cache_status = format!(
            "TV: {} programmes | Radio: {} programmes",
            tv_cache.programmes.len(),
            radio_cache.programmes.len()
        );

        let output_dir = config
            .options
            .get("output")
            .cloned()
            .unwrap_or_else(|| dirs_home().unwrap_or_else(|| ".".to_string()));
        let proxy = config.options.get("proxy").cloned().unwrap_or_default();
        let tv_quality = config
            .options
            .get("tvquality")
            .cloned()
            .unwrap_or_else(|| "hd".to_string());
        let radio_quality = config
            .options
            .get("radioquality")
            .cloned()
            .unwrap_or_else(|| "high".to_string());

        let enabled = |key: &str| config.options.contains_key(key);
        let download_options = DownloadOptions {
            output_dir: Some(PathBuf::from(&output_dir)),
            tv_quality: Some(tv_quality.clone()),
            radio_quality: Some(radio_quality.clone()),
            subtitles: enabled("subtitles"),
            subs_embed: enabled("subsembed"),
            thumb: config.options.get("thumb").is_none_or(|value| value != "0"),
            subdir: enabled("subdir"),
            no_tag: enabled("notag"),
            ..Default::default()
        };

        let max_parallel_downloads = config
            .options
            .get("parallel")
            .or_else(|| config.options.get("maxparallel"))
            .and_then(|v| v.parse::<u8>().ok())
            .map(|v| v.clamp(1, 3))
            .unwrap_or(2);
        let download_semaphore = Arc::new(Semaphore::new(max_parallel_downloads as usize));

        let mut app = Self {
            rt,
            config,
            tv_cache,
            radio_cache,
            cache_loading: false,
            cache_status,
            cache_refresh_rx: None,
            next_cache_refresh_check: Instant::now() + CACHE_MAX_AGE,
            archive_loading: false,
            archive_refresh_rx: None,
            api_search_loading: false,
            api_search_rx: None,
            quick_download_input: String::new(),
            quick_download_loading: false,
            quick_download_rx: None,
            history,
            search_query: String::new(),
            search_prog_type: ProgrammeType::Tv,
            search_channel: String::new(),
            search_hide_recorded: false,
            search_newest_first: true,
            search_results: Vec::new(),
            selected_prog: None,
            selected_episode_pids: HashSet::new(),
            film_shelves: Vec::new(),
            film_loading: false,
            film_status: "Select Load Films to fetch the current iPlayer catalogue".to_string(),
            film_refresh_rx: None,
            film_collection: String::new(),
            selected_film_pids: HashSet::new(),
            download_queue: Arc::new(Mutex::new(Vec::new())),
            download_options,
            download_semaphore,
            max_parallel_downloads,
            output_dir,
            proxy,
            tv_quality,
            radio_quality,
            settings_dirty: false,
            active_tab: Tab::Search,
            history_search: String::new(),
            status_message: String::new(),
            _show_info_panel: false,
        };
        if app.cache_files_are_stale() {
            app.refresh_cache(ProgrammeType::All, 7);
        }
        app
    }

    // ─── Search ──────────────────────────────────────────────────────────────

    fn run_search(&mut self) {
        let filter = SearchFilter {
            search_terms: if self.search_query.trim().is_empty() {
                vec![]
            } else {
                vec![self.search_query.trim().to_string()]
            },
            prog_types: match self.search_prog_type {
                ProgrammeType::Tv => vec![ProgrammeType::Tv],
                ProgrammeType::Radio => vec![ProgrammeType::Radio],
                ProgrammeType::All => vec![],
            },
            channel_filter: if self.search_channel.trim().is_empty() {
                None
            } else {
                Some(self.search_channel.trim().to_string())
            },
            hide_recorded: self.search_hide_recorded,
            future: true,
            ..Default::default()
        };

        let matches =
            Searcher::find_matches(&filter, &self.tv_cache, &self.radio_cache, &self.history);
        let now = chrono::Utc::now().timestamp();
        self.search_results = matches
            .into_iter()
            .filter(|programme| is_not_expired(programme, now))
            .cloned()
            .collect();
        self.selected_episode_pids.clear();
        self.selected_prog = None;

        // If local search found nothing and the query looks like a PID or iPlayer URL,
        // automatically query the BBC iPlayer API
        if self.search_results.is_empty() && BbcApi::parse_pid(&self.search_query).is_some() {
            self.search_iplayer_api();
        } else {
            self.status_message = format!("Found {} programmes", self.search_results.len());
        }
    }

    // ─── Cache refresh ────────────────────────────────────────────────────────

    fn refresh_cache(&mut self, prog_type: ProgrammeType, days: i64) {
        if self.cache_loading {
            return;
        }
        self.cache_loading = true;
        self.status_message = "Refreshing cache…".to_string();

        let tv_path = self.config.cache_file("tv");
        let radio_path = self.config.cache_file("radio");
        let (tx, rx) = mpsc::channel();
        self.cache_refresh_rx = Some(rx);
        std::thread::spawn(move || {
            let runtime = match Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = tx.send(Err(format!("could not start refresh runtime: {error}")));
                    return;
                }
            };
            let result = runtime.block_on(async move {
                let indexer = Indexer::new(7);
                let mut tv = Cache::load_file(&tv_path, ProgrammeType::Tv);
                let mut radio = Cache::load_file(&radio_path, ProgrammeType::Radio);
                async {
                    if prog_type == ProgrammeType::Tv || prog_type == ProgrammeType::All {
                        indexer
                            .refresh(&mut tv, ProgrammeType::Tv, days, false, false)
                            .await
                            .map_err(|error| error.to_string())?;
                        // Also index future schedules so upcoming broadcasts are cached
                        indexer
                            .refresh(&mut tv, ProgrammeType::Tv, 14, true, false)
                            .await
                            .map_err(|error| error.to_string())?;
                        tv.save_file(&tv_path).map_err(|error| error.to_string())?;
                    }
                    if prog_type == ProgrammeType::Radio || prog_type == ProgrammeType::All {
                        indexer
                            .refresh(&mut radio, ProgrammeType::Radio, days, false, false)
                            .await
                            .map_err(|error| error.to_string())?;
                        indexer
                            .refresh(&mut radio, ProgrammeType::Radio, 14, true, false)
                            .await
                            .map_err(|error| error.to_string())?;
                        radio
                            .save_file(&radio_path)
                            .map_err(|error| error.to_string())?;
                    }
                    Ok::<_, String>((tv, radio))
                }
                .await
            });
            let _ = tx.send(result);
        });
    }

    fn cache_files_are_stale(&self) -> bool {
        let now = SystemTime::now();
        [
            self.config.cache_file("tv"),
            self.config.cache_file("radio"),
        ]
        .iter()
        .any(|path| cache_file_is_stale(path, now))
    }

    fn poll_cache_refresh(&mut self) {
        let Some(receiver) = &self.cache_refresh_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok((tv, radio))) => {
                self.tv_cache = tv;
                self.radio_cache = radio;
                self.cache_status = format!(
                    "TV: {} programmes | Radio: {} programmes",
                    self.tv_cache.programmes.len(),
                    self.radio_cache.programmes.len()
                );
                self.status_message = "Cache refreshed".to_string();
                self.cache_loading = false;
                self.cache_refresh_rx = None;
            }
            Ok(Err(error)) => {
                self.status_message = format!("Cache refresh failed: {error}");
                self.cache_loading = false;
                self.cache_refresh_rx = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status_message = "Cache refresh failed: background task stopped".to_string();
                self.cache_loading = false;
                self.cache_refresh_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn load_series_archive(&mut self, programme: &Programme) {
        if self.archive_loading || programme.prog_type != ProgrammeType::Tv {
            return;
        }
        self.archive_loading = true;
        self.status_message = format!("Loading available series for {} from BBC…", programme.name);
        let seed = programme.clone();
        let (tx, rx) = mpsc::channel();
        self.archive_refresh_rx = Some(rx);
        std::thread::spawn(move || {
            let result = Runtime::new()
                .map_err(|error| error.to_string())
                .and_then(|runtime| {
                    runtime
                        .block_on(BbcApi::new().get_series_archive(&seed))
                        .map_err(|error| error.to_string())
                });
            let _ = tx.send(result);
        });
    }

    fn poll_series_archive(&mut self) {
        let Some(receiver) = &self.archive_refresh_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(programmes)) => {
                let selected_pid = self
                    .selected_prog
                    .as_ref()
                    .map(|programme| programme.pid.clone());
                let count = programmes.len();
                for programme in programmes {
                    if let Some(&index) = self.tv_cache.by_pid.get(&programme.pid) {
                        let existing = &mut self.tv_cache.programmes[index];
                        existing.seriesnum = programme.seriesnum.or(existing.seriesnum);
                        existing.episodenum = programme.episodenum.or(existing.episodenum);
                        if !programme.episode.is_empty() {
                            existing.episode = programme.episode;
                        }
                        if !programme.desc.is_empty() {
                            existing.desc = programme.desc;
                        }
                        if !programme.available.is_empty() {
                            existing.available = programme.available;
                        }
                        existing.expires = programme.expires;
                        if !programme.channel.is_empty() {
                            existing.channel = programme.channel;
                        }
                        if programme.duration > 0 {
                            existing.duration = programme.duration;
                        }
                        if !programme.thumbnail.is_empty() {
                            existing.thumbnail = programme.thumbnail;
                        }
                        existing.web = programme.web;
                    } else {
                        self.tv_cache.programmes.push(programme);
                    }
                }
                self.tv_cache
                    .sort_and_reindex(ProgrammeType::Tv.index_min());
                let save_result = self.tv_cache.save_file(self.config.cache_file("tv"));
                self.archive_loading = false;
                self.archive_refresh_rx = None;
                if let Err(error) = save_result {
                    self.status_message =
                        format!("Series loaded, but cache could not be saved: {error}");
                } else {
                    self.status_message = format!("Loaded {count} available episodes from BBC");
                }
                self.run_search();
                if let Some(pid) = selected_pid {
                    self.selected_prog = self
                        .tv_cache
                        .by_pid
                        .get(&pid)
                        .map(|&index| self.tv_cache.programmes[index].clone());
                }
            }
            Ok(Err(error)) => {
                self.status_message = format!("Could not load BBC series: {error}");
                self.archive_loading = false;
                self.archive_refresh_rx = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status_message =
                    "Could not load BBC series: background task stopped".to_string();
                self.archive_loading = false;
                self.archive_refresh_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn search_iplayer_api(&mut self) {
        let query = self.search_query.trim().to_string();
        if self.api_search_loading || query.is_empty() {
            return;
        }
        self.api_search_loading = true;
        self.status_message = format!("Searching BBC iPlayer API for '{query}'…");
        let (tx, rx) = mpsc::channel();
        self.api_search_rx = Some(rx);
        std::thread::spawn(move || {
            let result = Runtime::new()
                .map_err(|error| error.to_string())
                .and_then(|runtime| {
                    runtime
                        .block_on(BbcApi::new().search_iplayer(&query))
                        .map_err(|error| error.to_string())
                });
            let _ = tx.send(result);
        });
    }

    fn poll_api_search(&mut self) {
        let Some(receiver) = &self.api_search_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(programmes)) => {
                self.api_search_loading = false;
                self.api_search_rx = None;
                if programmes.is_empty() {
                    self.status_message = "No programmes found on BBC iPlayer API".to_string();
                    return;
                }
                let count = programmes.len();
                for prog in &programmes {
                    if let Some(&idx) = self.tv_cache.by_pid.get(&prog.pid) {
                        self.tv_cache.programmes[idx] = prog.clone();
                    } else {
                        let idx = self.tv_cache.programmes.len();
                        self.tv_cache.by_pid.insert(prog.pid.clone(), idx);
                        self.tv_cache.programmes.push(prog.clone());
                    }
                }
                self.tv_cache
                    .sort_and_reindex(ProgrammeType::Tv.index_min());
                let _ = self.tv_cache.save_file(self.config.cache_file("tv"));
                self.search_results = programmes;
                self.status_message = format!("Loaded {count} programme(s) from BBC iPlayer API");
            }
            Ok(Err(error)) => {
                self.api_search_loading = false;
                self.api_search_rx = None;
                self.status_message = format!("BBC iPlayer API search failed: {error}");
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.api_search_loading = false;
                self.api_search_rx = None;
                self.status_message =
                    "BBC iPlayer API search failed: background task stopped".to_string();
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn queue_from_url_or_pid(&mut self, input: &str) {
        let query = input.trim().to_string();
        if self.quick_download_loading || query.is_empty() {
            return;
        }
        self.quick_download_loading = true;
        self.status_message = format!("Resolving '{query}' from BBC iPlayer API…");
        let (tx, rx) = mpsc::channel();
        self.quick_download_rx = Some(rx);
        std::thread::spawn(move || {
            let result = Runtime::new()
                .map_err(|error| error.to_string())
                .and_then(|runtime| {
                    runtime
                        .block_on(BbcApi::new().get_programme_or_series(&query))
                        .map_err(|error| error.to_string())
                });
            let _ = tx.send(result);
        });
    }

    fn poll_quick_download(&mut self) {
        let Some(receiver) = &self.quick_download_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(episodes)) => {
                self.quick_download_loading = false;
                self.quick_download_rx = None;
                self.quick_download_input.clear();
                if episodes.is_empty() {
                    self.status_message =
                        "No available episodes found for that URL or PID on BBC iPlayer"
                            .to_string();
                    return;
                }
                let count = episodes.len();
                for ep in &episodes {
                    if let Some(&idx) = self.tv_cache.by_pid.get(&ep.pid) {
                        self.tv_cache.programmes[idx] = ep.clone();
                    } else {
                        let idx = self.tv_cache.programmes.len();
                        self.tv_cache.by_pid.insert(ep.pid.clone(), idx);
                        self.tv_cache.programmes.push(ep.clone());
                    }
                }
                self.tv_cache
                    .sort_and_reindex(ProgrammeType::Tv.index_min());
                let _ = self.tv_cache.save_file(self.config.cache_file("tv"));
                self.enqueue_episodes(episodes);
                self.status_message = format!("Queued {count} episode(s) from BBC iPlayer");
            }
            Ok(Err(error)) => {
                self.quick_download_loading = false;
                self.quick_download_rx = None;
                self.status_message = format!("Could not resolve from BBC iPlayer: {error}");
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.quick_download_loading = false;
                self.quick_download_rx = None;
                self.status_message = "Quick download failed: background task stopped".to_string();
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn load_film_catalogue(&mut self) {
        if self.film_loading {
            return;
        }
        self.film_loading = true;
        self.film_status = "Loading current BBC Films catalogue…".to_string();
        let (tx, rx) = mpsc::channel();
        self.film_refresh_rx = Some(rx);
        std::thread::spawn(move || {
            let result = Runtime::new()
                .map_err(|error| error.to_string())
                .and_then(|runtime| runtime.block_on(fetch_film_shelves()));
            let _ = tx.send(result);
        });
    }

    fn poll_film_catalogue(&mut self) {
        let Some(receiver) = &self.film_refresh_rx else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(shelves)) => {
                let film_count: usize = shelves.iter().map(|shelf| shelf.films.len()).sum();
                self.film_shelves = shelves;
                self.film_collection.clear();
                self.selected_film_pids.clear();
                self.film_status = format!(
                    "Loaded {film_count} film listings across {} BBC collections",
                    self.film_shelves.len()
                );
                self.film_loading = false;
                self.film_refresh_rx = None;
            }
            Ok(Err(error)) => {
                self.film_status = format!("Could not load BBC Films: {error}");
                self.film_loading = false;
                self.film_refresh_rx = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.film_status = "Could not load BBC Films: background task stopped".to_string();
                self.film_loading = false;
                self.film_refresh_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    // ─── Enqueue download ─────────────────────────────────────────────────────

    fn enqueue_download(&mut self, prog: Programme) {
        {
            let queue = self.download_queue.lock().unwrap();
            if queue
                .iter()
                .any(|d| d.prog.pid == prog.pid && d.status != DownloadStatus::Failed)
            {
                self.status_message = format!("'{}' is already queued or running", prog.name);
                return;
            }
        }

        {
            let mut queue = self.download_queue.lock().unwrap();
            queue.push(DownloadItem {
                prog: prog.clone(),
                progress: 0.0,
                stage: "Queued".to_string(),
                status: DownloadStatus::Queued,
                error: None,
            });
        }

        self.status_message = format!("Queued: {}", prog.name);
        self.spawn_download(prog);
    }

    fn episodes_for_show(&self, selected: &Programme) -> Vec<Programme> {
        available_episodes_for_show(
            &self.tv_cache.programmes,
            &self.radio_cache.programmes,
            selected,
        )
    }

    fn enqueue_episodes(&mut self, episodes: Vec<Programme>) {
        if episodes.is_empty() {
            self.status_message = "No available episodes to queue".to_string();
            return;
        }
        let count = episodes.len();
        for episode in episodes {
            self.enqueue_download(episode);
        }
        self.status_message = format!("Queued {count} available episode(s)");
        self.active_tab = Tab::Downloads;
    }

    fn spawn_download(&self, prog: Programme) {
        let queue_arc = self.download_queue.clone();
        let options = self.download_options.clone();
        let semaphore = self.download_semaphore.clone();
        let pid = prog.pid.clone();

        let proxy_str = if self.proxy.trim().is_empty() {
            None
        } else {
            Some(self.proxy.trim().to_string())
        };

        self.rt.spawn(async move {
            let Ok(_permit) = semaphore.acquire().await else {
                return;
            };

            {
                let mut queue = queue_arc.lock().unwrap();
                if let Some(item) = queue.iter_mut().find(|d| d.prog.pid == pid) {
                    item.status = DownloadStatus::Running;
                    item.stage = "Starting…".to_string();
                }
            }

            let bbc_api = BbcApi::new();
            let mediaselector = Mediaselector::new();
            let downloader = Downloader::new(proxy_str.as_deref());
            let postproc = PostProcessor::new(None, None);
            let tagger = Tagger::new(None, &postproc.ffmpeg_path);
            let history_path = Config::new(None).history_file();
            let mut history = History::new(history_path);

            let services = DownloadServices {
                bbc_api: &bbc_api,
                mediaselector: &mediaselector,
                downloader: &downloader,
                postproc: &postproc,
                tagger: &tagger,
            };

            let queue_cb = queue_arc.clone();
            let pid_cb = pid.clone();

            let progress_cb: Arc<dyn Fn(DownloadProgress) + Send + Sync> =
                Arc::new(move |p: DownloadProgress| {
                    let mut queue = queue_cb.lock().unwrap();
                    if let Some(item) = queue.iter_mut().find(|d| d.prog.pid == pid_cb) {
                        item.progress = p.percent;
                        item.stage = p.stage.as_str().to_string();
                        if p.stage == DownloadStage::Failed {
                            item.status = DownloadStatus::Failed;
                        }
                    }
                });

            let mut prog_mut = prog;
            let result = get_iplayer_common::downloader::pipeline::download_programme(
                &mut prog_mut,
                &options,
                &services,
                &mut history,
                Some(progress_cb),
            )
            .await;

            let mut queue = queue_arc.lock().unwrap();
            if let Some(item) = queue.iter_mut().find(|d| d.prog.pid == pid) {
                match result {
                    Ok(_) => {
                        item.status = DownloadStatus::Done;
                        item.progress = 100.0;
                        item.stage = "Completed".to_string();
                    }
                    Err(e) => {
                        item.status = DownloadStatus::Failed;
                        item.stage = "Failed".to_string();
                        item.error = Some(e.to_string());
                    }
                }
            }
        });
    }

    fn set_max_parallel_downloads(&mut self, n: u8) {
        let clamped = n.clamp(1, 3);
        if clamped != self.max_parallel_downloads {
            self.max_parallel_downloads = clamped;
            self.download_semaphore = Arc::new(Semaphore::new(clamped as usize));
            self.settings_dirty = true;
        }
    }

    // ─── UI panels ───────────────────────────────────────────────────────────

    fn ui_toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("🎬 get_iplayer_rs");
            ui.separator();

            for (tab, label) in [
                (Tab::Search, "🔍 Search"),
                (Tab::Browse, "🎞 Browse"),
                (Tab::Downloads, "⬇ Downloads"),
                (Tab::History, "🕐 History"),
                (Tab::Settings, "⚙ Settings"),
                (Tab::About, "ℹ About"),
            ] {
                let selected = self.active_tab == tab;
                if ui.selectable_label(selected, label).clicked() {
                    self.active_tab = tab;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Cache status
                ui.label(
                    RichText::new(&self.cache_status)
                        .font(FontId::proportional(11.0))
                        .color(Color32::GRAY),
                );
                ui.separator();

                if self.cache_loading {
                    ui.spinner();
                    ui.label("Refreshing…");
                } else {
                    if ui.button("🔄 Refresh TV").clicked() {
                        self.refresh_cache(ProgrammeType::Tv, 7);
                    }
                    if ui.button("🔄 Refresh Radio").clicked() {
                        self.refresh_cache(ProgrammeType::Radio, 7);
                    }
                }
            });
        });
    }

    fn ui_search_tab(&mut self, ui: &mut Ui) {
        // ── Filters bar
        ui.horizontal_wrapped(|ui| {
            ui.label("Search:");
            let query_response = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .desired_width(260.0)
                    .hint_text("Programme name, PID, or index…"),
            );
            if query_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.run_search();
            }

            ui.label("Type:");
            egui::ComboBox::from_id_salt("type_combo")
                .selected_text(match self.search_prog_type {
                    ProgrammeType::Tv => "TV",
                    ProgrammeType::Radio => "Radio",
                    ProgrammeType::All => "All",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.search_prog_type, ProgrammeType::Tv, "TV");
                    ui.selectable_value(&mut self.search_prog_type, ProgrammeType::Radio, "Radio");
                    ui.selectable_value(&mut self.search_prog_type, ProgrammeType::All, "All");
                });

            ui.label("Channel:");
            ui.add(
                egui::TextEdit::singleline(&mut self.search_channel)
                    .desired_width(130.0)
                    .hint_text("e.g. BBC One"),
            );

            ui.checkbox(&mut self.search_hide_recorded, "Hide recorded");

            ui.label("Order:");
            egui::ComboBox::from_id_salt("search_order_combo")
                .selected_text(if self.search_newest_first {
                    "Newest first"
                } else {
                    "Oldest first"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.search_newest_first, true, "Newest first");
                    ui.selectable_value(&mut self.search_newest_first, false, "Oldest first");
                });

            if ui.button("Search").clicked() {
                self.run_search();
            }
            if ui
                .add_enabled(
                    !self.api_search_loading && !self.search_query.trim().is_empty(),
                    egui::Button::new(if self.api_search_loading {
                        "Searching iPlayer…"
                    } else {
                        "🌐 Search iPlayer API"
                    }),
                )
                .on_hover_text("Search the live BBC iPlayer API for this title or URL/PID")
                .clicked()
            {
                self.search_iplayer_api();
            }
            if ui.button("Clear").clicked() {
                self.search_query.clear();
                self.search_channel.clear();
                self.search_results.clear();
                self.selected_prog = None;
                self.selected_episode_pids.clear();
                self.status_message.clear();
            }
        });

        ui.separator();

        // ── Results + info panel
        let show_info = self.selected_prog.is_some();
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let (list_width, info_width) = search_panel_widths(available_width, show_info);

        ui.horizontal(|ui| {
            // Results list
            ui.vertical(|ui| {
                ui.set_width(list_width.max(0.0));
                ui.set_height(available_height);
                egui::ScrollArea::vertical()
                    .id_salt("results_scroll")
                    .max_height(available_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.search_results.is_empty() {
                            if self.api_search_loading {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Searching BBC iPlayer API…");
                                });
                            } else if !self.search_query.trim().is_empty() {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("No matching programmes found in local cache.")
                                            .color(Color32::GRAY),
                                    );
                                    ui.add_space(4.0);
                                    if ui
                                        .button("🌐 Search BBC iPlayer API for this title")
                                        .on_hover_text("Query the live BBC iPlayer API directly")
                                        .clicked()
                                    {
                                        self.search_iplayer_api();
                                    }
                                });
                            } else {
                                ui.label(
                                    RichText::new("No results. Enter a programme name, PID, or iPlayer URL and press Search.")
                                        .color(Color32::GRAY),
                                );
                            }
                        } else {
                            let results_clone: Vec<Programme> = self.search_results.clone();
                            let show_groups =
                                group_search_results(&results_clone, self.search_newest_first);
                            let mut to_download: Option<Programme> = None;
                            let mut to_download_batch: Option<Vec<Programme>> = None;
                            let mut to_load_archive: Option<Programme> = None;
                            let mut to_select: Option<Programme> = None;
                            ui.horizontal(|ui| {
                                let selected_count = self.selected_episode_pids.len();
                                ui.label(format!(
                                    "{} matches in {} shows",
                                    results_clone.len(),
                                    show_groups.len()
                                ));
                                if ui
                                    .add_enabled(
                                        selected_count > 0,
                                        egui::Button::new(format!(
                                            "Download selected ({selected_count})"
                                        )),
                                    )
                                    .clicked()
                                {
                                    to_download_batch = Some(
                                        results_clone
                                            .iter()
                                            .filter(|episode| {
                                                self.selected_episode_pids.contains(&episode.pid)
                                            })
                                            .cloned()
                                            .collect(),
                                    );
                                }
                                if selected_count > 0
                                    && ui.small_button("Clear selection").clicked()
                                {
                                    self.selected_episode_pids.clear();
                                }
                            });
                            ui.separator();

                            for show in &show_groups {
                                let show_episodes: Vec<Programme> = show
                                    .seasons
                                    .iter()
                                    .flat_map(|season| season.episodes.iter().cloned())
                                    .collect();
                                egui::CollapsingHeader::new(format!(
                                    "{} ({})",
                                    show.title,
                                    show_episodes.len()
                                ))
                                .id_salt(("show", &show.title))
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if show_episodes.first().is_some_and(|episode| {
                                            episode.prog_type == ProgrammeType::Tv
                                        }) && ui
                                            .add_enabled(
                                                !self.archive_loading,
                                                egui::Button::new(if self.archive_loading {
                                                    "Loading available episodes…"
                                                } else {
                                                    "Load all available episodes"
                                                }),
                                            )
                                            .clicked()
                                        {
                                            to_load_archive = show_episodes.first().cloned();
                                        }
                                        if ui.small_button("Download series").clicked() {
                                            to_download_batch = Some(show_episodes.clone());
                                        }
                                        ui.label(
                                            RichText::new(
                                                "Select episodes below to queue a custom batch",
                                            )
                                            .size(10.0)
                                            .color(Color32::GRAY),
                                        );
                                    });
                                    for season in &show.seasons {
                                        egui::CollapsingHeader::new(format!(
                                            "{} ({})",
                                            season.label,
                                            season.episodes.len()
                                        ))
                                        .id_salt(("season", &show.title, &season.label))
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                if ui.small_button("Download season").clicked() {
                                                    to_download_batch =
                                                        Some(season.episodes.clone());
                                                }
                                            });
                                            for episode in &season.episodes {
                                                ui.horizontal_top(|ui| {
                                                    let mut selected = self
                                                        .selected_episode_pids
                                                        .contains(&episode.pid);
                                                    if ui.checkbox(&mut selected, "").changed() {
                                                        if selected {
                                                            self.selected_episode_pids
                                                                .insert(episode.pid.clone());
                                                        } else {
                                                            self.selected_episode_pids
                                                                .remove(&episode.pid);
                                                        }
                                                    }
                                                    ui.vertical(|ui| {
                                                        let label = format!(
                                                            "{}  {}{}",
                                                            episode.index,
                                                            episode_display_title(episode),
                                                            episode
                                                                .episodenum
                                                                .map(|number| format!(
                                                                    " (Episode {number})"
                                                                ))
                                                                .unwrap_or_default()
                                                        );
                                                        ui.horizontal(|ui| {
                                                            if ui
                                                                .selectable_label(
                                                                    self.selected_prog
                                                                        .as_ref()
                                                                        .is_some_and(|selected| {
                                                                            selected.pid
                                                                                == episode.pid
                                                                        }),
                                                                    label,
                                                                )
                                                                .clicked()
                                                            {
                                                                to_select = Some(episode.clone());
                                                            }
                                                            ui.with_layout(
                                                                egui::Layout::right_to_left(
                                                                    egui::Align::Center,
                                                                ),
                                                                |ui| {
                                                                    if ui
                                                                        .small_button("⬇")
                                                                        .on_hover_text(
                                                                            "Download this episode",
                                                                        )
                                                                        .clicked()
                                                                    {
                                                                        to_download =
                                                                            Some(episode.clone());
                                                                    }
                                                                    ui.label(
                                                                        RichText::new(
                                                                            &episode.channel,
                                                                        )
                                                                        .size(10.0)
                                                                        .color(Color32::GRAY),
                                                                    );
                                                                },
                                                            );
                                                        });
                                                        let available = format_available_date(
                                                            &episode.available,
                                                        );
                                                        let metadata = if available.is_empty() {
                                                            String::new()
                                                        } else {
                                                            format!("Available {available}")
                                                        };
                                                        if !metadata.is_empty() {
                                                            ui.label(
                                                                RichText::new(metadata)
                                                                    .size(10.0)
                                                                    .color(Color32::GRAY),
                                                            );
                                                        }
                                                        if !episode.desc.trim().is_empty() {
                                                            ui.label(
                                                                RichText::new(episode.desc.trim())
                                                                    .size(11.0)
                                                                    .color(Color32::LIGHT_GRAY),
                                                            );
                                                        }
                                                    });
                                                });
                                            }
                                        });
                                    }
                                });
                            }

                            if let Some(p) = to_download {
                                self.enqueue_download(p);
                            }
                            if let Some(batch) = to_download_batch {
                                self.enqueue_episodes(batch);
                            }
                            if let Some(programme) = to_load_archive {
                                self.load_series_archive(&programme);
                            }
                            if let Some(p) = to_select {
                                let already_selected = self
                                    .selected_prog
                                    .as_ref()
                                    .map(|s| s.pid == p.pid)
                                    .unwrap_or(false);
                                if already_selected {
                                    self.selected_prog = None;
                                } else {
                                    self.selected_prog = Some(p);
                                }
                            }
                        }
                    });
            });

            // ── Info panel
            if let Some(prog) = &self.selected_prog.clone() {
                let available_episodes = self.episodes_for_show(prog);
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(info_width);
                    ui.heading(RichText::new(&prog.name).size(15.0));

                    if !prog.episode.is_empty() && prog.episode != prog.name {
                        ui.label(RichText::new(&prog.episode).size(13.0).italics());
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Channel:").strong());
                        ui.label(&prog.channel);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Type:").strong());
                        ui.label(prog.prog_type.as_str());
                    });
                    if prog.duration > 0 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Duration:").strong());
                            ui.label(format_duration(prog.duration));
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PID:").strong());
                        ui.label(RichText::new(&prog.pid).monospace().size(11.0));
                    });
                    if !prog.available.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Available:").strong());
                            ui.label(&prog.available);
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Series:").strong());
                        ui.label(series_label(prog.seriesnum));
                    });
                    if let Some(e) = prog.episodenum {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Episode:").strong());
                            ui.label(format!("{}", e));
                        });
                    }
                    if !prog.categories.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Categories:").strong());
                            ui.label(&prog.categories);
                        });
                    }

                    ui.add_space(6.0);
                    if !prog.desc.is_empty() {
                        ui.label(RichText::new("Description:").strong());
                        ui.add(egui::Label::new(RichText::new(&prog.desc).size(12.0)).wrap());
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Available episodes ({})",
                                available_episodes.len()
                            ))
                            .strong(),
                        );
                        if ui.small_button("Download series").clicked() {
                            self.enqueue_episodes(available_episodes.clone());
                        }
                    });
                    if !available_episodes.is_empty() {
                        egui::ScrollArea::vertical()
                            .id_salt("available_episodes_scroll")
                            .max_height(220.0)
                            .show(ui, |ui| {
                                let mut seasons: Vec<Option<u32>> =
                                    available_episodes.iter().map(|ep| ep.seriesnum).collect();
                                seasons.sort();
                                seasons.dedup();
                                for season in seasons {
                                    let season_episodes: Vec<Programme> = available_episodes
                                        .iter()
                                        .filter(|ep| ep.seriesnum == season)
                                        .cloned()
                                        .collect();
                                    ui.horizontal(|ui| {
                                        let label = format!(
                                            "{} ({} episodes)",
                                            series_label(season),
                                            season_episodes.len()
                                        );
                                        ui.label(label);
                                        let download_label = if season.is_some() {
                                            "Download series"
                                        } else {
                                            "Download episodes"
                                        };
                                        if ui.small_button(download_label).clicked() {
                                            self.enqueue_episodes(season_episodes);
                                        }
                                    });
                                    for episode in available_episodes
                                        .iter()
                                        .filter(|ep| ep.seriesnum == season)
                                    {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{}{}",
                                                    episode_display_title(episode),
                                                    episode
                                                        .episodenum
                                                        .map(|n| format!(" (Episode {n})"))
                                                        .unwrap_or_default()
                                                ))
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                            );
                                            if ui
                                                .small_button("⬇")
                                                .on_hover_text("Download this episode")
                                                .clicked()
                                            {
                                                self.enqueue_download(episode.clone());
                                            }
                                        });
                                    }
                                }
                            });
                    }

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("⬇ Download").clicked() {
                            let p = prog.clone();
                            self.enqueue_download(p);
                            self.active_tab = Tab::Downloads;
                        }
                        if !prog.web.is_empty() && ui.button("🌐 Open in browser").clicked() {
                            let _ = open_url(&prog.web);
                        }
                        if ui.button("✖ Close").clicked() {
                            self.selected_prog = None;
                        }
                    });
                });
            }
        });
    }

    fn ui_browse_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("Browse iPlayer categories");
            ui.separator();
            ui.label("Category: Films");
            ui.label("BBC collection:");
            egui::ComboBox::from_id_salt("film_collection")
                .selected_text(if self.film_collection.is_empty() {
                    "All collections"
                } else {
                    &self.film_collection
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.film_collection.is_empty(), "All collections")
                        .clicked()
                    {
                        self.film_collection.clear();
                    }
                    for shelf in &self.film_shelves {
                        if ui
                            .selectable_label(self.film_collection == shelf.title, &shelf.title)
                            .clicked()
                        {
                            self.film_collection = shelf.title.clone();
                        }
                    }
                });
            if self.film_loading {
                ui.spinner();
                ui.label("Loading…");
            } else if ui.button("↻ Load Films").clicked() {
                self.load_film_catalogue();
            }
        });
        ui.label(
            RichText::new(&self.film_status)
                .size(11.0)
                .color(Color32::GRAY),
        );
        ui.label(RichText::new("BBC collections are used as shelves; BBC does not supply a consistent genre label for every film.").size(10.0).color(Color32::GRAY));
        ui.separator();

        let shown_shelves: Vec<FilmShelf> = self
            .film_shelves
            .iter()
            .filter(|shelf| self.film_collection.is_empty() || shelf.title == self.film_collection)
            .cloned()
            .collect();
        let mut to_download: Option<Programme> = None;
        let mut download_selected = false;
        ui.horizontal(|ui| {
            ui.label(format!("{} collections", shown_shelves.len()));
            let selected_count = self.selected_film_pids.len();
            if ui
                .add_enabled(
                    selected_count > 0,
                    egui::Button::new(format!("Download selected ({selected_count})")),
                )
                .clicked()
            {
                download_selected = true;
            }
            if selected_count > 0 && ui.small_button("Clear selection").clicked() {
                self.selected_film_pids.clear();
            }
        });

        egui::ScrollArea::vertical()
            .id_salt("film_catalogue_scroll")
            .show(ui, |ui| {
                for shelf in &shown_shelves {
                    egui::CollapsingHeader::new(format!("{} ({})", shelf.title, shelf.films.len()))
                        .id_salt(("film_shelf", &shelf.title))
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut films = shelf.films.clone();
                            films.sort_by_key(|film| film.name.to_lowercase());
                            for film in &films {
                                ui.horizontal(|ui| {
                                    let mut selected = self.selected_film_pids.contains(&film.pid);
                                    if ui.checkbox(&mut selected, "").changed() {
                                        if selected {
                                            self.selected_film_pids.insert(film.pid.clone());
                                        } else {
                                            self.selected_film_pids.remove(&film.pid);
                                        }
                                    }
                                    ui.label(RichText::new(&film.name).strong());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .small_button("⬇")
                                                .on_hover_text("Download this film")
                                                .clicked()
                                            {
                                                to_download = Some(film.clone());
                                            }
                                        },
                                    );
                                });
                            }
                        });
                }
            });

        if let Some(film) = to_download {
            self.enqueue_download(film);
            self.active_tab = Tab::Downloads;
        }
        if download_selected {
            let mut seen = HashSet::new();
            let selected: Vec<Programme> = self
                .film_shelves
                .iter()
                .flat_map(|shelf| shelf.films.iter())
                .filter(|film| {
                    self.selected_film_pids.contains(&film.pid) && seen.insert(film.pid.clone())
                })
                .cloned()
                .collect();
            self.enqueue_episodes(selected);
            self.selected_film_pids.clear();
        }
    }

    fn ui_downloads_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("Downloads");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let queue = self.download_queue.lock().unwrap();
                let done = queue
                    .iter()
                    .filter(|d| d.status == DownloadStatus::Done)
                    .count();
                let running = queue
                    .iter()
                    .filter(|d| d.status == DownloadStatus::Running)
                    .count();
                let failed = queue
                    .iter()
                    .filter(|d| d.status == DownloadStatus::Failed)
                    .count();
                let queued = queue
                    .iter()
                    .filter(|d| d.status == DownloadStatus::Queued)
                    .count();
                ui.label(format!(
                    "Queued: {queued}  Running: {running}  Done: {done}  Failed: {failed}"
                ));
            });
        });
        ui.separator();

        // Add by URL or PID
        ui.horizontal(|ui| {
            ui.label(RichText::new("Add by URL / PID:").strong());
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.quick_download_input)
                    .desired_width(340.0)
                    .hint_text("Paste BBC iPlayer episode/series URL, or PID…"),
            );
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let btn = ui
                .add_enabled(
                    !self.quick_download_loading && !self.quick_download_input.trim().is_empty(),
                    egui::Button::new(if self.quick_download_loading {
                        "Fetching from BBC API…"
                    } else {
                        "⬇ Queue from iPlayer"
                    }),
                )
                .on_hover_text("Fetch programme/series from BBC iPlayer API and queue download")
                .clicked();
            if (enter || btn) && !self.quick_download_input.trim().is_empty() {
                let input = self.quick_download_input.clone();
                self.queue_from_url_or_pid(&input);
            }
            if self.quick_download_loading {
                ui.spinner();
            }
        });
        ui.separator();

        // Clear done
        let can_clear = {
            let q = self.download_queue.lock().unwrap();
            q.iter().any(|d| d.status == DownloadStatus::Done)
        };
        if can_clear && ui.button("Clear completed").clicked() {
            let mut q = self.download_queue.lock().unwrap();
            q.retain(|d| d.status != DownloadStatus::Done);
        }
        ui.add_space(4.0);

        let items: Vec<DownloadItem> = { self.download_queue.lock().unwrap().clone() };

        if items.is_empty() {
            ui.label(RichText::new("No downloads queued.").color(Color32::GRAY));
        } else {
            egui::ScrollArea::vertical()
                .id_salt("downloads_scroll")
                .show(ui, |ui| {
                    for item in &items {
                        let (status_icon, status_color) = match item.status {
                            DownloadStatus::Queued => ("⏳", Color32::GRAY),
                            DownloadStatus::Running => ("▶", Color32::from_rgb(80, 160, 230)),
                            DownloadStatus::Done => ("✔", Color32::from_rgb(80, 200, 80)),
                            DownloadStatus::Failed => ("✖", Color32::from_rgb(220, 80, 80)),
                        };

                        egui::Frame::none()
                            .fill(Color32::from_rgb(28, 28, 32))
                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                            .rounding(4.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width().max(0.0));
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(status_icon).color(status_color).size(14.0),
                                    );
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&item.prog.name).strong());
                                            if !item.prog.episode.is_empty()
                                                && item.prog.episode != item.prog.name
                                            {
                                                ui.label(
                                                    RichText::new(format!(
                                                        " — {}",
                                                        item.prog.episode
                                                    ))
                                                    .size(11.0)
                                                    .color(Color32::GRAY),
                                                );
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&item.stage)
                                                    .size(11.0)
                                                    .color(status_color),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(item.prog.prog_type.as_str())
                                                            .size(10.0)
                                                            .color(Color32::GRAY),
                                                    );
                                                },
                                            );
                                        });

                                        if item.status == DownloadStatus::Running
                                            || item.status == DownloadStatus::Done
                                        {
                                            ui.add(
                                                egui::ProgressBar::new(item.progress / 100.0)
                                                    .show_percentage()
                                                    .desired_width(ui.available_width()),
                                            );
                                        }

                                        if let Some(ref err) = item.error {
                                            ui.label(
                                                RichText::new(format!("Error: {}", err))
                                                    .size(11.0)
                                                    .color(Color32::from_rgb(220, 80, 80)),
                                            );
                                        }
                                    });
                                });
                            });
                        ui.add_space(4.0);
                    }
                });
        }

        // Request repaint while downloads are active
        let has_running = items.iter().any(|d| d.status == DownloadStatus::Running);
        if has_running {
            ui.ctx().request_repaint();
        }
    }

    fn ui_history_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("Download History");
            ui.separator();
            ui.label("Search:");
            ui.add(
                egui::TextEdit::singleline(&mut self.history_search)
                    .desired_width(200.0)
                    .hint_text("Name, episode or PID…"),
            );
        });
        ui.separator();

        let terms: Vec<String> = if self.history_search.trim().is_empty() {
            vec![]
        } else {
            vec![self.history_search.trim().to_string()]
        };

        let entries = self.history.search(&terms);

        if entries.is_empty() {
            ui.label(RichText::new("No history entries found.").color(Color32::GRAY));
            return;
        }

        ui.label(format!("{} entries", entries.len()));
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("history_scroll")
            .show(ui, |ui| {
                for entry in entries.iter().rev() {
                    ui.horizontal(|ui| {
                        ui.set_width(ui.available_width().max(0.0));
                        ui.label(RichText::new(&entry.name).strong().size(12.0));
                        if !entry.episode.is_empty() && entry.episode != entry.name {
                            ui.label(
                                RichText::new(format!(" — {}", entry.episode))
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format_epoch(entry.timeadded))
                                    .size(11.0)
                                    .color(Color32::GRAY)
                                    .monospace(),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(&entry.channel)
                                    .size(11.0)
                                    .color(Color32::from_rgb(140, 160, 200)),
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("PID: {}", entry.pid))
                                .monospace()
                                .size(10.0)
                                .color(Color32::GRAY),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!("Mode: {}", entry.mode))
                                .size(10.0)
                                .color(Color32::GRAY),
                        );
                    });
                    ui.separator();
                }
            });
    }

    fn ui_settings_tab(&mut self, ui: &mut Ui) {
        ui.heading("Settings");
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .show(ui, |ui| {
                egui::Grid::new("settings_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Output directory").strong());
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.output_dir)
                                        .desired_width(320.0),
                                )
                                .changed()
                            {
                                self.settings_dirty = true;
                            }
                            if ui.button("Browse…").clicked() {
                                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                    self.output_dir = p.display().to_string();
                                    self.settings_dirty = true;
                                }
                            }
                        });
                        ui.end_row();

                        ui.label(RichText::new("TV quality").strong());
                        egui::ComboBox::from_id_salt("tv_quality_combo")
                            .selected_text(&self.tv_quality)
                            .show_ui(ui, |ui| {
                                for q in ["fhd", "hd", "sd", "web", "mobile"] {
                                    if ui.selectable_label(self.tv_quality == q, q).clicked() {
                                        self.tv_quality = q.to_string();
                                        self.settings_dirty = true;
                                    }
                                }
                            });
                        ui.end_row();

                        ui.label(RichText::new("Radio quality").strong());
                        egui::ComboBox::from_id_salt("radio_quality_combo")
                            .selected_text(&self.radio_quality)
                            .show_ui(ui, |ui| {
                                for q in ["high", "std", "med", "low"] {
                                    if ui.selectable_label(self.radio_quality == q, q).clicked() {
                                        self.radio_quality = q.to_string();
                                        self.settings_dirty = true;
                                    }
                                }
                            });
                        ui.end_row();

                        ui.label(RichText::new("HTTP proxy").strong());
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.proxy)
                                    .desired_width(320.0)
                                    .hint_text("http://host:port (optional)"),
                            )
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        ui.end_row();

                        ui.label(RichText::new("Include subtitles").strong());
                        if ui
                            .checkbox(&mut self.download_options.subtitles, "Download .srt")
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        ui.end_row();

                        ui.label(RichText::new("Embed subtitles").strong());
                        if ui
                            .checkbox(
                                &mut self.download_options.subs_embed,
                                "Embed into container",
                            )
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        ui.end_row();

                        ui.label(RichText::new("Save thumbnail").strong());
                        if ui
                            .checkbox(&mut self.download_options.thumb, "Keep thumbnail (.jpg)")
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        ui.end_row();

                        ui.label(RichText::new("Skip tagging").strong());
                        if ui
                            .checkbox(
                                &mut self.download_options.no_tag,
                                "Disable metadata tagging",
                            )
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        ui.end_row();

                        ui.label(RichText::new("Sub-directory per series").strong());
                        if ui
                            .checkbox(
                                &mut self.download_options.subdir,
                                "Organise into sub-folders",
                            )
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        ui.end_row();

                        ui.label(RichText::new("Max parallel downloads").strong());
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    self.max_parallel_downloads > 1,
                                    egui::Button::new("−"),
                                )
                                .clicked()
                            {
                                self.set_max_parallel_downloads(self.max_parallel_downloads - 1);
                            }
                            ui.label(
                                RichText::new(self.max_parallel_downloads.to_string()).strong(),
                            );
                            if ui
                                .add_enabled(
                                    self.max_parallel_downloads < 3,
                                    egui::Button::new("+"),
                                )
                                .clicked()
                            {
                                self.set_max_parallel_downloads(self.max_parallel_downloads + 1);
                            }
                            ui.label(
                                RichText::new("(1–3 simultaneous downloads)").color(Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        ui.label(RichText::new("Profile directory").strong());
                        ui.label(
                            RichText::new(self.config.profile_dir.display().to_string())
                                .monospace()
                                .size(11.0)
                                .color(Color32::GRAY),
                        );
                        ui.end_row();
                    });

                ui.add_space(12.0);

                if self.settings_dirty && ui.button("💾 Save settings").clicked() {
                    self.apply_settings();
                }
            });
    }

    fn ui_about_tab(&mut self, ui: &mut Ui) {
        ui.heading("About get_iplayer_rs");
        ui.separator();
        egui::Grid::new("about_details")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Version").strong());
                ui.label(env!("CARGO_PKG_VERSION"));
                ui.end_row();

                ui.label(RichText::new("Licence").strong());
                ui.hyperlink_to(
                    env!("CARGO_PKG_LICENSE"),
                    "https://www.gnu.org/licenses/gpl-3.0.html",
                );
                ui.end_row();

                ui.label(RichText::new("Copyright").strong());
                ui.label("get_iplayer_rs Contributors");
                ui.end_row();

                ui.label(RichText::new("Source code").strong());
                ui.hyperlink_to(
                    "GitHub repository",
                    "https://wob.github.com/henkez73/get_iplayer_rs",
                );
                ui.end_row();

                ui.label(RichText::new("Description").strong());
                ui.label("Lists and records BBC iPlayer TV and BBC Sounds radio programmes.");
                ui.end_row();

                ui.label(RichText::new("BBC services").strong());
                ui.label(BBC_ATTRIBUTION);
                ui.end_row();

                ui.label(RichText::new("BBC terms").strong());
                ui.hyperlink_to("BBC Terms of Use", BBC_TERMS_URL);
                ui.end_row();

                ui.label(RichText::new("TV Licence").strong());
                ui.vertical(|ui| {
                    ui.label(TV_LICENCE_NOTICE);
                    ui.hyperlink_to("TV Licensing: BBC iPlayer guidance", TV_LICENCE_URL);
                });
                ui.end_row();
            });
    }

    fn apply_settings(&mut self) {
        self.download_options.output_dir = Some(PathBuf::from(&self.output_dir));
        self.download_options.tv_quality = Some(self.tv_quality.clone());
        self.download_options.radio_quality = Some(self.radio_quality.clone());

        self.config.prefs_add("output", &self.output_dir);
        self.config.prefs_add("tvquality", &self.tv_quality);
        self.config.prefs_add("radioquality", &self.radio_quality);
        if self.proxy.trim().is_empty() {
            self.config.prefs_del("proxy");
        } else {
            self.config.prefs_add("proxy", self.proxy.trim());
        }
        for (key, enabled) in [
            ("subsembed", self.download_options.subs_embed),
            ("thumb", self.download_options.thumb),
            ("subdir", self.download_options.subdir),
            ("notag", self.download_options.no_tag),
        ] {
            if enabled {
                self.config.prefs_add(key, "1");
            } else {
                self.config.prefs_del(key);
            }
        }
        if self.download_options.subtitles {
            self.config.prefs_add("subtitles", "1");
        } else {
            self.config.prefs_del("subtitles");
        }
        self.download_semaphore = Arc::new(Semaphore::new(self.max_parallel_downloads as usize));
        self.config
            .prefs_add("parallel", &self.max_parallel_downloads.to_string());

        match self.config.save_options() {
            Ok(()) => {
                self.settings_dirty = false;
                self.status_message = "Settings saved".to_string();
            }
            Err(error) => self.status_message = format!("Could not save settings: {error}"),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_cache_refresh();
        self.poll_series_archive();
        self.poll_film_catalogue();
        self.poll_api_search();
        self.poll_quick_download();
        if Instant::now() >= self.next_cache_refresh_check {
            self.next_cache_refresh_check = Instant::now() + CACHE_MAX_AGE;
            if self.cache_files_are_stale() {
                self.refresh_cache(ProgrammeType::All, 7);
            }
        }
        if self.cache_loading
            || self.film_loading
            || self.archive_loading
            || self.api_search_loading
            || self.quick_download_loading
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(
                self.next_cache_refresh_check
                    .saturating_duration_since(Instant::now()),
            );
        }
        // Top toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.ui_toolbar(ui);
        });

        // Status bar
        egui::TopBottomPanel::bottom("statusbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&self.status_message)
                        .size(11.0)
                        .color(Color32::from_rgb(160, 180, 160)),
                );
            });
        });

        // Main area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Need to clone the tab to avoid borrow issues
            match self.active_tab.clone() {
                Tab::Search => self.ui_search_tab(ui),
                Tab::Browse => self.ui_browse_tab(ui),
                Tab::Downloads => self.ui_downloads_tab(ui),
                Tab::History => self.ui_history_tab(ui),
                Tab::Settings => self.ui_settings_tab(ui),
                Tab::About => self.ui_about_tab(ui),
            }
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn format_duration(secs: u32) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn search_panel_widths(available_width: f32, show_info: bool) -> (f32, f32) {
    let available_width = available_width.max(0.0);
    if show_info {
        let list_width = available_width * 0.55;
        (list_width, (available_width - list_width - 8.0).max(0.0))
    } else {
        (available_width, 0.0)
    }
}

fn format_epoch(ts: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(ts, 0).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M").to_string()
}

fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

fn available_episodes_for_show(
    tv_programmes: &[Programme],
    radio_programmes: &[Programme],
    selected: &Programme,
) -> Vec<Programme> {
    let now = chrono::Utc::now().timestamp();
    let mut episodes: Vec<Programme> = tv_programmes
        .iter()
        .chain(radio_programmes.iter())
        .filter(|episode| {
            episode.prog_type == selected.prog_type
                && episode.name.eq_ignore_ascii_case(&selected.name)
                && is_not_expired(episode, now)
        })
        .cloned()
        .collect();
    episodes.sort_by(|a, b| {
        a.seriesnum
            .cmp(&b.seriesnum)
            .then_with(|| a.episodenum.cmp(&b.episodenum))
            .then_with(|| a.available.cmp(&b.available))
            .then_with(|| a.episode.cmp(&b.episode))
    });
    episodes
}

fn is_not_expired(programme: &Programme, now: i64) -> bool {
    programme.expires == 0 || programme.expires > now
}

fn series_label(seriesnum: Option<u32>) -> String {
    seriesnum
        .map(|number| format!("Series {number}"))
        .unwrap_or_else(|| "Series not specified".to_string())
}

fn episode_display_title(episode: &Programme) -> String {
    if !episode.episode.trim().is_empty()
        && episode.episode != "-"
        && episode.episode != episode.name
    {
        return episode.episode.clone();
    }
    if let Some(description) = episode.desc.lines().find(|line| !line.trim().is_empty()) {
        return description.trim().to_string();
    }
    episode
        .episodenum
        .map(|number| format!("Episode {number}"))
        .unwrap_or_else(|| format!("Programme {}", episode.index))
}

fn format_available_date(available: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(available)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| available.to_string())
}

async fn fetch_film_shelves() -> Result<Vec<FilmShelf>, String> {
    let response = reqwest::Client::new()
        .get("https://www.bbc.co.uk/iplayer/categories/films/most-recent")
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (compatible; get_iplayer_rs_gui/1.0)",
        )
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("BBC returned HTTP {}", response.status()));
    }
    let html = response.text().await.map_err(|error| error.to_string())?;
    let shelves = parse_film_shelves(&html)?;
    if shelves.is_empty() {
        return Err("BBC returned no film listings".to_string());
    }
    Ok(shelves)
}

fn parse_film_shelves(html: &str) -> Result<Vec<FilmShelf>, String> {
    let tag_pattern = Regex::new(
        r#"(?is)<h2\b[^>]*>(?P<heading>.*?)</h2>|<a\b[^>]*href=["'](?P<url>/iplayer/episode/(?P<pid>[a-z0-9]{8})[^"']*)["'][^>]*>(?P<title>.*?)</a>"#,
    )
    .map_err(|error| error.to_string())?;
    let strip_tags = Regex::new(r"(?is)<[^>]+>").map_err(|error| error.to_string())?;
    let mut shelves: Vec<FilmShelf> = Vec::new();
    let mut current_title = "Films".to_string();

    for capture in tag_pattern.captures_iter(html) {
        if let Some(heading) = capture.name("heading") {
            let title = htmlescape::decode_html(&strip_tags.replace_all(heading.as_str(), " "))
                .unwrap_or_default()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if !title.is_empty() {
                current_title = title;
            }
            continue;
        }

        let Some(pid) = capture.name("pid").map(|m| m.as_str()) else {
            continue;
        };
        let Some(title) = capture.name("title") else {
            continue;
        };
        let title = htmlescape::decode_html(&strip_tags.replace_all(title.as_str(), " "))
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if title.is_empty() {
            continue;
        }

        let shelf = if let Some(shelf) = shelves
            .iter_mut()
            .find(|shelf| shelf.title == current_title)
        {
            shelf
        } else {
            shelves.push(FilmShelf {
                title: current_title.clone(),
                films: Vec::new(),
            });
            let Some(shelf) = shelves.last_mut() else {
                continue;
            };
            shelf
        };
        if shelf.films.iter().any(|film| film.pid == pid) {
            continue;
        }
        shelf.films.push(Programme {
            prog_type: ProgrammeType::Tv,
            name: title,
            pid: pid.to_string(),
            web: format!("https://www.bbc.co.uk/iplayer/episode/{pid}"),
            ..Default::default()
        });
    }

    shelves.retain(|shelf| !shelf.films.is_empty());
    Ok(shelves)
}

fn group_search_results(results: &[Programme], newest_first: bool) -> Vec<SearchShowGroup> {
    let mut by_show: HashMap<String, Vec<&Programme>> = HashMap::new();
    for result in results {
        by_show
            .entry(result.name.trim().to_lowercase())
            .or_default()
            .push(result);
    }

    let mut grouped: BTreeMap<(String, String), SearchShowBuilder> = BTreeMap::new();
    for (show_key, episodes) in by_show {
        let mut subtitle_counts: HashMap<String, usize> = HashMap::new();
        for episode in &episodes {
            let subtitle = episode.episode.trim();
            if !subtitle.is_empty() && subtitle != "-" && subtitle != episode.name {
                *subtitle_counts.entry(subtitle.to_lowercase()).or_default() += 1;
            }
        }

        for episode in episodes {
            // Repeated sub-titles such as "Uncloaked" identify a spin-off programme
            // whose episodes are otherwise catalogued under the parent show name.
            let subtitle = episode.episode.trim();
            let qualifier = if !subtitle.is_empty()
                && subtitle != "-"
                && subtitle != episode.name
                && subtitle_counts
                    .get(&subtitle.to_lowercase())
                    .is_some_and(|count| *count > 2)
            {
                subtitle.to_string()
            } else {
                String::new()
            };
            let group_title = if qualifier.is_empty() {
                episode.name.clone()
            } else {
                format!("{} — {}", episode.name, qualifier)
            };
            let key = (show_key.clone(), qualifier.to_lowercase());
            let entry = grouped.entry(key).or_insert_with(|| SearchShowBuilder {
                title: group_title,
                seasons: BTreeMap::new(),
            });
            entry
                .seasons
                .entry(episode.seriesnum)
                .or_default()
                .push(episode.clone());
        }
    }

    let mut groups: Vec<SearchShowGroup> = grouped
        .into_values()
        .map(|builder| SearchShowGroup {
            title: builder.title,
            seasons: builder
                .seasons
                .into_iter()
                .map(|(number, mut episodes)| {
                    episodes.sort_by(|a, b| {
                        let date_order = compare_available(&a.available, &b.available);
                        let date_order = if newest_first {
                            date_order.reverse()
                        } else {
                            date_order
                        };
                        date_order
                            .then_with(|| a.episodenum.cmp(&b.episodenum))
                            .then_with(|| a.index.cmp(&b.index))
                    });
                    SearchSeasonGroup {
                        label: series_label(number),
                        episodes,
                    }
                })
                .collect(),
        })
        .collect();
    groups.sort_by(|a, b| {
        let latest_a = a
            .seasons
            .iter()
            .flat_map(|season| season.episodes.iter())
            .map(|episode| episode.available.as_str())
            .max_by(|a, b| compare_available(a, b))
            .unwrap_or_default();
        let latest_b = b
            .seasons
            .iter()
            .flat_map(|season| season.episodes.iter())
            .map(|episode| episode.available.as_str())
            .max_by(|a, b| compare_available(a, b))
            .unwrap_or_default();
        let order = compare_available(latest_a, latest_b);
        if newest_first {
            order.reverse()
        } else {
            order
        }
    });
    groups
}

fn compare_available(a: &str, b: &str) -> std::cmp::Ordering {
    match (
        chrono::DateTime::parse_from_rfc3339(a),
        chrono::DateTime::parse_from_rfc3339(b),
    ) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}

fn cache_file_is_stale(path: &std::path::Path, now: SystemTime) -> bool {
    let modified = std::fs::metadata(path).and_then(|metadata| metadata.modified());
    cache_modified_is_stale(modified.ok(), now)
}

fn cache_modified_is_stale(modified: Option<SystemTime>, now: SystemTime) -> bool {
    modified
        .and_then(|modified| now.duration_since(modified).ok())
        .is_none_or(|age| age >= CACHE_MAX_AGE)
}

fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(url).spawn()?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_search_layout_never_produces_negative_widths() {
        assert_eq!(search_panel_widths(10.0, true), (5.5, 0.0));
        assert_eq!(search_panel_widths(0.0, true), (0.0, 0.0));
        assert_eq!(search_panel_widths(-10.0, true), (0.0, 0.0));
        assert_eq!(search_panel_widths(100.0, true), (55.0, 37.0));
        assert_eq!(search_panel_widths(10.0, false), (10.0, 0.0));
    }

    #[test]
    fn search_availability_excludes_expired_programmes_but_keeps_unknown_expiry() {
        let now = chrono::Utc::now().timestamp();
        let programmes = [
            Programme {
                expires: now - 1,
                ..episode("expired", 1, 1)
            },
            Programme {
                expires: now + 1,
                ..episode("available", 1, 2)
            },
            Programme {
                expires: 0,
                ..episode("unknown expiry", 1, 3)
            },
        ];

        let visible: Vec<&str> = programmes
            .iter()
            .filter(|programme| is_not_expired(programme, now))
            .map(|programme| programme.pid.as_str())
            .collect();
        assert_eq!(visible, ["available", "unknown expiry"]);
    }

    #[test]
    fn series_label_makes_missing_series_metadata_explicit() {
        assert_eq!(series_label(Some(14)), "Series 14");
        assert_eq!(series_label(None), "Series not specified");
    }

    fn episode(pid: &str, season: u32, number: u32) -> Programme {
        Programme {
            prog_type: ProgrammeType::Tv,
            name: "The Traitors Ireland".to_string(),
            episode: format!("Episode {number}"),
            seriesnum: Some(season),
            episodenum: Some(number),
            pid: pid.to_string(),
            expires: chrono::Utc::now().timestamp() + 3600,
            ..Default::default()
        }
    }

    #[test]
    fn lists_available_episodes_in_season_and_episode_order() {
        let selected = episode("b0000001", 2, 2);
        let cached = vec![
            episode("b0000003", 2, 3),
            episode("b0000001", 2, 1),
            episode("b0000002", 2, 2),
            Programme {
                name: "Celebrity Traitors".to_string(),
                ..episode("b0000004", 2, 4)
            },
        ];

        let listed = available_episodes_for_show(&cached, &[], &selected);
        let pids: Vec<&str> = listed
            .iter()
            .map(|programme| programme.pid.as_str())
            .collect();
        assert_eq!(pids, ["b0000001", "b0000002", "b0000003"]);
    }

    #[test]
    fn episode_without_title_uses_description_for_individual_listing() {
        let episode = Programme {
            episode: "-".to_string(),
            desc: "1/10 Claudia welcomes the contestants.\nMore details".to_string(),
            ..episode("b0000001", 1, 1)
        };

        assert_eq!(
            episode_display_title(&episode),
            "1/10 Claudia welcomes the contestants."
        );
    }

    #[test]
    fn groups_repeated_spin_off_subtitle_separately_and_by_series() {
        let programme = |pid: &str, title: &str, series: u32, number: u32| Programme {
            name: "The Traitors".to_string(),
            episode: title.to_string(),
            seriesnum: Some(series),
            episodenum: Some(number),
            pid: pid.to_string(),
            ..Default::default()
        };
        let results = vec![
            programme("b0000001", "Episode 1", 1, 1),
            programme("b0000002", "Episode 2", 1, 2),
            programme("b0000003", "Uncloaked", 1, 1),
            programme("b0000004", "Uncloaked", 1, 2),
            programme("b0000005", "Uncloaked", 2, 1),
        ];

        let groups = group_search_results(&results, true);
        assert_eq!(groups.len(), 2);
        let main_show = groups
            .iter()
            .find(|group| group.title == "The Traitors")
            .unwrap();
        let spin_off = groups
            .iter()
            .find(|group| group.title == "The Traitors — Uncloaked")
            .unwrap();
        assert_eq!(main_show.seasons[0].label, "Series 1");
        assert_eq!(main_show.seasons[0].episodes.len(), 2);
        assert_eq!(spin_off.seasons.len(), 2);
        assert_eq!(spin_off.seasons[0].episodes.len(), 2);
        assert_eq!(spin_off.seasons[1].episodes.len(), 1);
    }

    #[test]
    fn formats_available_timestamp_as_date() {
        assert_eq!(
            format_available_date("2026-09-23T18:30:00+01:00"),
            "2026-09-23"
        );
        assert_eq!(format_available_date("unknown"), "unknown");
    }

    #[test]
    fn about_metadata_is_available_from_package_manifest() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
        assert_eq!(env!("CARGO_PKG_LICENSE"), "GPL-3.0-or-later");
        assert!(BBC_ATTRIBUTION.contains("not affiliated with or endorsed"));
        assert!(TV_LICENCE_NOTICE.contains("watch or download BBC programmes"));
        assert!(BBC_TERMS_URL.starts_with("https://www.bbc.co.uk/"));
        assert!(TV_LICENCE_URL.starts_with("https://www.tvlicensing.co.uk/"));
    }

    #[test]
    fn cache_freshness_uses_three_hour_limit() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        assert!(cache_modified_is_stale(None, now));
        assert!(!cache_modified_is_stale(
            Some(now - CACHE_MAX_AGE + Duration::from_secs(1)),
            now
        ));
        assert!(cache_modified_is_stale(Some(now - CACHE_MAX_AGE), now));
        assert!(cache_modified_is_stale(
            Some(now + Duration::from_secs(1)),
            now
        ));
    }

    #[test]
    fn orders_search_episodes_and_shows_by_availability() {
        let mut results = vec![
            Programme {
                name: "News A".to_string(),
                available: "2026-09-22T10:00:00+00:00".to_string(),
                ..episode("b0000001", 1, 1)
            },
            Programme {
                name: "News A".to_string(),
                available: "2026-09-24T10:00:00+00:00".to_string(),
                ..episode("b0000002", 1, 2)
            },
            Programme {
                name: "News B".to_string(),
                available: "2026-09-23T10:00:00+00:00".to_string(),
                ..episode("b0000003", 1, 1)
            },
        ];

        for (newest_first, expected_episode_order, expected_show_order) in [
            (true, ["b0000002", "b0000001"], ["News A", "News B"]),
            (false, ["b0000001", "b0000002"], ["News B", "News A"]),
        ] {
            let groups = group_search_results(&results, newest_first);
            let news_a = groups.iter().find(|group| group.title == "News A").unwrap();
            let episode_order: Vec<&str> = news_a.seasons[0]
                .episodes
                .iter()
                .map(|episode| episode.pid.as_str())
                .collect();
            let show_order: Vec<&str> = groups.iter().map(|group| group.title.as_str()).collect();
            assert_eq!(episode_order, expected_episode_order);
            assert_eq!(show_order, expected_show_order);
        }

        results.clear();
        assert!(group_search_results(&results, true).is_empty());
    }

    #[test]
    fn parses_film_cards_under_bbc_collection_headings() {
        let html = r#"
            <h2>Featured Films</h2>
            <a href="/iplayer/episode/m002tsct"><h3>The Hunger Games &amp; More</h3></a>
            <h2>Films to Fall in Love With</h2>
            <a href="/iplayer/episode/b0123456"><span>Love Story</span></a>
            <a href="/iplayer/episode/b0123456"><span>Duplicate listing</span></a>
        "#;

        let shelves = parse_film_shelves(html).unwrap();
        assert_eq!(shelves.len(), 2);
        assert_eq!(shelves[0].title, "Featured Films");
        assert_eq!(shelves[0].films[0].name, "The Hunger Games & More");
        assert_eq!(shelves[1].title, "Films to Fall in Love With");
        assert_eq!(shelves[1].films.len(), 1);
        assert_eq!(shelves[1].films[0].pid, "b0123456");
    }

    #[test]
    fn groups_and_orders_episodes_from_api_archive() {
        let episodes = vec![
            Programme {
                name: "Bill's Included".to_string(),
                episode: "Episode 2".to_string(),
                seriesnum: Some(1),
                episodenum: Some(2),
                pid: "m0026b46".to_string(),
                available: "2026-11-20T06:00:00Z".to_string(),
                ..Default::default()
            },
            Programme {
                name: "Bill's Included".to_string(),
                episode: "Episode 1".to_string(),
                seriesnum: Some(1),
                episodenum: Some(1),
                pid: "m0026b45".to_string(),
                available: "2026-11-20T06:00:00Z".to_string(),
                ..Default::default()
            },
        ];

        let groups = group_search_results(&episodes, false);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].title, "Bill's Included");
        assert_eq!(groups[0].seasons.len(), 1);
        assert_eq!(groups[0].seasons[0].label, "Series 1");
        assert_eq!(groups[0].seasons[0].episodes.len(), 2);
        assert_eq!(groups[0].seasons[0].episodes[0].pid, "m0026b45");
        assert_eq!(groups[0].seasons[0].episodes[1].pid, "m0026b46");
    }

    #[test]
    fn max_parallel_downloads_clamps_to_valid_range() {
        let clamp_val = |raw: Option<&str>| -> u8 {
            raw.and_then(|v| v.parse::<u8>().ok())
                .map(|v| v.clamp(1, 3))
                .unwrap_or(2)
        };
        assert_eq!(clamp_val(None), 2);
        assert_eq!(clamp_val(Some("0")), 1);
        assert_eq!(clamp_val(Some("1")), 1);
        assert_eq!(clamp_val(Some("2")), 2);
        assert_eq!(clamp_val(Some("3")), 3);
        assert_eq!(clamp_val(Some("4")), 3);
        assert_eq!(clamp_val(Some("abc")), 2);
    }

    #[tokio::test]
    async fn download_semaphore_limits_concurrency() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let semaphore = Arc::new(Semaphore::new(2));
        let active = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..5 {
            let sem = semaphore.clone();
            let act = active.clone();
            let max_obs = max_observed.clone();
            handles.push(tokio::spawn(async move {
                let Ok(_permit) = sem.acquire().await else {
                    return;
                };
                let count = act.fetch_add(1, Ordering::SeqCst) + 1;
                max_obs.fetch_max(count, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(10)).await;
                act.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            let _ = h.await;
        }

        assert_eq!(max_observed.load(Ordering::SeqCst), 2);
    }
}
