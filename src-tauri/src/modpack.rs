use crate::content;
use crate::error::{Error, Result};
use crate::instances::{self, Instance, ModpackInfo};
use crate::minecraft::{install, loaders};
use crate::net::{self, DownloadItem};
use crate::storage;
use crate::tasks::Reporter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const MRPACK_HOSTS: [&str; 5] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com", "objects.githubusercontent.com"];

#[derive(Serialize)]
pub struct ImportResult {
    pub instance: Instance,
    pub warnings: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum Format {
    Mrpack,
    CurseForge,
    Prism(String),
    Zip(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    #[serde(default)]
    format_version: u32,
    #[serde(default)]
    game: String,
    #[serde(default)]
    version_id: String,
    name: String,
    #[serde(default)]
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrFile {
    path: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    manifest_type: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    files: Vec<CfFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Deserialize)]
struct CfLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "default_true")]
    required: bool,
}

fn default_true() -> bool {
    true
}

fn open(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).map_err(|_| Error::msg("The selected file could not be opened"))?;
    zip::ZipArchive::new(file).map_err(|_| Error::msg("The selected file is not a valid ZIP archive"))
}

fn normalized_names(archive: &zip::ZipArchive<std::fs::File>) -> Vec<String> {
    archive.file_names().map(|n| n.replace('\\', "/")).collect()
}

fn find_entry(archive: &zip::ZipArchive<std::fs::File>, name: &str) -> Option<usize> {
    (0..archive.len()).find(|i| archive.name_for_index(*i).map(|n| n.replace('\\', "/") == name).unwrap_or(false))
}

fn read_text(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String> {
    let index = find_entry(archive, name).ok_or_else(|| Error::msg(format!("Archive is missing {name}")))?;
    let mut entry = archive.by_index(index)?;
    if entry.size() > 32 * 1024 * 1024 {
        return Err(Error::msg(format!("{name} is unreasonably large")));
    }
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    Ok(text)
}

pub fn detect(archive: &mut zip::ZipArchive<std::fs::File>) -> Result<Format> {
    let names = normalized_names(archive);
    if names.iter().any(|n| n == "modrinth.index.json") {
        return Ok(Format::Mrpack);
    }
    if names.iter().any(|n| n == "manifest.json") {
        if let Ok(text) = read_text(archive, "manifest.json") {
            if text.contains("minecraftModpack") {
                return Ok(Format::CurseForge);
            }
        }
    }
    for name in &names {
        let depth = name.matches('/').count();
        if depth > 1 {
            continue;
        }
        if let Some(prefix) = name.strip_suffix(instances::EXPORT_MANIFEST) {
            return Ok(Format::Zip(prefix.to_string()));
        }
        if let Some(prefix) = name.strip_suffix("mmc-pack.json").or_else(|| name.strip_suffix("instance.cfg")) {
            if prefix.is_empty() || prefix.ends_with('/') {
                return Ok(Format::Prism(prefix.to_string()));
            }
        }
    }
    Err(Error::msg("Unrecognised archive. Supported: Modrinth .mrpack, CurseForge .zip, Prism Launcher .zip and Zip exports."))
}

fn extract_prefix(archive_path: &Path, prefix: &str, dest: &Path) -> Result<usize> {
    let mut archive = open(archive_path)?;
    let mut count = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(enclosed) = entry.enclosed_name() else {
            return Err(Error::msg(format!("Archive contains an unsafe path: {}", entry.name())));
        };
        let name = enclosed.to_string_lossy().replace('\\', "/");
        let Some(relative) = name.strip_prefix(prefix) else { continue };
        if relative.is_empty() {
            continue;
        }
        let target = storage::safe_join(dest, relative)?;
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut out)?;
        count += 1;
    }
    Ok(count)
}

fn host_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split('/').next().unwrap_or("").to_ascii_lowercase();
    MRPACK_HOSTS.iter().any(|h| host == *h)
}

pub fn loader_from_mrpack(minecraft: &str, deps: &HashMap<String, String>) -> (String, String) {
    if let Some(v) = deps.get("neoforge") {
        let full = if minecraft == "1.20.1" && !v.starts_with("1.20.1-") { format!("1.20.1-{v}") } else { v.clone() };
        return ("neoforge".into(), full);
    }
    if let Some(v) = deps.get("forge") {
        let full = if v.starts_with(&format!("{minecraft}-")) { v.clone() } else { format!("{minecraft}-{v}") };
        return ("forge".into(), full);
    }
    if let Some(v) = deps.get("fabric-loader") {
        return ("fabric".into(), v.clone());
    }
    if let Some(v) = deps.get("quilt-loader") {
        return ("quilt".into(), v.clone());
    }
    ("vanilla".into(), String::new())
}

pub fn loader_from_curseforge(minecraft: &str, id: &str) -> (String, String) {
    let (name, version) = id.split_once('-').unwrap_or((id, ""));
    match name {
        "forge" => ("forge".into(), if version.starts_with(&format!("{minecraft}-")) { version.into() } else { format!("{minecraft}-{version}") }),
        "neoforge" => ("neoforge".into(), if minecraft == "1.20.1" && !version.starts_with("1.20.1-") { format!("1.20.1-{version}") } else { version.into() }),
        "fabric" => ("fabric".into(), version.into()),
        "quilt" => ("quilt".into(), version.into()),
        _ => ("vanilla".into(), String::new()),
    }
}

pub fn loader_from_prism(components: &[serde_json::Value]) -> (String, String, String) {
    let mut minecraft = String::new();
    let mut loader = ("vanilla".to_string(), String::new());
    for c in components {
        let uid = c["uid"].as_str().unwrap_or("");
        let version = c["version"].as_str().or_else(|| c["cachedVersion"].as_str()).unwrap_or("").to_string();
        match uid {
            "net.minecraft" => minecraft = version,
            "net.fabricmc.fabric-loader" => loader = ("fabric".into(), version),
            "org.quiltmc.quilt-loader" => loader = ("quilt".into(), version),
            "net.minecraftforge" => loader = ("forge".into(), version),
            "net.neoforged" => loader = ("neoforge".into(), version),
            _ => {}
        }
    }
    if loader.0 == "forge" && !loader.1.starts_with(&format!("{minecraft}-")) {
        loader.1 = format!("{minecraft}-{}", loader.1);
    }
    if loader.0 == "neoforge" && minecraft == "1.20.1" && !loader.1.starts_with("1.20.1-") {
        loader.1 = format!("1.20.1-{}", loader.1);
    }
    (minecraft, loader.0, loader.1)
}

fn parse_cfg(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

async fn validate_versions(instance: &Instance, warnings: &mut Vec<String>) -> Result<()> {
    install::ensure_vanilla_json(&instance.minecraft)
        .await
        .map_err(|e| Error::msg(format!("Minecraft {} could not be resolved: {}", instance.minecraft, e.0)))?;
    if instance.loader != "vanilla" {
        match loaders::list(&instance.loader, &instance.minecraft).await {
            Ok(list) if list.iter().any(|v| v.version == instance.loader_version) => {}
            Ok(_) => warnings.push(format!("{} {} was not found in the official metadata", instance.loader, instance.loader_version)),
            Err(e) => warnings.push(format!("Loader metadata unavailable: {}", e.0)),
        }
    }
    Ok(())
}

fn cleanup(instance: &Instance) {
    if let Ok(dir) = instances::dir(&instance.id) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub async fn import(path: &Path, reporter: &Reporter) -> Result<ImportResult> {
    reporter.phase("Reading archive");
    let format = detect(&mut open(path)?)?;
    let mut warnings = Vec::new();
    let template = match &format {
        Format::Mrpack => {
            let index: MrIndex = serde_json::from_str(&read_text(&mut open(path)?, "modrinth.index.json")?)
                .map_err(|e| Error::msg(format!("Invalid modrinth.index.json: {e}")))?;
            if index.game != "minecraft" || index.format_version != 1 {
                return Err(Error::msg("Unsupported .mrpack format"));
            }
            let minecraft = index.dependencies.get("minecraft").cloned().ok_or_else(|| Error::msg("Modpack does not specify a Minecraft version"))?;
            let (loader, loader_version) = loader_from_mrpack(&minecraft, &index.dependencies);
            Instance {
                name: index.name.clone(),
                minecraft,
                loader,
                loader_version,
                modpack: Some(ModpackInfo { source: "modrinth".into(), name: index.name.clone(), version: index.version_id.clone(), project_id: String::new() }),
                ..Default::default()
            }
        }
        Format::CurseForge => {
            let manifest: CfManifest = serde_json::from_str(&read_text(&mut open(path)?, "manifest.json")?)
                .map_err(|e| Error::msg(format!("Invalid CurseForge manifest: {e}")))?;
            if manifest.manifest_type != "minecraftModpack" {
                return Err(Error::msg("Not a CurseForge Minecraft modpack"));
            }
            let primary = manifest.minecraft.mod_loaders.iter().find(|l| l.primary).or_else(|| manifest.minecraft.mod_loaders.first());
            let (loader, loader_version) = primary.map(|l| loader_from_curseforge(&manifest.minecraft.version, &l.id)).unwrap_or(("vanilla".into(), String::new()));
            Instance {
                name: if manifest.name.is_empty() { "CurseForge pack".into() } else { manifest.name.clone() },
                minecraft: manifest.minecraft.version.clone(),
                loader,
                loader_version,
                modpack: Some(ModpackInfo { source: "curseforge".into(), name: manifest.name.clone(), version: manifest.version.clone(), project_id: String::new() }),
                ..Default::default()
            }
        }
        Format::Prism(prefix) => {
            let mut archive = open(path)?;
            let pack: serde_json::Value = read_text(&mut archive, &format!("{prefix}mmc-pack.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            let cfg = read_text(&mut archive, &format!("{prefix}instance.cfg")).map(|t| parse_cfg(&t)).unwrap_or_default();
            let components = pack["components"].as_array().cloned().unwrap_or_default();
            let (minecraft, loader, loader_version) = loader_from_prism(&components);
            if minecraft.is_empty() {
                return Err(Error::msg("The Prism instance does not specify a Minecraft version"));
            }
            let managed = match cfg.get("ManagedPackType").map(String::as_str) {
                Some("modrinth") => Some("modrinth"),
                Some("flame") => Some("curseforge"),
                _ => None,
            }
            .filter(|_| cfg.get("ManagedPack").map(|v| v == "true").unwrap_or(false));
            let mut instance = Instance {
                name: cfg.get("name").cloned().unwrap_or_else(|| "Prism instance".into()),
                minecraft,
                loader,
                loader_version,
                modpack: managed.map(|source| ModpackInfo {
                    source: source.into(),
                    name: cfg.get("ManagedPackName").cloned().unwrap_or_default(),
                    version: cfg.get("ManagedPackVersionName").cloned().unwrap_or_default(),
                    project_id: cfg.get("ManagedPackID").cloned().unwrap_or_default(),
                }),
                ..Default::default()
            };
            if cfg.get("OverrideMemory").map(|v| v == "true").unwrap_or(false) {
                instance.memory_max = cfg.get("MaxMemAlloc").and_then(|v| v.parse().ok()).unwrap_or(0);
                instance.memory_min = cfg.get("MinMemAlloc").and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            if cfg.get("OverrideJavaArgs").map(|v| v == "true").unwrap_or(false) {
                instance.jvm_args = cfg.get("JvmArgs").cloned().unwrap_or_default();
            }
            instance
        }
        Format::Zip(prefix) => {
            let mut parsed: Instance = serde_json::from_str(&read_text(&mut open(path)?, &format!("{prefix}{}", instances::EXPORT_MANIFEST))?)
                .map_err(|e| Error::msg(format!("Invalid Zip export: {e}")))?;
            parsed.history.clear();
            parsed
        }
    };
    let mut instance = instances::create_from(template)?;
    match finish_import(path, &format, &mut instance, reporter, &mut warnings).await {
        Ok(()) => {
            attach_icon(path, &format, &mut instance).await;
            Ok(ImportResult { instance, warnings })
        }
        Err(e) => {
            cleanup(&instance);
            Err(e)
        }
    }
}

async fn finish_import(path: &Path, format: &Format, instance: &mut Instance, reporter: &Reporter, warnings: &mut Vec<String>) -> Result<()> {
    validate_versions(instance, warnings).await?;
    let game_dir = instances::game_dir(instance)?;
    let archive_path = path.to_path_buf();
    match format {
        Format::Mrpack => {
            reporter.phase("Extracting overrides");
            let (a, g) = (archive_path.clone(), game_dir.clone());
            blocking(move || {
                extract_prefix(&a, "overrides/", &g)?;
                extract_prefix(&a, "client-overrides/", &g)
            })
            .await?;
            let index: MrIndex = serde_json::from_str(&read_text(&mut open(path)?, "modrinth.index.json")?)?;
            let mut items = Vec::new();
            for file in &index.files {
                if file.env.as_ref().and_then(|e| e.get("client")).map(|c| c == "unsupported").unwrap_or(false) {
                    continue;
                }
                let target = storage::safe_join(&game_dir, &file.path)?;
                let Some(url) = file.downloads.iter().find(|u| host_allowed(u)) else {
                    warnings.push(format!("Skipped {}: download host is not trusted", file.path));
                    continue;
                };
                let mut item = DownloadItem::new(url.clone(), target, file.hashes.get("sha1").cloned(), (file.file_size > 0).then_some(file.file_size));
                item.fallback_urls = file.downloads.iter().filter(|u| *u != url && host_allowed(u)).cloned().collect();
                if item.sha1.is_none() {
                    warnings.push(format!("Skipped {}: no checksum provided", file.path));
                    continue;
                }
                items.push(item);
            }
            reporter.phase("Downloading modpack files");
            net::download_all(items, reporter.progress(), true).await?;
        }
        Format::CurseForge => {
            let manifest: CfManifest = serde_json::from_str(&read_text(&mut open(path)?, "manifest.json")?)?;
            let overrides = format!("{}/", manifest.overrides.clone().unwrap_or_else(|| "overrides".into()).trim_end_matches('/'));
            reporter.phase("Extracting overrides");
            let (a, g) = (archive_path.clone(), game_dir.clone());
            blocking(move || extract_prefix(&a, &overrides, &g)).await?;
            let wanted: Vec<&CfFile> = manifest.files.iter().filter(|f| f.required).collect();
            if !wanted.is_empty() {
                reporter.phase("Resolving CurseForge files");
                let file_ids: Vec<u64> = wanted.iter().map(|f| f.file_id).collect();
                let mod_ids: Vec<u64> = wanted.iter().map(|f| f.project_id).collect();
                let files = content::curseforge_post("/mods/files", json!({"fileIds": file_ids})).await?;
                let mods = content::curseforge_post("/mods", json!({"modIds": mod_ids})).await?;
                let mut class_of: HashMap<u64, u64> = HashMap::new();
                let mut name_of: HashMap<u64, String> = HashMap::new();
                for m in mods["data"].as_array().cloned().unwrap_or_default() {
                    if let Some(id) = m["id"].as_u64() {
                        class_of.insert(id, m["classId"].as_u64().unwrap_or(6));
                        name_of.insert(id, m["name"].as_str().unwrap_or("").to_string());
                    }
                }
                let mut items = Vec::new();
                let mut returned = std::collections::HashSet::new();
                for f in files["data"].as_array().cloned().unwrap_or_default() {
                    let mod_id = f["modId"].as_u64().unwrap_or(0);
                    returned.insert(f["id"].as_u64().unwrap_or(0));
                    let file_name = f["fileName"].as_str().unwrap_or("");
                    let safe_name = Path::new(file_name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let folder = match class_of.get(&mod_id) {
                        Some(12) => "resourcepacks",
                        Some(6552) => "shaderpacks",
                        Some(17) => "saves",
                        _ => "mods",
                    };
                    let Some(url) = f["downloadUrl"].as_str().filter(|u| u.starts_with("https://")) else {
                        let label = name_of.get(&mod_id).cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| safe_name.clone());
                        warnings.push(format!("{label} must be downloaded manually from CurseForge (third-party downloads disabled by its author)"));
                        continue;
                    };
                    if safe_name.is_empty() {
                        continue;
                    }
                    let sha1 = f["hashes"].as_array().and_then(|h| h.iter().find(|x| x["algo"].as_u64() == Some(1))).and_then(|x| x["value"].as_str()).map(String::from);
                    let size = f["fileLength"].as_u64();
                    items.push(DownloadItem::new(url, storage::safe_join(&game_dir, &format!("{folder}/{safe_name}"))?, sha1, size));
                }
                for f in &wanted {
                    if !returned.contains(&f.file_id) {
                        warnings.push(format!("CurseForge file {} (project {}) is no longer available", f.file_id, f.project_id));
                    }
                }
                reporter.phase("Downloading modpack files");
                net::download_all(items, reporter.progress(), true).await?;
            }
        }
        Format::Prism(prefix) => {
            reporter.phase("Copying instance files");
            let archive = open(path)?;
            let names = normalized_names(&archive);
            drop(archive);
            let game_prefix = [".minecraft/", "minecraft/"]
                .iter()
                .map(|p| format!("{prefix}{p}"))
                .find(|p| names.iter().any(|n| n.starts_with(p.as_str())));
            if let Some(game_prefix) = game_prefix {
                let (a, g) = (archive_path.clone(), game_dir.clone());
                blocking(move || extract_prefix(&a, &game_prefix, &g)).await?;
            } else {
                warnings.push("The Prism instance contained no game folder".into());
            }
        }
        Format::Zip(prefix) => {
            reporter.phase("Copying instance files");
            let (a, g, p) = (archive_path.clone(), game_dir.clone(), format!("{prefix}minecraft/"));
            blocking(move || extract_prefix(&a, &p, &g)).await?;
        }
    }
    Ok(())
}

const ICON_LIMIT: usize = 2 * 1024 * 1024;

fn is_image(bytes: &[u8]) -> bool {
    bytes.len() <= ICON_LIMIT
        && (bytes.starts_with(&[0x89, b'P', b'N', b'G'])
            || bytes.starts_with(&[0xFF, 0xD8])
            || bytes.starts_with(b"GIF8")
            || (bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"))
}

fn archive_icon(path: &Path, format: &Format) -> Option<Vec<u8>> {
    let mut archive = open(path).ok()?;
    let candidates: Vec<String> = match format {
        Format::Zip(prefix) => vec![format!("{prefix}icon.png")],
        Format::Prism(prefix) => {
            let cfg = read_text(&mut archive, &format!("{prefix}instance.cfg")).map(|t| parse_cfg(&t)).unwrap_or_default();
            let key = cfg.get("iconKey").cloned().unwrap_or_default();
            if key.is_empty() || key.contains(['/', '\\']) {
                return None;
            }
            ["png", "jpg", "jpeg", "webp", "gif"].iter().map(|ext| format!("{prefix}{key}.{ext}")).collect()
        }
        Format::Mrpack | Format::CurseForge => vec!["icon.png".into()],
    };
    candidates.iter().find_map(|name| {
        let index = find_entry(&archive, name)?;
        let mut entry = archive.by_index(index).ok()?;
        if entry.size() > ICON_LIMIT as u64 {
            return None;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).ok()?;
        Some(bytes).filter(|b| is_image(b))
    })
}

async fn remote_icon(path: &Path, format: &Format, instance: &mut Instance) -> Option<Vec<u8>> {
    let info = instance.modpack.as_mut()?;
    if info.project_id.is_empty() && *format == Format::Mrpack {
        let file = path.to_path_buf();
        if let Ok(sha1) = blocking(move || net::sha1_file(&file)).await {
            if let Some(id) = content::modrinth_project_by_hash(&sha1).await {
                info.project_id = id;
            }
        }
    }
    let url = if info.project_id.is_empty() {
        content::modpack_icon_by_name(&info.source, &info.name).await
    } else {
        content::project_icon(&info.source, &info.project_id).await
    }?;
    net::get_bytes(&url).await.ok().filter(|b| is_image(b))
}

async fn attach_icon(path: &Path, format: &Format, instance: &mut Instance) {
    let bytes = match archive_icon(path, format) {
        Some(bytes) => Some(bytes),
        None => remote_icon(path, format, instance).await,
    };
    if let (Some(bytes), Ok(dir)) = (bytes, instances::dir(&instance.id)) {
        if storage::write_bytes_atomic(&dir.join("icon.png"), &bytes).is_ok() {
            instance.icon = format!("custom:{}", instances::now());
        }
    }
    let _ = instances::save(instance);
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::msg(e.to_string()))?
}

pub async fn install_modrinth_pack(project_id: &str, version_id: Option<String>, reporter: &Reporter) -> Result<ImportResult> {
    let versions = content::versions("modrinth", project_id, "modpack", "", "").await?;
    let chosen = match version_id.filter(|v| !v.is_empty()) {
        Some(id) => versions.into_iter().find(|v| v.id == id),
        None => {
            let stable = versions.iter().position(|v| v.channel == "release").unwrap_or(0);
            versions.into_iter().nth(stable)
        }
    }
    .ok_or_else(|| Error::msg("No downloadable version of this modpack was found"))?;
    if !chosen.file_name.ends_with(".mrpack") {
        return Err(Error::msg("This modpack version has no .mrpack file"));
    }
    let target: PathBuf = storage::cache_dir().join("packs").join(format!("{}.mrpack", chosen.id));
    reporter.phase("Downloading modpack");
    net::download_all(
        vec![DownloadItem::new(chosen.url.clone(), target.clone(), (!chosen.sha1.is_empty()).then(|| chosen.sha1.clone()), (chosen.size > 0).then_some(chosen.size))],
        reporter.progress(),
        true,
    )
    .await?;
    let mut result = import(&target, reporter).await?;
    if let Some(info) = result.instance.modpack.as_mut() {
        info.project_id = project_id.to_string();
    }
    instances::save(&result.instance)?;
    let _ = std::fs::remove_file(&target);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pack.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, body) in entries {
            writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
        (dir, path)
    }

    #[test]
    fn detects_formats() {
        let (_d, p) = make_zip(&[("modrinth.index.json", b"{}")]);
        assert_eq!(detect(&mut open(&p).unwrap()).unwrap(), Format::Mrpack);
        let (_d, p) = make_zip(&[("manifest.json", br#"{"manifestType":"minecraftModpack"}"#)]);
        assert_eq!(detect(&mut open(&p).unwrap()).unwrap(), Format::CurseForge);
        let (_d, p) = make_zip(&[("My Pack/instance.cfg", b"name=x"), ("My Pack/.minecraft/options.txt", b"")]);
        assert_eq!(detect(&mut open(&p).unwrap()).unwrap(), Format::Prism("My Pack/".into()));
        let (_d, p) = make_zip(&[("mmc-pack.json", b"{}")]);
        assert_eq!(detect(&mut open(&p).unwrap()).unwrap(), Format::Prism(String::new()));
        let (_d, p) = make_zip(&[("zip-instance.json", b"{}")]);
        assert_eq!(detect(&mut open(&p).unwrap()).unwrap(), Format::Zip(String::new()));
        let (_d, p) = make_zip(&[("Win Pack\\instance.cfg", b"name=x"), ("Win Pack\\mmc-pack.json", b"{}")]);
        assert_eq!(detect(&mut open(&p).unwrap()).unwrap(), Format::Prism("Win Pack/".into()));
        assert_eq!(read_text(&mut open(&p).unwrap(), "Win Pack/instance.cfg").unwrap(), "name=x");
        let (_d, p) = make_zip(&[("random.txt", b"x")]);
        assert!(detect(&mut open(&p).unwrap()).is_err());
    }

    #[test]
    fn extraction_blocks_traversal() {
        let (_d, p) = make_zip(&[("overrides/config/a.toml", b"ok"), ("overrides/../../evil.txt", b"bad")]);
        let out = tempfile::tempdir().unwrap();
        let result = extract_prefix(&p, "overrides/", out.path());
        assert!(result.is_err());
        assert!(!out.path().parent().unwrap().join("evil.txt").exists());
        let (_d, p) = make_zip(&[("overrides/config/a.toml", b"ok"), ("other/x", b"skip")]);
        let out = tempfile::tempdir().unwrap();
        assert_eq!(extract_prefix(&p, "overrides/", out.path()).unwrap(), 1);
        assert_eq!(std::fs::read(out.path().join("config/a.toml")).unwrap(), b"ok");
        assert!(!out.path().join("x").exists());
    }

    #[test]
    fn loader_mapping() {
        let mut deps = HashMap::new();
        deps.insert("minecraft".to_string(), "1.20.1".to_string());
        deps.insert("forge".to_string(), "47.2.0".to_string());
        assert_eq!(loader_from_mrpack("1.20.1", &deps), ("forge".into(), "1.20.1-47.2.0".into()));
        let mut deps = HashMap::new();
        deps.insert("neoforge".to_string(), "21.1.77".to_string());
        assert_eq!(loader_from_mrpack("1.21.1", &deps), ("neoforge".into(), "21.1.77".into()));
        let mut deps = HashMap::new();
        deps.insert("fabric-loader".to_string(), "0.16.9".to_string());
        assert_eq!(loader_from_mrpack("1.21.1", &deps), ("fabric".into(), "0.16.9".into()));
        assert_eq!(loader_from_curseforge("1.20.1", "forge-47.2.0"), ("forge".into(), "1.20.1-47.2.0".into()));
        assert_eq!(loader_from_curseforge("1.20.1", "neoforge-47.1.106"), ("neoforge".into(), "1.20.1-47.1.106".into()));
        assert_eq!(loader_from_curseforge("1.21.1", "fabric-0.16.9"), ("fabric".into(), "0.16.9".into()));
        let comps = vec![json!({"uid":"net.minecraft","version":"1.20.1"}), json!({"uid":"net.minecraftforge","version":"47.2.0"})];
        assert_eq!(loader_from_prism(&comps), ("1.20.1".into(), "forge".into(), "1.20.1-47.2.0".into()));
        let comps = vec![json!({"uid":"net.minecraft","version":"1.21.1"}), json!({"uid":"org.quiltmc.quilt-loader","version":"0.27.1"})];
        assert_eq!(loader_from_prism(&comps).1, "quilt");
    }

    #[test]
    fn mrpack_hosts() {
        assert!(host_allowed("https://cdn.modrinth.com/data/x.jar"));
        assert!(!host_allowed("http://cdn.modrinth.com/data/x.jar"));
        assert!(!host_allowed("https://evil.example/x.jar"));
        assert!(!host_allowed("https://cdn.modrinth.com.evil.example/x.jar"));
    }

    #[test]
    fn parses_prism_cfg() {
        let cfg = parse_cfg("[General]\nname=My Pack\nMaxMemAlloc=6144\nOverrideMemory=true\n");
        assert_eq!(cfg.get("name").unwrap(), "My Pack");
        assert_eq!(cfg.get("MaxMemAlloc").unwrap(), "6144");
    }
}
