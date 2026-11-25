use crate::error::{Error, Result};
use crate::storage;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;

pub const USER_AGENT: &str = concat!("zip-launcher/", env!("CARGO_PKG_VERSION"));

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(16)
            .build()
            .expect("http client")
    })
}

fn request_timeout() -> Duration {
    Duration::from_secs(storage::settings().downloads.timeout_secs as u64)
}

pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let response = client().get(url).timeout(request_timeout()).send().await?.error_for_status()?;
    Ok(response.json::<T>().await?)
}

pub async fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let response = client().get(url).timeout(request_timeout()).send().await?.error_for_status()?;
    Ok(response.bytes().await?.to_vec())
}

fn cache_file(key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
        .collect();
    storage::cache_dir().join("meta").join(format!("{safe}.json"))
}

fn age_secs(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(modified).ok().map(|d| d.as_secs())
}

pub async fn get_json_cached<T: DeserializeOwned>(url: &str, key: &str, ttl_secs: u64) -> Result<T> {
    let path = cache_file(key);
    if let Some(age) = age_secs(&path) {
        if age < ttl_secs {
            if let Ok(Some(value)) = storage::read_json::<T>(&path) {
                return Ok(value);
            }
        }
    }
    match get_bytes(url).await {
        Ok(bytes) => {
            let value: T = serde_json::from_slice(&bytes)?;
            let _ = storage::write_bytes_atomic(&path, &bytes);
            Ok(value)
        }
        Err(network_error) => match storage::read_json::<T>(&path) {
            Ok(Some(stale)) => Ok(stale),
            _ => Err(network_error),
        },
    }
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn sha1_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

#[derive(Clone, Debug)]
pub struct DownloadItem {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub fallback_urls: Vec<String>,
}

impl DownloadItem {
    pub fn new(url: impl Into<String>, path: PathBuf, sha1: Option<String>, size: Option<u64>) -> Self {
        DownloadItem { url: url.into(), path, sha1, size, fallback_urls: Vec::new() }
    }
}

#[derive(Default)]
pub struct Progress {
    pub total_bytes: AtomicU64,
    pub done_bytes: AtomicU64,
    pub total_files: AtomicU64,
    pub done_files: AtomicU64,
}

impl Progress {
    pub fn fraction(&self) -> f64 {
        let total = self.total_bytes.load(Ordering::Relaxed);
        if total > 0 {
            return (self.done_bytes.load(Ordering::Relaxed) as f64 / total as f64).min(1.0);
        }
        let files = self.total_files.load(Ordering::Relaxed);
        if files > 0 {
            return (self.done_files.load(Ordering::Relaxed) as f64 / files as f64).min(1.0);
        }
        0.0
    }
}

fn needs_download(item: &DownloadItem, verify_hash: bool) -> bool {
    let Ok(meta) = std::fs::metadata(&item.path) else { return true };
    if let Some(size) = item.size {
        if meta.len() != size {
            return true;
        }
    } else if meta.len() == 0 {
        return true;
    }
    if verify_hash {
        if let Some(expected) = &item.sha1 {
            return sha1_file(&item.path).map(|h| !h.eq_ignore_ascii_case(expected)).unwrap_or(true);
        }
    }
    false
}

async fn fetch_once(url: &str, item: &DownloadItem, progress: &Progress) -> Result<()> {
    let response = client().get(url).timeout(Duration::from_secs(600)).send().await?.error_for_status()?;
    if let Some(parent) = item.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let part = item.path.with_extension("zipdl");
    let mut file = tokio::fs::File::create(&part).await?;
    let mut hasher = Sha1::new();
    let mut written: u64 = 0;
    let mut stream = response.bytes_stream();
    let idle = request_timeout();
    loop {
        let next = tokio::time::timeout(idle, stream.next()).await.map_err(|_| Error::msg(format!("Download stalled: {url}")))?;
        let Some(chunk) = next else { break };
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
        if item.size.is_some() {
            progress.done_bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }
    }
    file.flush().await?;
    drop(file);
    let fail = |message: String, counted: u64| {
        let _ = std::fs::remove_file(&part);
        if item.size.is_some() {
            progress.done_bytes.fetch_sub(counted, Ordering::Relaxed);
        }
        Err(Error::msg(message))
    };
    if let Some(size) = item.size {
        if size != written {
            return fail(format!("Size mismatch for {url}: expected {size}, got {written}"), written);
        }
    }
    if let Some(expected) = &item.sha1 {
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            return fail(format!("Checksum mismatch for {url}"), written);
        }
    }
    tokio::fs::rename(&part, &item.path).await?;
    Ok(())
}

async fn download_one(item: &DownloadItem, progress: &Progress, retries: u32) -> Result<()> {
    let mut last_error = Error::msg("No download source");
    let urls: Vec<&String> = std::iter::once(&item.url).chain(item.fallback_urls.iter()).collect();
    for url in urls {
        for attempt in 0..=retries {
            match fetch_once(url, item, progress).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = e;
                    if attempt < retries {
                        tokio::time::sleep(Duration::from_millis(400 * 2u64.pow(attempt))).await;
                    }
                }
            }
        }
    }
    Err(last_error)
}

pub async fn download_all(items: Vec<DownloadItem>, progress: Arc<Progress>, verify_hash: bool) -> Result<()> {
    let settings = storage::settings();
    let concurrency = settings.downloads.concurrency.max(1) as usize;
    let retries = settings.downloads.retries;
    let mut unique = std::collections::HashMap::new();
    for item in items {
        unique.entry(item.path.clone()).or_insert(item);
    }
    let candidates: Vec<DownloadItem> = unique.into_values().collect();
    let pending: Vec<DownloadItem> = tokio::task::spawn_blocking(move || {
        candidates.into_iter().filter(|item| needs_download(item, verify_hash)).collect()
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?;
    progress.total_files.fetch_add(pending.len() as u64, Ordering::Relaxed);
    progress
        .total_bytes
        .fetch_add(pending.iter().filter_map(|i| i.size).sum::<u64>(), Ordering::Relaxed);
    let results: Vec<Result<()>> = futures_util::stream::iter(pending.into_iter().map(|item| {
        let progress = progress.clone();
        async move {
            let outcome = download_one(&item, &progress, retries).await;
            progress.done_files.fetch_add(1, Ordering::Relaxed);
            outcome
        }
    }))
    .buffer_unordered(concurrency)
    .collect()
    .await;
    let failures: Vec<String> = results.into_iter().filter_map(|r| r.err().map(|e| e.0)).collect();
    if let Some(first) = failures.first() {
        if failures.len() == 1 {
            return Err(Error::msg(first.clone()));
        }
        return Err(Error::msg(format!("{} downloads failed. First error: {first}", failures.len())));
    }
    Ok(())
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            total += if meta.is_dir() { dir_size(&entry.path()) } else { meta.len() };
        }
    }
    total
}

fn collect_files(path: &Path, out: &mut Vec<(PathBuf, u64, SystemTime)>) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                collect_files(&entry.path(), out);
            } else {
                out.push((entry.path(), meta.len(), meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
            }
        }
    }
}

pub fn prune_cache(limit_bytes: u64) -> u64 {
    let mut files = Vec::new();
    collect_files(&storage::cache_dir(), &mut files);
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= limit_bytes {
        return total;
    }
    files.sort_by_key(|f| f.2);
    for (path, size, _) in files {
        if total <= limit_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
    total
}

pub fn clear_cache() -> Result<()> {
    let dir = storage::cache_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_known_value() {
        assert_eq!(sha1_bytes(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(sha1_file(&p).unwrap(), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn needs_download_checks_size_and_hash() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        let mut item = DownloadItem::new("x", p.clone(), Some("a9993e364706816aba3e25717850c26c9cd0d89d".into()), Some(3));
        assert!(needs_download(&item, false));
        std::fs::write(&p, b"abc").unwrap();
        assert!(!needs_download(&item, true));
        std::fs::write(&p, b"abd").unwrap();
        assert!(!needs_download(&item, false));
        assert!(needs_download(&item, true));
        item.size = Some(4);
        assert!(needs_download(&item, false));
    }
}
