use crate::error::{Error, Result};
use crate::{auth, content, instances, net, nbt};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

const LOG_LINES: usize = 20000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchLog {
    pub found: bool,
    pub path: String,
    pub lines: Vec<String>,
    pub summary: String,
}

pub fn launch_log(id: &str, started: u64, crashed: bool) -> Result<LaunchLog> {
    let instance = instances::get(id)?;
    let path = instances::dir(id)?.join("launcher-logs").join(format!("launch-{started}.log"));
    if !path.is_file() {
        return Ok(LaunchLog { found: false, path: String::new(), lines: Vec::new(), summary: String::new() });
    }
    let text = String::from_utf8_lossy(&std::fs::read(&path)?).into_owned();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    if lines.len() > LOG_LINES {
        lines.drain(..lines.len() - LOG_LINES);
    }
    let summary = if crashed { crate::crash::analyze(&lines, None, &instances::game_dir(&instance)?.join("mods")) } else { String::new() };
    Ok(LaunchLog { found: true, path: path.to_string_lossy().into_owned(), lines, summary })
}

fn plain_name(name: &str) -> Result<&str> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name.contains("..") {
        return Err(Error::msg("Invalid file name"));
    }
    Ok(name)
}

fn modified(path: &Path) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: u64,
}

pub fn screenshots_dir(id: &str) -> Result<PathBuf> {
    Ok(instances::game_dir(&instances::get(id)?)?.join("screenshots"))
}

pub fn screenshots(id: &str) -> Result<Vec<Screenshot>> {
    let dir = screenshots_dir(id)?;
    let mut out: Vec<Screenshot> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()).is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg")))
        .map(|e| {
            let path = e.path();
            Screenshot { name: e.file_name().to_string_lossy().into_owned(), size: e.metadata().map(|m| m.len()).unwrap_or(0), modified: modified(&path), path: path.to_string_lossy().into_owned() }
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

pub fn delete_screenshot(id: &str, name: &str) -> Result<()> {
    let path = screenshots_dir(id)?.join(plain_name(name)?);
    std::fs::remove_file(path)?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    pub folder: String,
    pub name: String,
    pub last_played: i64,
    pub mode: String,
    pub version: String,
    pub hardcore: bool,
    pub icon: Option<String>,
    pub datapacks: usize,
}

fn level_data(path: &Path) -> Option<nbt::Tag> {
    let raw = std::fs::read(path).ok()?;
    let mut bytes = Vec::new();
    if raw.starts_with(&[0x1f, 0x8b]) {
        flate2::read::GzDecoder::new(&raw[..]).take(64 * 1024 * 1024).read_to_end(&mut bytes).ok()?;
    } else {
        bytes = raw;
    }
    nbt::read(&bytes).ok()?.get("Data").cloned()
}

fn int(tag: Option<&nbt::Tag>) -> Option<i64> {
    match tag? {
        nbt::Tag::Byte(v) => Some(*v as i64),
        nbt::Tag::Short(v) => Some(*v as i64),
        nbt::Tag::Int(v) => Some(*v as i64),
        nbt::Tag::Long(v) => Some(*v),
        _ => None,
    }
}

fn saves(id: &str) -> Result<PathBuf> {
    Ok(instances::game_dir(&instances::get(id)?)?.join("saves"))
}

fn count_packs(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.flatten().filter(|e| e.path().is_dir() || e.path().extension().is_some_and(|x| x == "zip")).count()).unwrap_or(0)
}

pub fn worlds(id: &str) -> Result<Vec<World>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(saves(id)?).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.join("level.dat").is_file() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().into_owned();
        let data = level_data(&path.join("level.dat"));
        let get = |k: &str| data.as_ref().and_then(|d| d.get(k));
        let mode = match int(get("GameType")) {
            Some(1) => "Creative",
            Some(2) => "Adventure",
            Some(3) => "Spectator",
            _ => "Survival",
        };
        out.push(World {
            name: get("LevelName").and_then(|t| t.as_str()).filter(|s| !s.trim().is_empty()).unwrap_or(&folder).to_string(),
            last_played: int(get("LastPlayed")).unwrap_or_else(|| modified(&path.join("level.dat")) as i64 * 1000),
            mode: mode.into(),
            version: get("Version").and_then(|v| v.get("Name")).and_then(|t| t.as_str()).unwrap_or_default().to_string(),
            hardcore: int(get("hardcore")).unwrap_or(0) != 0,
            icon: std::fs::read(path.join("icon.png")).ok().filter(|b| b.len() < 512 * 1024).and_then(|b| content::data_url(&b)),
            datapacks: count_packs(&path.join("datapacks")),
            folder,
        });
    }
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Datapack {
    pub name: String,
    pub size: u64,
    pub folder: bool,
}

pub fn datapacks(id: &str, world: &str) -> Result<Vec<Datapack>> {
    let dir = saves(id)?.join(plain_name(world)?).join("datapacks");
    let mut out: Vec<Datapack> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir() || e.path().extension().is_some_and(|x| x == "zip"))
        .map(|e| Datapack { name: e.file_name().to_string_lossy().into_owned(), size: e.metadata().map(|m| m.len()).unwrap_or(0), folder: e.path().is_dir() })
        .collect();
    out.sort_by_key(|d| d.name.to_lowercase());
    Ok(out)
}

pub fn import_datapacks(id: &str, world: &str, paths: Vec<String>) -> Result<usize> {
    let dir = saves(id)?.join(plain_name(world)?).join("datapacks");
    std::fs::create_dir_all(&dir)?;
    let mut added = 0;
    for p in paths {
        let src = PathBuf::from(&p);
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| Error::msg("Invalid file"))?;
        if !name.to_ascii_lowercase().ends_with(".zip") {
            return Err(Error::msg(format!("{name} is not a .zip datapack")));
        }
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&src)?).map_err(|_| Error::msg(format!("{name} is not a valid zip file")))?;
        if archive.by_name("pack.mcmeta").is_err() {
            return Err(Error::msg(format!("{name} has no pack.mcmeta, so it is not a datapack")));
        }
        std::fs::copy(&src, dir.join(plain_name(&name)?))?;
        added += 1;
    }
    Ok(added)
}

pub fn remove_datapack(id: &str, world: &str, name: &str) -> Result<()> {
    let path = saves(id)?.join(plain_name(world)?).join("datapacks").join(plain_name(name)?);
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub active: bool,
}

pub async fn capes(account_id: &str) -> Result<Vec<Cape>> {
    let token = auth::access_token(account_id).await?;
    let profile: serde_json::Value = net::client()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(profile["capes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(Cape {
                        id: c["id"].as_str()?.into(),
                        alias: c["alias"].as_str().unwrap_or("Cape").into(),
                        url: c["url"].as_str().unwrap_or("").into(),
                        active: c["state"].as_str() == Some("ACTIVE"),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

pub async fn set_cape(account_id: &str, cape_id: Option<&str>) -> Result<()> {
    let token = auth::access_token(account_id).await?;
    let url = "https://api.minecraftservices.com/minecraft/profile/capes/active";
    let request = match cape_id {
        Some(id) => net::client().put(url).json(&serde_json::json!({ "capeId": id })),
        None => net::client().delete(url),
    };
    let response = request.bearer_auth(token).send().await?;
    if !response.status().is_success() {
        return Err(Error::msg(format!("Minecraft did not accept the cape change (HTTP {})", response.status().as_u16())));
    }
    auth::refresh_profile(account_id).await
}
