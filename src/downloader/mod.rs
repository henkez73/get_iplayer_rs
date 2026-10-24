pub mod dash;
pub mod hls;

use dash::DashDownloader;
use hls::HlsDownloader;
use std::path::Path;

pub struct Downloader {
    pub client: reqwest::Client,
    pub hls: HlsDownloader,
    pub dash: DashDownloader,
}

impl Downloader {
    pub fn new(proxy: Option<&str>) -> Self {
        let mut builder = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30));

        if let Some(p) = proxy {
            if let Ok(prx) = reqwest::Proxy::all(p) {
                builder = builder.proxy(prx);
            }
        }

        let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());
        let hls = HlsDownloader::new(client.clone());
        let dash = DashDownloader::new(client.clone());

        Self { client, hls, dash }
    }

    /// Downloads a raw file from HTTP/HTTPS (for subtitles, thumbnail, etc.)
    pub async fn download_file(
        &self,
        url: &str,
        dest: &Path,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(format!("HTTP error {}", resp.status()).into());
        }
        let bytes = resp.bytes().await?;
        std::fs::write(dest, &bytes)?;
        Ok(bytes.len() as u64)
    }

    /// Fetches text content from URL (for playlists, manifests, etc.)
    pub async fn get_text(&self, url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.client.get(url).send().await?;
        let text = resp.text().await?;
        Ok(text)
    }
}
