use crate::error::{Error, Result};
use crate::net::{self, DownloadItem, Progress};
use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RUNTIME_INDEX: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Serialize, Clone, Debug)]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub vendor: String,
    pub source: String,
}

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: RuntimeManifestRef,
    version: RuntimeVersion,
}

#[derive(Deserialize)]
struct RuntimeManifestRef {
    sha1: String,
    url: String,
}

#[derive(Deserialize)]
struct RuntimeVersion {
    name: String,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    downloads: Option<RuntimeDownloads>,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: RuntimeDownload,
}

#[derive(Deserialize)]
struct RuntimeDownload {
    sha1: String,
    size: u64,
    url: String,
}

pub fn parse_major(version: &str) -> u32 {
    let trimmed = version.trim().trim_matches('"');
    let mut parts = trimmed.split(|c: char| c == '.' || c == '_' || c == '-' || c == '+');
    let first: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    if first == 1 {
        parts.next().and_then(|p| p.parse().ok()).unwrap_or(0)
    } else {
        first
    }
}

fn read_release(home: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let mut version = None;
    let mut vendor = String::from("Unknown");
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("JAVA_VERSION=") {
            version = Some(v.trim_matches('"').to_string());
        } else if let Some(v) = line.strip_prefix("IMPLEMENTOR=") {
            vendor = v.trim_matches('"').to_string();
        }
    }
    version.map(|v| (v, vendor))
}

fn java_in(home: &Path) -> PathBuf {
    home.join("bin").join(format!("java{}", crate::platform::EXE))
}

fn probe(home: &Path, source: &str, out: &mut Vec<JavaInstall>) {
    let exe = java_in(home);
    if !exe.is_file() {
        return;
    }
    let canonical = exe.to_string_lossy().to_string();
    if out.iter().any(|j| j.path.eq_ignore_ascii_case(&canonical)) {
        return;
    }
    let Some((version, vendor)) = read_release(home) else { return };
    out.push(JavaInstall { path: canonical, major: parse_major(&version), version, vendor, source: source.into() });
}

fn probe_children(dir: &Path, source: &str, out: &mut Vec<JavaInstall>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            probe(&entry.path(), source, out);
        }
    }
}

fn probe_mojang_runtime_dir(dir: &Path, source: &str, out: &mut Vec<JavaInstall>) {
    if let Ok(components) = std::fs::read_dir(dir) {
        for component in components.flatten() {
            let name = component.file_name().to_string_lossy().to_string();
            probe(&component.path().join(crate::platform::MOJANG_RUNTIME_PLATFORM).join(&name), source, out);
            probe(&component.path(), source, out);
        }
    }
}

pub fn detect() -> Vec<JavaInstall> {
    let mut found = Vec::new();
    probe_mojang_runtime_dir(&storage::runtimes_dir(), "managed", &mut found);
    if let Ok(home) = std::env::var("JAVA_HOME") {
        probe(Path::new(&home), "system", &mut found);
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join(format!("java{}", crate::platform::EXE)).is_file() {
                if let Some(home) = dir.parent() {
                    probe(home, "system", &mut found);
                }
            }
        }
    }
    let vendor_dirs = [
        "Java",
        "Eclipse Adoptium",
        "Eclipse Foundation",
        "AdoptOpenJDK",
        "Zulu",
        "Microsoft",
        "BellSoft",
        "Amazon Corretto",
        "Semeru",
        "ojdkbuild",
    ];
    if cfg!(windows) {
        for base in ["C:\\Program Files", "C:\\Program Files (x86)"] {
            for vendor in vendor_dirs {
                probe_children(&Path::new(base).join(vendor), "system", &mut found);
            }
        }
    } else {
        for base in ["/usr/lib/jvm", "/usr/lib64/jvm", "/usr/java", "/opt"] {
            probe_children(Path::new(base), "system", &mut found);
        }
        if let Some(home) = dirs::home_dir() {
            probe_mojang_runtime_dir(&home.join(".minecraft").join("runtime"), "minecraft", &mut found);
        }
    }
    if let Some(appdata) = dirs::data_dir().filter(|_| cfg!(windows)) {
        probe_mojang_runtime_dir(&appdata.join(".minecraft").join("runtime"), "minecraft", &mut found);
    }
    if let Some(local) = dirs::data_local_dir().filter(|_| cfg!(windows)) {
        probe_mojang_runtime_dir(
            &local.join("Packages").join("Microsoft.4297127D64EC6_8wekyb3d8bbwe").join("LocalCache").join("Local").join("runtime"),
            "minecraft",
            &mut found,
        );
    }
    found.sort_by(|a, b| b.major.cmp(&a.major).then(a.source.cmp(&b.source)));
    found
}

pub fn java_exe_version(path: &Path) -> Option<JavaInstall> {
    let home = path.parent()?.parent()?;
    let (version, vendor) = read_release(home)?;
    Some(JavaInstall { path: path.to_string_lossy().to_string(), major: parse_major(&version), version, vendor, source: "custom".into() })
}

pub fn managed_runtime_path(component: &str) -> PathBuf {
    storage::runtimes_dir().join(component)
}

async fn runtime_works(exe: &Path) -> bool {
    let mut command = tokio::process::Command::new(exe);
    command.arg("-version").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    match tokio::time::timeout(std::time::Duration::from_secs(20), command.status()).await {
        Ok(Ok(status)) => status.success(),
        _ => false,
    }
}

fn component_major(component: &str) -> u32 {
    match component {
        "jre-legacy" => 8,
        "java-runtime-alpha" => 16,
        "java-runtime-beta" | "java-runtime-gamma" | "java-runtime-gamma-snapshot" => 17,
        "java-runtime-delta" => 21,
        _ => 25,
    }
}

#[derive(Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
    release_name: String,
}

#[derive(Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Deserialize)]
struct AdoptiumPackage {
    link: String,
    checksum: String,
    name: String,
    size: u64,
}

pub async fn ensure_temurin(major: u32, progress: Arc<Progress>) -> Result<PathBuf> {
    if !cfg!(windows) {
        return Err(Error::msg(format!("Java {major} could not be prepared. Install Java {major} with your package manager and choose it in Settings > Minecraft.")));
    }
    let home = storage::runtimes_dir().join(format!("temurin-{major}"));
    let exe = java_in(&home);
    let marker = home.join(".zip-temurin");
    if exe.is_file() && marker.is_file() {
        return Ok(exe);
    }
    let url = format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture=x64&image_type=jre&os=windows&vendor=eclipse");
    let assets: Vec<AdoptiumAsset> = net::get_json_cached(&url, &format!("adoptium-{major}"), 24 * 3600).await?;
    let asset = assets
        .into_iter()
        .find(|a| a.binary.package.name.ends_with(".zip") && a.binary.package.link.starts_with("https://github.com/adoptium/"))
        .ok_or_else(|| Error::msg(format!("No Temurin {major} runtime is published for Windows x64")))?;
    let archive = storage::runtimes_dir().join(format!("temurin-{major}.zip"));
    net::download_all(vec![DownloadItem::new(asset.binary.package.link.clone(), archive.clone(), None, Some(asset.binary.package.size))], progress, false).await?;
    let expected = asset.binary.package.checksum.to_ascii_lowercase();
    let archive_for_task = archive.clone();
    let home_for_task = home.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        use sha2::Digest;
        let bytes = std::fs::read(&archive_for_task)?;
        if hex::encode(sha2::Sha256::digest(&bytes)) != expected {
            let _ = std::fs::remove_file(&archive_for_task);
            return Err(Error::msg("Temurin download failed checksum verification"));
        }
        if home_for_task.exists() {
            std::fs::remove_dir_all(&home_for_task)?;
        }
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let Some(name) = entry.enclosed_name() else { return Err(Error::msg("Runtime archive contains unsafe paths")) };
            let mut components = name.components();
            components.next();
            let relative = components.as_path().to_string_lossy().replace('\\', "/");
            if relative.is_empty() {
                continue;
            }
            let target = storage::safe_join(&home_for_task, &relative)?;
            if entry.is_dir() {
                std::fs::create_dir_all(&target)?;
            } else {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&target)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        let _ = std::fs::remove_file(&archive_for_task);
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    if !runtime_works(&exe).await {
        return Err(Error::msg(format!("Temurin {} could not start on this system", asset.release_name)));
    }
    std::fs::write(&marker, &asset.release_name)?;
    Ok(exe)
}

pub async fn ensure_runtime(component: &str, progress: Arc<Progress>, verify: bool) -> Result<PathBuf> {
    let home = managed_runtime_path(component);
    if home.join(".zip-broken").is_file() && !verify {
        return ensure_temurin(component_major(component), progress).await;
    }
    match ensure_mojang_runtime(component, progress.clone(), verify).await {
        Ok(exe) => {
            let checked = home.join(".zip-checked");
            let marker = std::fs::read_to_string(home.join(".zip-runtime")).unwrap_or_default();
            if std::fs::read_to_string(&checked).map(|c| c == marker).unwrap_or(false) || runtime_works(&exe).await {
                let _ = std::fs::write(&checked, marker);
                return Ok(exe);
            }
            let _ = std::fs::write(home.join(".zip-broken"), "Mojang runtime failed to start");
            crate::diagnostics::error(&format!("Mojang runtime {component} failed its self-test; using Eclipse Temurin instead"));
            ensure_temurin(component_major(component), progress).await
        }
        Err(e) => match ensure_temurin(component_major(component), progress).await {
            Ok(exe) => Ok(exe),
            Err(_) => Err(e),
        },
    }
}

async fn ensure_mojang_runtime(component: &str, progress: Arc<Progress>, verify: bool) -> Result<PathBuf> {
    let home = managed_runtime_path(component);
    let exe = java_in(&home);
    let marker = home.join(".zip-runtime");
    let index: HashMap<String, HashMap<String, Vec<RuntimeEntry>>> =
        match net::get_json_cached(RUNTIME_INDEX, "java-runtimes", 6 * 3600).await {
            Ok(index) => index,
            Err(e) => {
                if exe.is_file() {
                    return Ok(exe);
                }
                return Err(e);
            }
        };
    let entry = index
        .get(crate::platform::MOJANG_RUNTIME_PLATFORM)
        .and_then(|platform| platform.get(component))
        .and_then(|entries| entries.first())
        .ok_or_else(|| Error::msg(format!("Mojang does not provide Java runtime '{component}' for {}", crate::platform::MOJANG_RUNTIME_PLATFORM)))?;
    let current_marker = std::fs::read_to_string(&marker).unwrap_or_default();
    if exe.is_file() && current_marker.trim() == entry.manifest.sha1 && !verify {
        return Ok(exe);
    }
    let manifest: RuntimeManifest = net::get_json(&entry.manifest.url).await?;
    let mut items = Vec::new();
    let mut executables = Vec::new();
    let mut links = Vec::new();
    for (relative, file) in &manifest.files {
        let target = storage::safe_join(&home, relative)?;
        match file.kind.as_str() {
            "directory" => std::fs::create_dir_all(&target)?,
            "file" => {
                if let Some(downloads) = &file.downloads {
                    if file.executable {
                        executables.push(target.clone());
                    }
                    items.push(DownloadItem::new(
                        downloads.raw.url.clone(),
                        target,
                        Some(downloads.raw.sha1.clone()),
                        Some(downloads.raw.size),
                    ));
                }
            }
            "link" => {
                if let Some(link) = &file.target {
                    links.push((target, link.clone()));
                }
            }
            _ => {}
        }
    }
    net::download_all(items, progress, verify || current_marker.trim() != entry.manifest.sha1).await?;
    for path in &executables {
        crate::platform::make_executable(path)?;
    }
    #[cfg(unix)]
    for (path, link) in &links {
        if Path::new(link).is_absolute() {
            continue;
        }
        let _ = std::fs::remove_file(path);
        std::os::unix::fs::symlink(link, path)?;
    }
    #[cfg(not(unix))]
    let _ = links;
    std::fs::write(&marker, &entry.manifest.sha1)?;
    if !exe.is_file() {
        return Err(Error::msg(format!("Java runtime {} ({}) is missing its java executable after install", component, entry.version.name)));
    }
    Ok(exe)
}

pub fn component_for(java: Option<&super::version::JavaVersionRef>) -> (String, u32) {
    match java {
        Some(j) if !j.component.is_empty() => (j.component.clone(), j.major_version.max(8)),
        Some(j) if j.major_version > 0 => (
            match j.major_version {
                8 => "jre-legacy",
                16 => "java-runtime-alpha",
                17 => "java-runtime-gamma",
                21 => "java-runtime-delta",
                _ => "java-runtime-epsilon",
            }
            .to_string(),
            j.major_version,
        ),
        _ => ("jre-legacy".to_string(), 8),
    }
}

pub fn quick_select(java: Option<&super::version::JavaVersionRef>, instance_override: Option<&str>) -> Option<PathBuf> {
    if let Some(custom) = instance_override.filter(|p| !p.trim().is_empty()) {
        return Some(PathBuf::from(custom)).filter(|p| p.is_file());
    }
    let settings = storage::settings();
    match settings.minecraft.java_mode.as_str() {
        "custom" => return Some(PathBuf::from(&settings.minecraft.java_path)).filter(|p| p.is_file()),
        "system" => return None,
        _ => {}
    }
    let (component, major) = component_for(java);
    let home = managed_runtime_path(&component);
    if home.join(".zip-broken").is_file() {
        let temurin = storage::runtimes_dir().join(format!("temurin-{major}"));
        return temurin.join(".zip-temurin").is_file().then(|| java_in(&temurin)).filter(|p| p.is_file());
    }
    let exe = java_in(&home);
    (exe.is_file() && home.join(".zip-checked").is_file()).then_some(exe)
}

pub async fn select(
    java: Option<&super::version::JavaVersionRef>,
    instance_override: Option<&str>,
    progress: Arc<Progress>,
) -> Result<PathBuf> {
    let (component, major) = component_for(java);
    if let Some(custom) = instance_override.filter(|p| !p.trim().is_empty()) {
        let path = PathBuf::from(custom);
        if !path.is_file() {
            return Err(Error::msg(format!("The instance Java path does not exist: {custom}")));
        }
        return Ok(path);
    }
    let settings = storage::settings();
    match settings.minecraft.java_mode.as_str() {
        "custom" if !settings.minecraft.java_path.trim().is_empty() => {
            let path = PathBuf::from(&settings.minecraft.java_path);
            if path.is_file() {
                return Ok(path);
            }
            Err(Error::msg(format!("The default Java path does not exist: {}", settings.minecraft.java_path)))
        }
        "system" => {
            let detected = detect();
            if let Some(found) = detected.iter().find(|j| j.major == major && j.source != "managed") {
                return Ok(PathBuf::from(&found.path));
            }
            ensure_runtime(&component, progress, false).await
        }
        _ => ensure_runtime(&component, progress, false).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_major_parsing() {
        assert_eq!(parse_major("1.8.0_51"), 8);
        assert_eq!(parse_major("\"17.0.2\""), 17);
        assert_eq!(parse_major("21"), 21);
        assert_eq!(parse_major("25.0.1+8"), 25);
        assert_eq!(parse_major("garbage"), 0);
    }

    #[test]
    fn component_selection() {
        use super::super::version::JavaVersionRef;
        assert_eq!(component_for(None), ("jre-legacy".to_string(), 8));
        let j = JavaVersionRef { component: "java-runtime-delta".into(), major_version: 21 };
        assert_eq!(component_for(Some(&j)).0, "java-runtime-delta");
        let j = JavaVersionRef { component: String::new(), major_version: 17 };
        assert_eq!(component_for(Some(&j)).0, "java-runtime-gamma");
    }

    #[test]
    fn detects_release_file() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("jdk");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(java_in(&home), b"").unwrap();
        std::fs::write(home.join("release"), "IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"17.0.9\"\n").unwrap();
        let mut out = Vec::new();
        probe(&home, "system", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].major, 17);
        assert_eq!(out[0].vendor, "Eclipse Adoptium");
    }
}
