use super::version::{self, Features, VersionJson};
use crate::error::{Error, Result};
use crate::net::{self, DownloadItem};
use crate::storage;
use crate::tasks::Reporter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub release_time: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

pub async fn manifest(force: bool) -> Result<VersionManifest> {
    net::get_json_cached(MANIFEST_URL, "version_manifest_v2", if force { 0 } else { 600 }).await
}

#[derive(Deserialize)]
struct AssetIndex {
    objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    is_virtual: bool,
    #[serde(default)]
    map_to_resources: bool,
}

#[derive(Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

pub async fn ensure_vanilla_json(id: &str) -> Result<()> {
    let versions = storage::versions_dir();
    let path = version::version_json_path(&versions, id);
    let manifest = match manifest(false).await {
        Ok(m) => m,
        Err(e) => {
            if path.is_file() {
                return Ok(());
            }
            return Err(e);
        }
    };
    let entry = match manifest.versions.iter().find(|v| v.id == id) {
        Some(entry) => entry.clone(),
        None => {
            let fresh = self::manifest(true).await?;
            fresh
                .versions
                .into_iter()
                .find(|v| v.id == id)
                .ok_or_else(|| Error::msg(format!("Minecraft {id} was not found in Mojang's version manifest")))?
        }
    };
    if path.is_file() {
        if let Some(expected) = &entry.sha1 {
            if net::sha1_file(&path).map(|h| &h == expected).unwrap_or(false) {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }
    let bytes = net::get_bytes(&entry.url).await?;
    if let Some(expected) = &entry.sha1 {
        if &net::sha1_bytes(&bytes) != expected {
            return Err(Error::msg(format!("Checksum mismatch for version {id} metadata")));
        }
    }
    serde_json::from_slice::<VersionJson>(&bytes)?;
    storage::write_bytes_atomic(&path, &bytes)?;
    Ok(())
}

pub fn client_jar_path(id: &str) -> PathBuf {
    storage::versions_dir().join(id).join(format!("{id}.jar"))
}

pub async fn install_version(id: &str, reporter: &Reporter, verify: bool, features: &Features) -> Result<VersionJson> {
    let versions = storage::versions_dir();
    let libraries = storage::libraries_dir();
    let merged = version::load_merged(&versions, id)?;

    reporter.phase("Downloading game files");
    let mut items = Vec::new();
    let jar_id = merged.jar.clone().unwrap_or_else(|| merged.id.clone());
    if let Some(client) = merged.downloads.as_ref().and_then(|d| d.get("client")) {
        items.push(DownloadItem::new(client.url.clone(), client_jar_path(&jar_id), client.sha1.clone(), client.size));
    }
    for lib in version::resolve_libraries(&merged, &libraries, features)? {
        if lib.url.is_empty() || lib.url.ends_with('/') {
            continue;
        }
        let relative = lib.path.strip_prefix(&libraries).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        let mut item = DownloadItem::new(lib.url.clone(), lib.path.clone(), lib.sha1.clone(), lib.size);
        if lib.sha1.is_none() {
            item.fallback_urls = version::fallback_urls(&relative, &lib.url);
        }
        items.push(item);
    }
    if let Some(logging) = merged.logging.as_ref().and_then(|l| l.get("client")) {
        items.push(DownloadItem::new(
            logging.file.url.clone(),
            storage::assets_dir().join("log_configs").join(&logging.file.id),
            logging.file.sha1.clone(),
            logging.file.size,
        ));
    }
    net::download_all(items, reporter.progress(), verify).await?;

    for lib in version::resolve_libraries(&merged, &libraries, features)? {
        if !lib.path.is_file() {
            return Err(Error::msg(format!("Library {} is missing and has no download source", lib.name)));
        }
    }

    if jar_id != merged.id {
        let source = client_jar_path(&jar_id);
        let target = client_jar_path(&merged.id);
        let stale = match (std::fs::metadata(&source), std::fs::metadata(&target)) {
            (Ok(s), Ok(t)) => s.len() != t.len(),
            (Ok(_), Err(_)) => true,
            _ => false,
        };
        if stale {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source, &target)?;
        }
    }

    if let Some(index_ref) = &merged.asset_index {
        reporter.phase("Downloading assets");
        let index_path = storage::assets_dir().join("indexes").join(format!("{}.json", index_ref.id));
        net::download_all(
            vec![DownloadItem::new(index_ref.url.clone(), index_path.clone(), index_ref.sha1.clone(), index_ref.size)],
            reporter.progress(),
            verify,
        )
        .await?;
        let index: AssetIndex = storage::read_json(&index_path)?.ok_or_else(|| Error::msg("Asset index missing"))?;
        let objects = storage::assets_dir().join("objects");
        let items: Vec<DownloadItem> = index
            .objects
            .values()
            .map(|o| {
                let prefix = &o.hash[..2];
                DownloadItem::new(
                    format!("{RESOURCES_URL}/{prefix}/{}", o.hash),
                    objects.join(prefix).join(&o.hash),
                    Some(o.hash.clone()),
                    Some(o.size),
                )
            })
            .collect();
        reporter.phase("Downloading assets");
        net::download_all(items, reporter.progress(), verify).await?;
    }
    Ok(merged)
}

pub struct LegacyAssets {
    pub virtual_dir: Option<PathBuf>,
}

pub fn prepare_legacy_assets(merged: &VersionJson, game_dir: &Path) -> Result<LegacyAssets> {
    let Some(index_ref) = &merged.asset_index else { return Ok(LegacyAssets { virtual_dir: None }) };
    let index_path = storage::assets_dir().join("indexes").join(format!("{}.json", index_ref.id));
    let index: AssetIndex = storage::read_json(&index_path)?.ok_or_else(|| Error::msg("Asset index missing"))?;
    if !index.is_virtual && !index.map_to_resources {
        return Ok(LegacyAssets { virtual_dir: None });
    }
    let objects = storage::assets_dir().join("objects");
    let target_root = if index.map_to_resources {
        game_dir.join("resources")
    } else {
        storage::assets_dir().join("virtual").join(&index_ref.id)
    };
    for (name, object) in &index.objects {
        let target = storage::safe_join(&target_root, name)?;
        if target.is_file() && std::fs::metadata(&target).map(|m| m.len() == object.size).unwrap_or(false) {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(objects.join(&object.hash[..2]).join(&object.hash), &target)?;
    }
    Ok(LegacyAssets { virtual_dir: Some(target_root) })
}

pub fn extract_natives(merged: &VersionJson, natives_dir: &Path, features: &Features) -> Result<()> {
    let marker = natives_dir.join(".zip-natives");
    let libs: Vec<_> = version::resolve_libraries(merged, &storage::libraries_dir(), features)?
        .into_iter()
        .filter(|l| l.native)
        .collect();
    let signature = libs.iter().map(|l| l.path.to_string_lossy().to_string()).collect::<Vec<_>>().join("|");
    let signature = net::sha1_bytes(signature.as_bytes());
    if std::fs::read_to_string(&marker).map(|m| m == signature).unwrap_or(false) {
        return Ok(());
    }
    std::fs::create_dir_all(natives_dir)?;
    for lib in &libs {
        let file = std::fs::File::open(&lib.path)
            .map_err(|_| Error::msg(format!("Native library missing: {}", lib.path.display())))?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if name.starts_with("META-INF") || lib.exclude.iter().any(|ex| name.starts_with(ex.as_str())) {
                continue;
            }
            let lower = name.to_ascii_lowercase();
            if !(lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib") || lower.ends_with(".jnilib")) {
                continue;
            }
            let file_name = Path::new(&name).file_name().map(|f| f.to_os_string()).unwrap_or_default();
            if file_name.is_empty() {
                continue;
            }
            let target = natives_dir.join(file_name);
            let mut buffer = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buffer)?;
            if target.is_file() && std::fs::metadata(&target).map(|m| m.len() == buffer.len() as u64).unwrap_or(false) {
                continue;
            }
            if let Err(e) = std::fs::write(&target, &buffer) {
                if !target.is_file() {
                    return Err(e.into());
                }
            }
        }
    }
    std::fs::write(&marker, signature)?;
    Ok(())
}
