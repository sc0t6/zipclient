use crate::error::Result;
use crate::net;
use serde::Serialize;

const NEWS_URL: &str = "https://launchercontent.mojang.com/v2/news.json";
const CONTENT_BASE: &str = "https://launchercontent.mojang.com";

#[derive(Serialize)]
pub struct NewsItem {
    pub title: String,
    pub text: String,
    pub date: String,
    pub url: String,
    pub image: String,
}

pub async fn minecraft_news() -> Result<Vec<NewsItem>> {
    let feed: serde_json::Value = net::get_json_cached(NEWS_URL, "mojang-news", 3600).await?;
    let entries = feed["entries"].as_array().cloned().unwrap_or_default();
    let mut items: Vec<NewsItem> = entries
        .iter()
        .filter(|e| e["category"].as_str() == Some("Minecraft: Java Edition"))
        .filter_map(|e| {
            let url = e["readMoreLink"].as_str()?.to_string();
            if !url.starts_with("https://") {
                return None;
            }
            let image = e["newsPageImage"]["url"].as_str().or(e["playPageImage"]["url"].as_str()).map(|p| format!("{CONTENT_BASE}{p}")).unwrap_or_default();
            Some(NewsItem {
                title: e["title"].as_str().unwrap_or("").to_string(),
                text: e["text"].as_str().unwrap_or("").to_string(),
                date: e["date"].as_str().unwrap_or("").to_string(),
                url,
                image,
            })
        })
        .collect();
    items.sort_by(|a, b| b.date.cmp(&a.date));
    items.truncate(12);
    Ok(items)
}

pub fn zip_repo() -> Option<String> {
    std::env::var("ZIP_GITHUB_REPO").ok().or(option_env!("ZIP_GITHUB_REPO").map(String::from)).filter(|r| r.split('/').count() == 2)
}

pub async fn zip_news() -> Result<Vec<NewsItem>> {
    let Some(repo) = zip_repo() else { return Ok(Vec::new()) };
    let releases: Vec<serde_json::Value> = net::get_json_cached(&format!("https://api.github.com/repos/{repo}/releases?per_page=10"), "zip-news", 3600).await?;
    Ok(releases
        .iter()
        .filter(|r| r["draft"].as_bool() != Some(true))
        .filter_map(|r| {
            let url = r["html_url"].as_str()?.to_string();
            let title = r["name"].as_str().filter(|n| !n.is_empty()).or(r["tag_name"].as_str())?.to_string();
            let text = r["body"].as_str().unwrap_or("").lines().map(|l| l.trim_start_matches(['#', '-', '*', ' '])).find(|l| !l.is_empty()).unwrap_or("").to_string();
            let date = r["published_at"].as_str().unwrap_or("").chars().take(10).collect();
            Some(NewsItem { title, text, date, url, image: String::new() })
        })
        .collect())
}
