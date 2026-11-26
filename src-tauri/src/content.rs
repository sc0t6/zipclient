use crate::error::{Error, Result};
use crate::instances::{self, Instance};
use crate::net::{self, DownloadItem};
use crate::storage;
use crate::tasks::Reporter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";
const INDEX_FILE: &str = ".zip-content.json";

pub fn folder_for(kind: &str) -> Result<&'static str> {
    match kind {
        "mod" => Ok("mods"),
        "resourcepack" => Ok("resourcepacks"),
        "shader" => Ok("shaderpacks"),
        other => Err(Error::msg(format!("Unknown content type: {other}"))),
    }
}

fn folder_in(instance: &Instance, kind: &str) -> Result<&'static str> {
    if instance.loader == "horizon" && kind == "mod" {
        return Ok("horizon-mods");
    }
    folder_for(kind)
}

fn kind_for_folder(folder: &str) -> &'static str {
    match folder {
        "resourcepacks" => "resourcepack",
        "shaderpacks" => "shader",
        _ => "mod",
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct IndexEntry {
    pub source: String,
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub icon_url: String,
    pub version_number: String,
    pub sha1: String,
    pub size: u64,
    pub local_checked: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct ContentIndex {
    #[serde(default)]
    files: HashMap<String, IndexEntry>,
}

fn index_path(game_dir: &Path) -> PathBuf {
    game_dir.join(INDEX_FILE)
}

fn load_index(game_dir: &Path) -> ContentIndex {
    storage::read_json(&index_path(game_dir)).ok().flatten().unwrap_or_default()
}

fn save_index(game_dir: &Path, index: &ContentIndex) -> Result<()> {
    storage::write_json(&index_path(game_dir), index)
}

fn canonical_key(relative: &str) -> String {
    relative.trim_end_matches(".disabled").replace('\\', "/")
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub source: String,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: String,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub latest_version: String,
    pub installable: bool,
    pub note: String,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub total: u64,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub source: String,
    pub kind: String,
    pub query: String,
    pub minecraft: String,
    pub loader: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub limit: u32,
}

pub fn curseforge_key() -> Option<String> {
    std::env::var("ZIP_CURSEFORGE_KEY").ok().or(option_env!("ZIP_CURSEFORGE_KEY").map(str::to_string)).filter(|s| !s.trim().is_empty())
}

fn curseforge_class(kind: &str) -> u32 {
    match kind {
        "resourcepack" => 12,
        "shader" => 6552,
        "modpack" => 4471,
        _ => 6,
    }
}

fn curseforge_loader(loader: &str) -> Option<u32> {
    match loader {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

async fn curseforge_get(path: &str, query: &[(&str, String)]) -> Result<serde_json::Value> {
    let key = curseforge_key().ok_or_else(|| Error::msg("CurseForge is not available in this build of Zip."))?;
    Ok(net::client()
        .get(format!("{CURSEFORGE}{path}"))
        .query(query)
        .header("x-api-key", key)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

pub async fn curseforge_post(path: &str, body: serde_json::Value) -> Result<serde_json::Value> {
    let key = curseforge_key().ok_or_else(|| Error::msg("CurseForge is not available in this build of Zip."))?;
    Ok(net::client()
        .post(format!("{CURSEFORGE}{path}"))
        .header("x-api-key", key)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn search_horizon(q: &SearchQuery) -> Result<SearchResult> {
    let needle = q.query.trim().to_lowercase();
    let mut mods = crate::horizon::mod_index().await?;
    let latest = |m: &crate::horizon::ModManifest| m.versions.iter().map(|v| v.published.clone()).max().unwrap_or_default();
    match q.sort.as_str() {
        "downloads" => mods.sort_by(|a, b| b.downloads.cmp(&a.downloads)),
        "newest" | "updated" => mods.sort_by_key(|m| std::cmp::Reverse(latest(m))),
        _ if needle.is_empty() => mods.sort_by_key(|m| (m.kind != "mod", std::cmp::Reverse(m.downloads))),
        _ => mods.sort_by_key(|m| (!m.name.to_lowercase().starts_with(&needle), m.kind != "mod", std::cmp::Reverse(m.downloads))),
    }
    let hits: Vec<SearchHit> = mods
        .into_iter()
        .filter(|m| if q.kind == "mod" { m.kind == "mod" || m.kind == "native-mod" } else { m.kind == q.kind })
        .filter(|m| needle.is_empty() || m.name.to_lowercase().contains(&needle) || m.summary.to_lowercase().contains(&needle) || m.authors.iter().any(|a| a.to_lowercase().contains(&needle)))
        .filter(|m| m.versions.iter().any(|v| q.minecraft.is_empty() || v.game == q.minecraft))
        .filter(|m| q.category.is_empty() || m.categories.iter().any(|c| *c == q.category))
        .map(|m| SearchHit {
            source: "horizon".into(),
            slug: m.id.clone(),
            title: m.name.clone(),
            description: m.summary.lines().next().unwrap_or("").to_string(),
            author: m.authors.join(", "),
            icon_url: m.icon.clone(),
            downloads: m.downloads,
            categories: std::iter::once(if m.kind == "mod" { "content".to_string() } else { "native".to_string() }).chain(m.categories.iter().cloned()).collect(),
            latest_version: m.versions.first().map(|v| v.version.clone()).unwrap_or_default(),
            installable: m.kind == "mod",
            note: if m.kind == "mod" { String::new() } else { "Native mod: built into the game by the Horizon launcher".into() },
            id: m.id,
        })
        .collect();
    let total = hits.len() as u64;
    Ok(SearchResult { hits: hits.into_iter().skip(q.offset as usize).take(if q.limit == 0 { 20 } else { q.limit as usize }).collect(), total })
}

pub async fn search(q: SearchQuery) -> Result<SearchResult> {
    if q.source == "horizon" {
        return search_horizon(&q).await;
    }
    let limit = if q.limit == 0 { 20 } else { q.limit.min(50) };
    if q.source == "curseforge" {
        let mut params = vec![
            ("gameId", "432".to_string()),
            ("classId", curseforge_class(&q.kind).to_string()),
            ("searchFilter", q.query.clone()),
            ("index", q.offset.to_string()),
            ("pageSize", limit.to_string()),
            ("sortField", match q.sort.as_str() { "updated" => "3", "newest" => "11", "downloads" => "6", _ => "2" }.to_string()),
            ("sortOrder", "desc".to_string()),
        ];
        if !q.minecraft.is_empty() {
            params.push(("gameVersion", q.minecraft.clone()));
        }
        if q.kind == "mod" {
            if let Some(l) = curseforge_loader(&q.loader) {
                params.push(("modLoaderType", l.to_string()));
            }
        }
        if !q.category.is_empty() {
            params.push(("categoryId", q.category.clone()));
        }
        let body = curseforge_get("/mods/search", &params).await?;
        let hits = body["data"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|m| SearchHit {
                        source: "curseforge".into(),
                        id: m["id"].as_u64().unwrap_or(0).to_string(),
                        slug: m["slug"].as_str().unwrap_or("").into(),
                        title: m["name"].as_str().unwrap_or("").into(),
                        description: m["summary"].as_str().unwrap_or("").into(),
                        author: m["authors"][0]["name"].as_str().unwrap_or("").into(),
                        icon_url: m["logo"]["thumbnailUrl"].as_str().unwrap_or("").into(),
                        downloads: m["downloadCount"].as_f64().unwrap_or(0.0) as u64,
                        categories: m["categories"].as_array().map(|c| c.iter().filter_map(|x| x["name"].as_str().map(String::from)).collect()).unwrap_or_default(),
                        latest_version: String::new(),
                        installable: true,
                        note: String::new(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Ok(SearchResult { hits, total: body["pagination"]["totalCount"].as_u64().unwrap_or(0) });
    }
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", q.kind)]];
    if !q.minecraft.is_empty() && q.kind != "modpack" {
        facets.push(vec![format!("versions:{}", q.minecraft)]);
    }
    if q.kind == "mod" && !q.loader.is_empty() && q.loader != "vanilla" {
        facets.push(vec![format!("categories:{}", q.loader)]);
    }
    if !q.category.is_empty() {
        facets.push(vec![format!("categories:{}", q.category)]);
    }
    let index = match q.sort.as_str() {
        "downloads" | "follows" | "newest" | "updated" => q.sort.as_str(),
        _ => "relevance",
    };
    let body: serde_json::Value = net::client()
        .get(format!("{MODRINTH}/search"))
        .query(&[
            ("query", q.query.clone()),
            ("facets", serde_json::to_string(&facets)?),
            ("index", index.to_string()),
            ("offset", q.offset.to_string()),
            ("limit", limit.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let hits = body["hits"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|h| SearchHit {
                    source: "modrinth".into(),
                    id: h["project_id"].as_str().unwrap_or("").into(),
                    slug: h["slug"].as_str().unwrap_or("").into(),
                    title: h["title"].as_str().unwrap_or("").into(),
                    description: h["description"].as_str().unwrap_or("").into(),
                    author: h["author"].as_str().unwrap_or("").into(),
                    icon_url: h["icon_url"].as_str().unwrap_or("").into(),
                    downloads: h["downloads"].as_u64().unwrap_or(0),
                    categories: h["display_categories"].as_array().map(|c| c.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
                    latest_version: h["latest_version"].as_str().unwrap_or("").into(),
                    installable: true,
                    note: String::new(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SearchResult { hits, total: body["total_hits"].as_u64().unwrap_or(0) })
}

#[derive(Serialize, Clone)]
pub struct Category {
    pub id: String,
    pub name: String,
}

pub async fn categories(source: &str, kind: &str) -> Result<Vec<Category>> {
    if source == "horizon" {
        let mut names: Vec<String> = crate::horizon::mod_index().await?.into_iter().flat_map(|m| m.categories).collect::<HashSet<_>>().into_iter().collect();
        names.sort();
        return Ok(names.into_iter().map(|n| Category { name: capitalize(&n), id: n }).collect());
    }
    if source == "curseforge" {
        let body = curseforge_get("/categories", &[("gameId", "432".into()), ("classId", curseforge_class(kind).to_string())]).await?;
        return Ok(body["data"]
            .as_array()
            .map(|c| {
                c.iter()
                    .map(|x| Category { id: x["id"].as_u64().unwrap_or(0).to_string(), name: x["name"].as_str().unwrap_or("").into() })
                    .collect()
            })
            .unwrap_or_default());
    }
    let all: Vec<serde_json::Value> = net::get_json_cached(&format!("{MODRINTH}/tag/category"), "modrinth-categories", 24 * 3600).await?;
    let mut out: Vec<Category> = all
        .iter()
        .filter(|c| c["project_type"].as_str() == Some(kind) && c["header"].as_str() == Some("categories"))
        .filter_map(|c| c["name"].as_str().map(|n| Category { id: n.into(), name: capitalize(n) }))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn capitalize(s: &str) -> String {
    s.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date: String,
    pub file_name: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub channel: String,
    pub dependencies: Vec<Dependency>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub project_id: String,
    pub version_id: String,
    pub kind: String,
}

fn modrinth_version(v: &serde_json::Value) -> Option<ContentVersion> {
    let files = v["files"].as_array()?;
    let file = files.iter().find(|f| f["primary"].as_bool() == Some(true)).or_else(|| files.first())?;
    Some(ContentVersion {
        id: v["id"].as_str()?.into(),
        name: v["name"].as_str().unwrap_or("").into(),
        version_number: v["version_number"].as_str().unwrap_or("").into(),
        game_versions: v["game_versions"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        loaders: v["loaders"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        date: v["date_published"].as_str().unwrap_or("").into(),
        file_name: file["filename"].as_str()?.into(),
        url: file["url"].as_str()?.into(),
        sha1: file["hashes"]["sha1"].as_str().unwrap_or("").into(),
        size: file["size"].as_u64().unwrap_or(0),
        channel: v["version_type"].as_str().unwrap_or("release").into(),
        dependencies: v["dependencies"]
            .as_array()
            .map(|deps| {
                deps.iter()
                    .map(|d| Dependency {
                        project_id: d["project_id"].as_str().unwrap_or("").into(),
                        version_id: d["version_id"].as_str().unwrap_or("").into(),
                        kind: d["dependency_type"].as_str().unwrap_or("").into(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn curseforge_version(f: &serde_json::Value) -> Option<ContentVersion> {
    let id = f["id"].as_u64()?;
    let file_name = f["fileName"].as_str()?.to_string();
    let url = f["downloadUrl"].as_str().map(String::from).unwrap_or_default();
    let versions: Vec<String> = f["gameVersions"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    let loaders = versions.iter().filter(|v| matches!(v.to_lowercase().as_str(), "forge" | "fabric" | "quilt" | "neoforge")).map(|v| v.to_lowercase()).collect();
    Some(ContentVersion {
        id: id.to_string(),
        name: f["displayName"].as_str().unwrap_or(&file_name).into(),
        version_number: f["displayName"].as_str().unwrap_or(&file_name).into(),
        game_versions: versions.into_iter().filter(|v| v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)).collect(),
        loaders,
        date: f["fileDate"].as_str().unwrap_or("").into(),
        url,
        sha1: f["hashes"].as_array().and_then(|h| h.iter().find(|x| x["algo"].as_u64() == Some(1))).and_then(|x| x["value"].as_str()).unwrap_or("").into(),
        size: f["fileLength"].as_u64().unwrap_or(0),
        channel: match f["releaseType"].as_u64() { Some(2) => "beta", Some(3) => "alpha", _ => "release" }.into(),
        file_name,
        dependencies: f["dependencies"]
            .as_array()
            .map(|deps| {
                deps.iter()
                    .map(|d| Dependency {
                        project_id: d["modId"].as_u64().unwrap_or(0).to_string(),
                        version_id: String::new(),
                        kind: match d["relationType"].as_u64() {
                            Some(3) => "required",
                            Some(2) => "optional",
                            Some(5) => "incompatible",
                            _ => "other",
                        }
                        .into(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub async fn versions(source: &str, project_id: &str, kind: &str, minecraft: &str, loader: &str) -> Result<Vec<ContentVersion>> {
    if source == "horizon" {
        let index = crate::horizon::mod_index().await?;
        let manifest = index.into_iter().find(|m| m.id == project_id).ok_or_else(|| Error::msg("Unknown Horizon mod"))?;
        return Ok(manifest
            .versions
            .iter()
            .filter(|v| minecraft.is_empty() || v.game == minecraft)
            .map(|v| ContentVersion {
                id: v.version.clone(),
                name: manifest.name.clone(),
                version_number: v.version.clone(),
                game_versions: vec![v.game.clone()],
                loaders: vec!["horizon".into()],
                date: v.published.clone(),
                file_name: v.file.name().to_string(),
                url: String::new(),
                sha1: String::new(),
                size: 0,
                channel: "release".into(),
                dependencies: Vec::new(),
            })
            .collect());
    }
    if source == "curseforge" {
        let mut params = vec![("pageSize", "50".to_string())];
        if !minecraft.is_empty() {
            params.push(("gameVersion", minecraft.to_string()));
        }
        if kind == "mod" {
            if let Some(l) = curseforge_loader(loader) {
                params.push(("modLoaderType", l.to_string()));
            }
        }
        let body = curseforge_get(&format!("/mods/{project_id}/files"), &params).await?;
        return Ok(body["data"].as_array().map(|a| a.iter().filter_map(curseforge_version).collect()).unwrap_or_default());
    }
    let mut params: Vec<(&str, String)> = Vec::new();
    if !minecraft.is_empty() {
        params.push(("game_versions", serde_json::to_string(&[minecraft])?));
    }
    if kind == "mod" && loader != "vanilla" && !loader.is_empty() {
        params.push(("loaders", serde_json::to_string(&[loader])?));
    }
    let body: Vec<serde_json::Value> = net::client()
        .get(format!("{MODRINTH}/project/{project_id}/version"))
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(body.iter().filter_map(modrinth_version).collect())
}

async fn modrinth_version_by_id(id: &str) -> Result<ContentVersion> {
    let v: serde_json::Value = net::get_json(&format!("{MODRINTH}/version/{id}")).await?;
    modrinth_version(&v).ok_or_else(|| Error::msg("Modrinth returned an incomplete version"))
}

async fn project_meta(source: &str, id: &str) -> (String, String) {
    if source == "curseforge" {
        if let Ok(v) = curseforge_get(&format!("/mods/{id}"), &[]).await {
            return (v["data"]["name"].as_str().unwrap_or("").into(), v["data"]["logo"]["thumbnailUrl"].as_str().unwrap_or("").into());
        }
        return (String::new(), String::new());
    }
    match net::get_json::<serde_json::Value>(&format!("{MODRINTH}/project/{id}")).await {
        Ok(p) => (p["title"].as_str().unwrap_or("").into(), p["icon_url"].as_str().unwrap_or("").into()),
        Err(_) => (String::new(), String::new()),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub url: String,
    pub title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    pub title: String,
    pub summary: String,
    pub body: String,
    pub html: bool,
    pub icon_url: String,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub gallery: Vec<GalleryImage>,
    pub updated: String,
}

fn strings(value: &serde_json::Value, key: Option<&str>) -> Vec<String> {
    value.as_array().map(|a| a.iter().filter_map(|x| key.map_or(x, |k| &x[k]).as_str().map(String::from)).collect()).unwrap_or_default()
}

pub async fn project(source: &str, id: &str) -> Result<ProjectDetails> {
    if source == "horizon" {
        let m = crate::horizon::mod_index().await?.into_iter().find(|m| m.id == id).ok_or_else(|| Error::msg("Unknown Horizon mod"))?;
        return Ok(ProjectDetails {
            summary: m.summary.lines().next().unwrap_or("").to_string(),
            updated: m.versions.iter().map(|v| v.published.clone()).max().unwrap_or_default(),
            title: m.name,
            body: m.summary,
            html: false,
            icon_url: m.icon,
            downloads: m.downloads,
            categories: m.categories,
            gallery: Vec::new(),
        });
    }
    if source == "curseforge" {
        let (info_path, description_path) = (format!("/mods/{id}"), format!("/mods/{id}/description"));
        let (info, description) = futures_util::join!(curseforge_get(&info_path, &[]), curseforge_get(&description_path, &[]));
        let d = &info?["data"];
        return Ok(ProjectDetails {
            title: d["name"].as_str().unwrap_or("").into(),
            summary: d["summary"].as_str().unwrap_or("").into(),
            body: description.ok().and_then(|v| v["data"].as_str().map(String::from)).unwrap_or_default(),
            html: true,
            icon_url: d["logo"]["thumbnailUrl"].as_str().unwrap_or("").into(),
            downloads: d["downloadCount"].as_f64().unwrap_or(0.0) as u64,
            categories: strings(&d["categories"], Some("name")),
            gallery: d["screenshots"]
                .as_array()
                .map(|a| a.iter().filter_map(|s| Some(GalleryImage { url: s["url"].as_str()?.into(), title: s["title"].as_str().unwrap_or("").into() })).collect())
                .unwrap_or_default(),
            updated: d["dateModified"].as_str().unwrap_or("").into(),
        });
    }
    let p: serde_json::Value = net::get_json(&format!("{MODRINTH}/project/{id}")).await?;
    let mut gallery: Vec<&serde_json::Value> = p["gallery"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    gallery.sort_by_key(|g| (g["featured"].as_bool() != Some(true), g["ordering"].as_i64().unwrap_or(0)));
    Ok(ProjectDetails {
        title: p["title"].as_str().unwrap_or("").into(),
        summary: p["description"].as_str().unwrap_or("").into(),
        body: p["body"].as_str().unwrap_or("").into(),
        html: false,
        icon_url: p["icon_url"].as_str().unwrap_or("").into(),
        downloads: p["downloads"].as_u64().unwrap_or(0),
        categories: strings(&p["categories"], None),
        gallery: gallery.into_iter().filter_map(|g| Some(GalleryImage { url: g["url"].as_str()?.into(), title: g["title"].as_str().unwrap_or("").into() })).collect(),
        updated: p["updated"].as_str().unwrap_or("").into(),
    })
}

pub async fn project_icon(source: &str, id: &str) -> Option<String> {
    Some(project_meta(source, id).await.1).filter(|u| !u.is_empty())
}

pub async fn modrinth_project_by_hash(sha1: &str) -> Option<String> {
    let v: serde_json::Value = net::get_json(&format!("{MODRINTH}/version_file/{sha1}?algorithm=sha1")).await.ok()?;
    v["project_id"].as_str().map(String::from)
}

pub async fn modpack_icon_by_name(source: &str, name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let query = SearchQuery {
        source: source.into(),
        kind: "modpack".into(),
        query: name.into(),
        minecraft: String::new(),
        loader: String::new(),
        category: String::new(),
        sort: String::new(),
        offset: 0,
        limit: 10,
    };
    search(query).await.ok()?.hits.into_iter().find(|h| h.title.trim().eq_ignore_ascii_case(name)).map(|h| h.icon_url).filter(|u| !u.is_empty())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
}

fn installed_projects(game_dir: &Path) -> HashSet<String> {
    load_index(game_dir)
        .files
        .iter()
        .filter(|(path, _)| game_dir.join(path).exists() || game_dir.join(format!("{path}.disabled")).exists())
        .map(|(_, e)| e.project_id.clone())
        .collect()
}

pub async fn install(
    instance_id: &str,
    source: &str,
    project_id: &str,
    version_id: Option<String>,
    kind: &str,
    reporter: &Reporter,
) -> Result<InstallOutcome> {
    let instance = instances::get(instance_id)?;
    let game_dir = instances::game_dir(&instance)?;
    if source == "horizon" {
        let index = crate::horizon::mod_index().await?;
        let manifest = index.into_iter().find(|m| m.id == project_id).ok_or_else(|| Error::msg("Unknown Horizon mod"))?;
        let chosen = manifest
            .versions
            .iter()
            .find(|v| version_id.as_deref().map(|id| id == v.version).unwrap_or(v.game == instance.minecraft))
            .cloned()
            .ok_or_else(|| Error::msg(format!("{} has no version for Minecraft {}", manifest.name, instance.minecraft)))?;
        let relative = crate::horizon::install_mod(&game_dir, &manifest, &chosen, reporter).await?;
        let mut index = load_index(&game_dir);
        index.files.retain(|path, e| !(e.project_id == manifest.id && *path != relative));
        index.files.insert(relative, IndexEntry { source: "horizon".into(), project_id: manifest.id.clone(), version_id: chosen.version.clone(), title: manifest.name.clone(), icon_url: manifest.icon.clone(), version_number: chosen.version.clone(), sha1: String::new(), size: 0, local_checked: false });
        save_index(&game_dir, &index)?;
        return Ok(InstallOutcome { installed: vec![manifest.name], skipped: Vec::new() });
    }
    let mut outcome = InstallOutcome { installed: Vec::new(), skipped: Vec::new() };
    let mut queue: Vec<(String, Option<String>, String)> = vec![(project_id.to_string(), version_id, kind.to_string())];
    let mut visited = HashSet::new();
    let already = installed_projects(&game_dir);
    while let Some((project, version, kind)) = queue.pop() {
        if !visited.insert(project.clone()) {
            continue;
        }
        if project != project_id && already.contains(&project) {
            continue;
        }
        let chosen = match (&version, source) {
            (Some(v), "modrinth") if !v.is_empty() => modrinth_version_by_id(v).await?,
            (Some(v), _) if !v.is_empty() => versions(source, &project, &kind, &instance.minecraft, &instance.loader)
                .await?
                .into_iter()
                .find(|x| &x.id == v)
                .ok_or_else(|| Error::msg("That version is not available"))?,
            _ => {
                let all = versions(source, &project, &kind, &instance.minecraft, &instance.loader).await?;
                let stable = all.iter().position(|v| v.channel == "release").unwrap_or(0);
                all.into_iter().nth(stable)
            }
                .ok_or_else(|| Error::msg(format!("No version is compatible with {} {}", instance.loader, instance.minecraft)))?,
        };
        if chosen.url.is_empty() {
            outcome.skipped.push(format!("{} (the author disabled third-party downloads)", chosen.file_name));
            continue;
        }
        let file_kind = if kind == "mod" && !chosen.file_name.ends_with(".jar") && chosen.loaders.iter().any(|l| l == "iris" || l == "optifine") {
            "shader".to_string()
        } else {
            kind.clone()
        };
        let folder = folder_in(&instance, &file_kind)?;
        let safe_name = Path::new(&chosen.file_name).file_name().map(|f| f.to_string_lossy().to_string()).ok_or_else(|| Error::msg("Invalid file name"))?;
        let relative = format!("{folder}/{safe_name}");
        let target = storage::safe_join(&game_dir, &relative)?;
        let existed = target.exists();
        reporter.phase(&format!("Downloading {safe_name}"));
        net::download_all(
            vec![DownloadItem::new(chosen.url.clone(), target.clone(), (!chosen.sha1.is_empty()).then(|| chosen.sha1.clone()), (chosen.size > 0).then_some(chosen.size))],
            reporter.progress(),
            true,
        )
        .await?;
        let mut index = load_index(&game_dir);
        let previous: Vec<String> = index
            .files
            .iter()
            .filter(|(path, e)| e.project_id == project && **path != relative)
            .map(|(p, _)| p.clone())
            .collect();
        if file_kind == "mod" && !existed && matches!(instance.loader.as_str(), "fabric" | "quilt") && safe_name.ends_with(".jar") {
            let replaced: Vec<PathBuf> = previous.iter().map(|p| game_dir.join(p)).collect();
            if let Some(problem) = crate::modcheck::conflict(&game_dir.join(folder), &target, &replaced) {
                let _ = std::fs::remove_file(&target);
                return Err(Error::msg(problem));
            }
        }
        for old in previous {
            let _ = std::fs::remove_file(game_dir.join(&old));
            let _ = std::fs::remove_file(game_dir.join(format!("{old}.disabled")));
            index.files.remove(&old);
        }
        let (title, icon) = project_meta(source, &project).await;
        index.files.insert(
            relative.clone(),
            IndexEntry {
                source: source.into(),
                project_id: project.clone(),
                version_id: chosen.id.clone(),
                title: if title.is_empty() { safe_name.clone() } else { title.clone() },
                icon_url: icon,
                version_number: chosen.version_number.clone(),
                sha1: chosen.sha1.clone(),
                size: chosen.size,
                local_checked: false,
            },
        );
        save_index(&game_dir, &index)?;
        outcome.installed.push(if title.is_empty() { safe_name } else { title });
        if file_kind == "mod" {
            for dep in chosen.dependencies.iter().filter(|d| d.kind == "required") {
                if dep.project_id.is_empty() && !dep.version_id.is_empty() && source == "modrinth" {
                    if let Ok(v) = modrinth_version_by_id(&dep.version_id).await {
                        let resolved: serde_json::Value = net::get_json(&format!("{MODRINTH}/version/{}", v.id)).await?;
                        if let Some(pid) = resolved["project_id"].as_str() {
                            queue.push((pid.to_string(), Some(dep.version_id.clone()), "mod".into()));
                        }
                    }
                } else if !dep.project_id.is_empty() && dep.project_id != "0" {
                    queue.push((dep.project_id.clone(), None, "mod".into()));
                }
            }
        }
    }
    Ok(outcome)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFile {
    pub path: String,
    pub file_name: String,
    pub kind: String,
    pub enabled: bool,
    pub size: u64,
    pub title: String,
    pub version: String,
    pub project_id: String,
    pub version_id: String,
    pub source: String,
    pub icon_url: String,
}

pub struct LocalMeta {
    pub name: String,
    pub version: String,
    pub icon: Option<Vec<u8>>,
}

const ICON_LIMIT: usize = 512 * 1024;

pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub fn data_url(bytes: &[u8]) -> Option<String> {
    use base64::Engine;
    let mime = image_mime(bytes)?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn icon_cache() -> PathBuf {
    storage::cache_dir().join("mod-icons")
}

fn icon_src(url: &str) -> String {
    match url.strip_prefix("local:") {
        Some(name) if !name.contains(['/', '\\']) => std::fs::read(icon_cache().join(name)).ok().and_then(|b| data_url(&b)).unwrap_or_default(),
        Some(_) => String::new(),
        None => url.to_string(),
    }
}

fn zip_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str, limit: usize) -> Option<Vec<u8>> {
    let name = name.trim_start_matches(['/', '.']).trim_start_matches('/');
    let mut entry = archive.by_name(name).ok()?;
    if entry.size() as usize > limit {
        return None;
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    std::io::Read::read_to_end(&mut entry, &mut bytes).ok()?;
    Some(bytes)
}

fn zip_text(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    zip_entry(archive, name, 4 * 1024 * 1024).map(|b| String::from_utf8_lossy(&b).trim_start_matches('\u{feff}').to_string())
}

fn fabric_icon(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(sizes) => {
            let mut options: Vec<(u32, String)> = sizes.iter().filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_string()))).collect();
            options.sort_by_key(|(size, _)| (*size < 64, (*size as i64 - 128).abs()));
            options.into_iter().next().map(|(_, path)| path)
        }
        _ => None,
    }
}

fn manifest_version(archive: &mut zip::ZipArchive<std::fs::File>) -> String {
    zip_text(archive, "META-INF/MANIFEST.MF")
        .and_then(|m| m.lines().find_map(|l| l.strip_prefix("Implementation-Version:").map(|v| v.trim().to_string())))
        .unwrap_or_default()
}

pub fn mod_identity(path: &Path) -> Option<(String, String)> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let pick = |id: Option<&str>, name: Option<&str>| {
        let id = id.map(str::trim).filter(|i| !i.is_empty())?.to_ascii_lowercase();
        let name = name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(id.as_str()).to_string();
        Some((id, name))
    };
    if let Some(json) = zip_text(&mut archive, "fabric.mod.json").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        return pick(json["id"].as_str(), json["name"].as_str());
    }
    if let Some(json) = zip_text(&mut archive, "quilt.mod.json").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        let loader = &json["quilt_loader"];
        return pick(loader["id"].as_str(), loader["metadata"]["name"].as_str());
    }
    let text = zip_text(&mut archive, "META-INF/neoforge.mods.toml").or_else(|| zip_text(&mut archive, "META-INF/mods.toml"))?;
    let toml = toml::from_str::<toml::Value>(&text).ok()?;
    let first = toml.get("mods")?.as_array()?.first()?;
    pick(first.get("modId").and_then(|v| v.as_str()), first.get("displayName").and_then(|v| v.as_str()))
}

pub fn local_meta(path: &Path) -> Option<LocalMeta> {
    if path.is_dir() {
        let icon = std::fs::read(path.join("pack.png")).ok().filter(|b| b.len() <= ICON_LIMIT && image_mime(b).is_some());
        return Some(LocalMeta { name: String::new(), version: String::new(), icon });
    }
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut meta = LocalMeta { name: String::new(), version: String::new(), icon: None };
    let mut icon_path: Option<String> = None;
    if let Some(json) = zip_text(&mut archive, "fabric.mod.json").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        meta.name = json["name"].as_str().or(json["id"].as_str()).unwrap_or("").to_string();
        meta.version = json["version"].as_str().unwrap_or("").to_string();
        icon_path = fabric_icon(&json["icon"]);
    } else if let Some(json) = zip_text(&mut archive, "quilt.mod.json").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        let loader = &json["quilt_loader"];
        meta.name = loader["metadata"]["name"].as_str().or(loader["id"].as_str()).unwrap_or("").to_string();
        meta.version = loader["version"].as_str().unwrap_or("").to_string();
        icon_path = fabric_icon(&loader["metadata"]["icon"]);
    } else if let Some(text) = zip_text(&mut archive, "META-INF/neoforge.mods.toml").or_else(|| zip_text(&mut archive, "META-INF/mods.toml")) {
        if let Ok(toml) = toml::from_str::<toml::Value>(&text) {
            let first = toml.get("mods").and_then(|m| m.as_array()).and_then(|m| m.first());
            meta.name = first.and_then(|m| m.get("displayName")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            meta.version = first.and_then(|m| m.get("version")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            icon_path = first.and_then(|m| m.get("logoFile")).or_else(|| toml.get("logoFile")).and_then(|v| v.as_str()).map(String::from);
        }
        if meta.version.contains("${") {
            meta.version = manifest_version(&mut archive);
        }
    } else if let Some(json) = zip_text(&mut archive, "mcmod.info").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        let first = if json.is_array() { &json[0] } else { &json["modList"][0] };
        meta.name = first["name"].as_str().unwrap_or("").to_string();
        meta.version = first["version"].as_str().unwrap_or("").to_string();
        icon_path = first["logoFile"].as_str().filter(|s| !s.is_empty()).map(String::from);
    }
    if meta.version.contains("${") {
        meta.version.clear();
    }
    let candidates = icon_path.into_iter().chain(["pack.png".to_string()]);
    for candidate in candidates {
        if let Some(bytes) = zip_entry(&mut archive, &candidate, ICON_LIMIT).filter(|b| image_mime(b).is_some()) {
            meta.icon = Some(bytes);
            break;
        }
    }
    Some(meta)
}

fn content_extension_ok(name: &str, folder: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let base = lower.trim_end_matches(".disabled");
    match folder {
        "mods" => base.ends_with(".jar") || base.ends_with(".zip"),
        "horizon-mods" => base.ends_with(".hmod"),
        _ => base.ends_with(".zip") || !base.contains('.'),
    }
}

pub async fn list_installed(instance_id: &str, kind: &str, identify: bool) -> Result<Vec<InstalledFile>> {
    let instance = instances::get(instance_id)?;
    let game_dir = instances::game_dir(&instance)?;
    let folder = folder_in(&instance, kind)?;
    let identify = identify && folder != "horizon-mods";
    let dir = game_dir.join(folder);
    let mut index = load_index(&game_dir);
    let mut unknown: Vec<(String, PathBuf, u64)> = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(meta) = entry.metadata() else { continue };
            if !content_extension_ok(&name, folder) || (meta.is_dir() && folder == "mods") {
                continue;
            }
            let relative = format!("{folder}/{name}");
            let key = canonical_key(&relative);
            let size = if meta.is_dir() { 0 } else { meta.len() };
            let known = index.files.get(&key).filter(|e| e.size == 0 || e.size == size).is_some();
            if !known && meta.is_file() {
                unknown.push((key.clone(), entry.path(), size));
            }
            files.push((relative, name, size));
        }
    }
    if identify && !unknown.is_empty() {
        let hashed: Vec<(String, String, u64)> = tokio::task::spawn_blocking(move || {
            unknown.into_iter().filter_map(|(key, path, size)| net::sha1_file(&path).ok().map(|h| (key, h, size))).collect()
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))?;
        let hashes: Vec<&String> = hashed.iter().map(|h| &h.1).collect();
        let lookup: Option<HashMap<String, serde_json::Value>> =
            match net::client().post(format!("{MODRINTH}/version_files")).json(&json!({"hashes": hashes, "algorithm": "sha1"})).send().await {
                Ok(response) if response.status().is_success() => response.json().await.ok(),
                _ => None,
            };
        let lookup_ok = lookup.is_some();
        let lookup = lookup.unwrap_or_default();
        let project_ids: Vec<String> = lookup.values().filter_map(|v| v["project_id"].as_str().map(String::from)).collect::<HashSet<_>>().into_iter().collect();
        let mut projects: HashMap<String, (String, String)> = HashMap::new();
        if !project_ids.is_empty() {
            if let Ok(list) = net::client()
                .get(format!("{MODRINTH}/projects"))
                .query(&[("ids", serde_json::to_string(&project_ids)?)])
                .send()
                .await
                .and_then(|r| r.error_for_status())
            {
                if let Ok(items) = list.json::<Vec<serde_json::Value>>().await {
                    for p in items {
                        if let Some(id) = p["id"].as_str() {
                            projects.insert(id.into(), (p["title"].as_str().unwrap_or("").into(), p["icon_url"].as_str().unwrap_or("").into()));
                        }
                    }
                }
            }
        }
        for (key, hash, size) in hashed {
            let entry = match lookup.get(&hash) {
                Some(v) => {
                    let pid = v["project_id"].as_str().unwrap_or("").to_string();
                    let (title, icon) = projects.get(&pid).cloned().unwrap_or_default();
                    IndexEntry {
                        source: "modrinth".into(),
                        project_id: pid,
                        version_id: v["id"].as_str().unwrap_or("").into(),
                        title,
                        icon_url: icon,
                        version_number: v["version_number"].as_str().unwrap_or("").into(),
                        sha1: hash,
                        size,
                        local_checked: false,
                    }
                }
                None if lookup_ok => IndexEntry { source: "local".into(), sha1: hash, size, ..Default::default() },
                None => continue,
            };
            index.files.insert(key, entry);
        }
        save_index(&game_dir, &index)?;
    }
    if identify {
        let pending: Vec<(String, PathBuf, String)> = files
            .iter()
            .filter_map(|(relative, name, _)| {
                let key = canonical_key(relative);
                let entry = index.files.get(&key)?;
                (entry.source == "local" && !entry.local_checked).then(|| (key, dir.join(name), entry.sha1.clone()))
            })
            .collect();
        if !pending.is_empty() {
            let found = tokio::task::spawn_blocking(move || pending.into_iter().map(|(key, path, sha)| (key, sha, local_meta(&path))).collect::<Vec<_>>())
                .await
                .map_err(|e| Error::msg(e.to_string()))?;
            for (key, sha, meta) in found {
                let Some(entry) = index.files.get_mut(&key) else { continue };
                entry.local_checked = true;
                let Some(meta) = meta else { continue };
                if !meta.name.trim().is_empty() {
                    entry.title = meta.name.trim().to_string();
                }
                if !meta.version.trim().is_empty() {
                    entry.version_number = meta.version.trim().to_string();
                }
                if let Some(bytes) = meta.icon {
                    let name = format!("{}.img", if sha.is_empty() { net::sha1_bytes(key.as_bytes()) } else { sha });
                    if storage::write_bytes_atomic(&icon_cache().join(&name), &bytes).is_ok() {
                        entry.icon_url = format!("local:{name}");
                    }
                }
            }
            save_index(&game_dir, &index)?;
        }
    }
    let mut out: Vec<InstalledFile> = files
        .into_iter()
        .map(|(relative, name, size)| {
            let key = canonical_key(&relative);
            let entry = index.files.get(&key).cloned().unwrap_or_default();
            let display = name.trim_end_matches(".disabled").to_string();
            InstalledFile {
                enabled: !name.ends_with(".disabled"),
                kind: kind_for_folder(folder).into(),
                title: if entry.title.is_empty() { display.clone() } else { entry.title.clone() },
                version: entry.version_number.clone(),
                project_id: entry.project_id.clone(),
                version_id: entry.version_id.clone(),
                source: if entry.source.is_empty() { "local".into() } else { entry.source.clone() },
                icon_url: if entry.icon_url.is_empty() && folder != "mods" && dir.join(&name).is_dir() {
                    local_meta(&dir.join(&name)).and_then(|m| m.icon).and_then(|b| data_url(&b)).unwrap_or_default()
                } else {
                    icon_src(&entry.icon_url)
                },
                path: relative,
                file_name: display,
                size,
            }
        })
        .collect();
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

fn relative_target(instance: &Instance, relative: &str) -> Result<(PathBuf, PathBuf)> {
    let game_dir = instances::game_dir(instance)?;
    let folder = relative.split('/').next().unwrap_or("");
    if !matches!(folder, "mods" | "resourcepacks" | "shaderpacks" | "horizon-mods") {
        return Err(Error::msg("Content path outside managed folders"));
    }
    let path = storage::safe_join(&game_dir, relative)?;
    Ok((game_dir, path))
}

pub fn set_enabled(instance_id: &str, relative: &str, enabled: bool) -> Result<String> {
    let instance = instances::get(instance_id)?;
    let (_, path) = relative_target(&instance, relative)?;
    if !path.exists() {
        return Err(Error::msg("File not found"));
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let is_disabled = name.ends_with(".disabled");
    if enabled == !is_disabled {
        return Ok(relative.to_string());
    }
    let new_name = if enabled { name.trim_end_matches(".disabled").to_string() } else { format!("{name}.disabled") };
    let target = path.with_file_name(&new_name);
    if target.exists() {
        return Err(Error::msg(format!("{new_name} already exists")));
    }
    std::fs::rename(&path, &target)?;
    let folder = relative.split('/').next().unwrap_or("");
    Ok(format!("{folder}/{new_name}"))
}

pub fn remove(instance_id: &str, relative: &str) -> Result<()> {
    let instance = instances::get(instance_id)?;
    let (game_dir, path) = relative_target(&instance, relative)?;
    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else if path.is_file() {
        std::fs::remove_file(&path)?;
    }
    let mut index = load_index(&game_dir);
    index.files.remove(&canonical_key(relative));
    save_index(&game_dir, &index)
}

pub fn import_local(instance_id: &str, kind: &str, paths: Vec<String>) -> Result<Vec<String>> {
    let instance = instances::get(instance_id)?;
    let game_dir = instances::game_dir(&instance)?;
    let folder = folder_in(&instance, kind)?;
    let dir = game_dir.join(folder);
    std::fs::create_dir_all(&dir)?;
    let mut imported = Vec::new();
    for source in paths {
        let source = PathBuf::from(source);
        let name = source.file_name().map(|n| n.to_string_lossy().to_string()).ok_or_else(|| Error::msg("Invalid file"))?;
        if !content_extension_ok(&name, folder) || !source.is_file() {
            return Err(Error::msg(format!("{name} is not a valid {kind} file")));
        }
        std::fs::copy(&source, dir.join(&name))?;
        imported.push(name);
    }
    Ok(imported)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub path: String,
    pub project_id: String,
    pub current: String,
    pub latest: String,
    pub version_id: String,
}

pub async fn check_updates(instance_id: &str, kind: &str) -> Result<Vec<UpdateInfo>> {
    let instance = instances::get(instance_id)?;
    let game_dir = instances::game_dir(&instance)?;
    let files = list_installed(instance_id, kind, true).await?;
    let index = load_index(&game_dir);
    let mut hashes = Vec::new();
    let mut by_hash = HashMap::new();
    for file in files.iter().filter(|f| f.source == "modrinth") {
        if let Some(entry) = index.files.get(&canonical_key(&file.path)) {
            if !entry.sha1.is_empty() {
                hashes.push(entry.sha1.clone());
                by_hash.insert(entry.sha1.clone(), (file.path.clone(), entry.clone()));
            }
        }
    }
    if hashes.is_empty() {
        return Ok(horizon_updates(&instance, &files, &index).await);
    }
    let mut body = json!({"hashes": hashes, "algorithm": "sha1"});
    if !instance.minecraft.is_empty() {
        body["game_versions"] = json!([instance.minecraft]);
    }
    if kind == "mod" && instance.loader != "vanilla" {
        body["loaders"] = json!([instance.loader]);
    }
    let latest: HashMap<String, serde_json::Value> = net::client()
        .post(format!("{MODRINTH}/version_files/update"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut out = horizon_updates(&instance, &files, &index).await;
    for (hash, version) in latest {
        let Some((path, entry)) = by_hash.get(&hash) else { continue };
        let mut chosen = (version["id"].as_str().unwrap_or("").to_string(), version["version_number"].as_str().unwrap_or("").to_string());
        if version["version_type"].as_str().is_some_and(|t| t != "release") {
            if let Ok(list) = versions("modrinth", &entry.project_id, kind, &instance.minecraft, &instance.loader).await {
                if list.iter().find(|v| v.id == entry.version_id).is_none_or(|v| v.channel == "release") {
                    match list.iter().find(|v| v.channel == "release") {
                        Some(v) => chosen = (v.id.clone(), v.version_number.clone()),
                        None => continue,
                    }
                }
            }
        }
        if !chosen.0.is_empty() && chosen.0 != entry.version_id {
            out.push(UpdateInfo {
                path: path.clone(),
                project_id: entry.project_id.clone(),
                current: entry.version_number.clone(),
                latest: chosen.1,
                version_id: chosen.0,
            });
        }
    }
    Ok(out)
}

async fn horizon_updates(instance: &Instance, files: &[InstalledFile], index: &ContentIndex) -> Vec<UpdateInfo> {
    let installed: Vec<&InstalledFile> = files.iter().filter(|f| f.source == "horizon").collect();
    if installed.is_empty() {
        return Vec::new();
    }
    let Ok(mods) = crate::horizon::mod_index().await else { return Vec::new() };
    installed
        .into_iter()
        .filter_map(|file| {
            let entry = index.files.get(&canonical_key(&file.path))?;
            let manifest = mods.iter().find(|m| m.id == entry.project_id)?;
            let newest = manifest.versions.iter().find(|v| v.game == instance.minecraft)?;
            (newest.version != entry.version_id).then(|| UpdateInfo {
                path: file.path.clone(),
                project_id: entry.project_id.clone(),
                current: entry.version_number.clone(),
                latest: newest.version.clone(),
                version_id: newest.version.clone(),
            })
        })
        .collect()
}

pub async fn update_file(instance_id: &str, relative: &str, version_id: &str, reporter: &Reporter) -> Result<()> {
    let instance = instances::get(instance_id)?;
    let game_dir = instances::game_dir(&instance)?;
    let index = load_index(&game_dir);
    let entry = index.files.get(&canonical_key(relative)).cloned().ok_or_else(|| Error::msg("Unknown file"))?;
    let was_disabled = relative.ends_with(".disabled");
    let kind = kind_for_folder(relative.split('/').next().unwrap_or("mods"));
    let outcome = install(instance_id, &entry.source, &entry.project_id, Some(version_id.to_string()), kind, reporter).await?;
    if was_disabled && !outcome.installed.is_empty() {
        let refreshed = load_index(&game_dir);
        if let Some((path, _)) = refreshed.files.iter().find(|(_, e)| e.project_id == entry.project_id) {
            let _ = set_enabled(instance_id, path, false);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mod.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, body) in entries {
            writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
        (dir, path)
    }

    #[test]
    fn reads_local_mod_metadata() {
        let png: &[u8] = &[0x89, b'P', b'N', b'G', 1, 2, 3];
        let (_d, p) = jar(&[("fabric.mod.json", br#"{"id":"cool","name":"Cool Mod","version":"1.2.3","icon":{"16":"a.png","128":"assets/cool/icon.png"}}"#), ("assets/cool/icon.png", png)]);
        let m = local_meta(&p).unwrap();
        assert_eq!((m.name.as_str(), m.version.as_str(), m.icon.as_deref()), ("Cool Mod", "1.2.3", Some(png)));
        assert_eq!(mod_identity(&p), Some(("cool".into(), "Cool Mod".into())));
        let (_d, p) = jar(&[("META-INF/mods.toml", b"modLoader=\"javafml\"
[[mods]]
modId=\"x\"
version=\"${file.jarVersion}\"
displayName=\"Forge Thing\"
logoFile=\"logo.png\"
"), ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0
Implementation-Version: 4.5
"), ("logo.png", png)]);
        let m = local_meta(&p).unwrap();
        assert_eq!((m.name.as_str(), m.version.as_str(), m.icon.is_some()), ("Forge Thing", "4.5", true));
        assert_eq!(mod_identity(&p), Some(("x".into(), "Forge Thing".into())));
        let (_d, p) = jar(&[("pack.mcmeta", b"{}"), ("pack.png", png)]);
        assert!(local_meta(&p).unwrap().icon.is_some());
    }

    #[test]
    fn folders_and_keys() {
        assert_eq!(folder_for("mod").unwrap(), "mods");
        assert_eq!(folder_for("shader").unwrap(), "shaderpacks");
        assert!(folder_for("world").is_err());
        assert_eq!(canonical_key("mods/a.jar.disabled"), "mods/a.jar");
        assert!(content_extension_ok("sodium.jar.disabled", "mods"));
        assert!(!content_extension_ok("readme.txt", "mods"));
        assert!(content_extension_ok("Faithful.zip", "resourcepacks"));
    }

    #[test]
    fn parses_modrinth_version() {
        let v = json!({"id":"abc","name":"Sodium 0.6","version_number":"0.6.0","game_versions":["1.21.1"],"loaders":["fabric"],"date_published":"2024","files":[{"url":"https://cdn.modrinth.com/x.jar","filename":"sodium.jar","primary":true,"size":5,"hashes":{"sha1":"aa"}}],"dependencies":[{"project_id":"P1","version_id":null,"dependency_type":"required"}]});
        let parsed = modrinth_version(&v).unwrap();
        assert_eq!(parsed.file_name, "sodium.jar");
        assert_eq!(parsed.dependencies[0].kind, "required");
        assert_eq!(parsed.sha1, "aa");
    }
}
