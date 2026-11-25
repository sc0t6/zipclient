use crate::error::{Error, Result};
use crate::minecraft::{install, java, version};
use crate::net::{self, DownloadItem};
use crate::storage;
use crate::tasks::Reporter;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const RELEASE_API: &str = "https://api.github.com/repos/horizon-rust/horizon/releases/tags/nightly";
const RELEASE_TAG: &str = "nightly";
const LOCK_URL: &str = "https://raw.githubusercontent.com/horizon-rust/horizon/main/research-lock.json";
const RELEASE_KEYS: [&str; 1] = ["MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw="];
const ASSET: &str = if cfg!(windows) { "horizon-windows-x64.zip" } else { "horizon-linux-x86_64.tar.gz" };
const MODS_API: &str = "https://api.github.com/repos/horizon-rust/horizon-mods/contents/mods";
const MODS_RAW: &str = "https://raw.githubusercontent.com/horizon-rust/horizon-mods/main";
const MODS_SITE: &str = "https://horizon-rust.github.io/horizon-mods";
const MODS_RELEASES: &str = "https://github.com/horizon-rust/horizon-mods/releases/download";

#[derive(Deserialize)]
struct Lock {
    minecraft: LockMinecraft,
}

#[derive(Deserialize)]
struct LockMinecraft {
    version: String,
    java_major: u32,
}

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Serialize, Deserialize, Default)]
struct State {
    build: String,
    dir: String,
    prepared: bool,
    resources: String,
    #[serde(default)]
    checked: u64,
}

pub struct Ready {
    pub client: PathBuf,
    pub resources: PathBuf,
}

pub fn root() -> PathBuf {
    storage::game_root().join("horizon")
}

fn load_state() -> State {
    storage::read_json(&root().join("state.json")).ok().flatten().unwrap_or_default()
}

fn save_state(state: &State) -> Result<()> {
    storage::write_json(&root().join("state.json"), state)
}

pub async fn minecraft_version() -> Result<String> {
    let lock: Lock = net::get_json_cached(LOCK_URL, "horizon-lock", 6 * 3600).await?;
    Ok(lock.minecraft.version)
}

pub fn verify_signature(tag: &str, sums: &[u8], signatures: &str, keys: &[&str]) -> bool {
    let mut message = format!("horizon release {tag}\n").into_bytes();
    message.extend_from_slice(sums);
    let engine = base64::engine::general_purpose::STANDARD;
    signatures.lines().filter_map(|l| engine.decode(l.trim()).ok()).any(|signature| {
        keys.iter().filter_map(|k| engine.decode(k).ok()).any(|key| {
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key).verify(&message, &signature).is_ok()
        })
    })
}

pub fn checksum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hex, file) = line.split_once(' ')?;
        let file = file.trim_start_matches([' ', '*']);
        (file == name && hex.len() == 64).then(|| hex.to_ascii_lowercase())
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(hex::encode(Sha256::digest(std::fs::read(path)?)))
}

fn extract(archive: &Path, target: &Path) -> Result<()> {
    if archive.to_string_lossy().ends_with(".tar.gz") {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(std::fs::File::open(archive)?));
        for entry in tar.entries()? {
            let mut entry = entry?;
            let kind = entry.header().entry_type();
            if kind.is_symlink() || kind.is_hard_link() {
                continue;
            }
            let path = entry.path()?.into_owned();
            let mut parts = path.components();
            parts.next();
            let relative = parts.as_path().to_string_lossy().replace('\\', "/");
            if relative.is_empty() {
                continue;
            }
            let out = storage::safe_join(target, &relative)?;
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&out)?;
        }
        return Ok(());
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.enclosed_name().ok_or_else(|| Error::msg("Horizon archive contains an unsafe path"))?;
        let mut parts = name.components();
        parts.next();
        let relative = parts.as_path().to_string_lossy().replace('\\', "/");
        if relative.is_empty() {
            continue;
        }
        let out = storage::safe_join(target, &relative)?;
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
        }
    }
    Ok(())
}

async fn run(exe: &Path, args: &[&str], java_home: &Path, cwd: &Path) -> Result<()> {
    let mut command = tokio::process::Command::new(exe);
    let path = format!("{};{}", java_home.join("bin").display(), std::env::var("PATH").unwrap_or_default());
    command.args(args).current_dir(cwd).env("JAVA_HOME", java_home).env("PATH", path).stdin(std::process::Stdio::null());
    crate::platform::hide_console(&mut command);
    let output = command.output().await.map_err(|e| Error::msg(format!("Could not start {}: {e}", exe.display())))?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stderr).to_string() + &String::from_utf8_lossy(&output.stdout);
    let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    Err(Error::msg(format!("Horizon preparation failed:\n{}", tail[tail.len().saturating_sub(10)..].join("\n"))))
}

async fn prepare(dir: &Path, reporter: &Reporter) -> Result<PathBuf> {
    let lock: Lock = serde_json::from_slice(&std::fs::read(dir.join("research-lock.json"))?)?;
    let minecraft = lock.minecraft.version;
    reporter.phase(&format!("Preparing Minecraft {minecraft}"));
    install::ensure_vanilla_json(&minecraft).await?;
    install::install_version(&minecraft, reporter, false, &version::Features::default()).await?;
    let local = version::load_local(&storage::versions_dir(), &minecraft)?;
    let server = local.downloads.as_ref().and_then(|d| d.get("server")).ok_or_else(|| Error::msg("Mojang lists no server jar for this version"))?;
    let server_jar = storage::versions_dir().join(&minecraft).join(format!("{minecraft}-server.jar"));
    net::download_all(vec![DownloadItem::new(server.url.clone(), server_jar.clone(), server.sha1.clone(), server.size)], reporter.progress(), false).await?;
    let java_ref = version::JavaVersionRef { component: String::new(), major_version: lock.minecraft.java_major };
    let java_exe = java::select(Some(&java_ref), None, reporter.progress()).await?;
    let java_home = java_exe.parent().and_then(Path::parent).ok_or_else(|| Error::msg("Java runtime folder not found"))?.to_path_buf();
    let prepare_exe = dir.join(format!("horizon-prepare{}", crate::platform::EXE));
    let game_root = storage::game_root().to_string_lossy().to_string();
    let dir_text = dir.to_string_lossy().to_string();
    let jar_text = server_jar.to_string_lossy().to_string();
    reporter.phase("Building Horizon resources (first time takes a few minutes)");
    run(&prepare_exe, &["--prepare", "--server-jar", &jar_text, "--minecraft-dir", &game_root, "--root", &dir_text], &java_home, dir).await?;
    reporter.phase("Adding Horizon sounds");
    if let Err(e) = run(&prepare_exe, &["sounds", "--minecraft-dir", &game_root, "--root", &dir_text], &java_home, dir).await {
        crate::diagnostics::error(&format!("Horizon sounds skipped: {}", e.0));
    }
    let base = dir.join(".local").join("horizon-client");
    std::fs::read_dir(&base)
        .ok()
        .and_then(|mut entries| entries.find_map(|e| e.ok().filter(|e| e.path().is_dir()).map(|e| e.path())))
        .ok_or_else(|| Error::msg("Horizon preparation produced no resources"))
}

pub async fn ensure(reporter: &Reporter, force_prepare: bool) -> Result<Ready> {
    let base = root();
    std::fs::create_dir_all(&base)?;
    let mut state = load_state();
    reporter.phase("Checking Horizon");
    let ready_now = state.prepared && base.join(&state.dir).join(format!("horizon-client{}", crate::platform::EXE)).is_file() && Path::new(&state.resources).is_dir();
    let recent = crate::instances::now().saturating_sub(state.checked) < 12 * 3600;
    let release = if ready_now && recent && !force_prepare { None } else { Some(net::get_json::<Release>(RELEASE_API).await) };
    match release.unwrap_or_else(|| Err(Error::msg("skip"))) {
        Ok(release) => {
            let url_of = |name: &str| release.assets.iter().find(|a| a.name == name);
            let (Some(archive), Some(sums), Some(sig)) = (url_of(ASSET), url_of("SHA256SUMS"), url_of("SHA256SUMS.sig")) else {
                return Err(Error::msg("The Horizon release has no Windows build or no signed checksums"));
            };
            let sums_bytes = net::get_bytes(&sums.browser_download_url).await?;
            let sig_text = String::from_utf8_lossy(&net::get_bytes(&sig.browser_download_url).await?).to_string();
            if !verify_signature(RELEASE_TAG, &sums_bytes, &sig_text, &RELEASE_KEYS) {
                return Err(Error::msg("Horizon's release signature did not verify. Zip refused to install it."));
            }
            let expected = checksum_for(&String::from_utf8_lossy(&sums_bytes), ASSET).ok_or_else(|| Error::msg("Horizon checksum list has no Windows build"))?;
            state.checked = crate::instances::now();
            let installed = base.join(&state.dir).join(format!("horizon-client{}", crate::platform::EXE)).is_file();
            if state.build != expected || !installed {
                reporter.phase("Downloading Horizon");
                let zip_path = base.join(ASSET);
                net::download_all(vec![DownloadItem::new(archive.browser_download_url.clone(), zip_path.clone(), None, None)], reporter.progress(), true).await?;
                if sha256_file(&zip_path)? != expected {
                    let _ = std::fs::remove_file(&zip_path);
                    return Err(Error::msg("Horizon download failed its signed checksum"));
                }
                let dir_name = format!("b-{}", &expected[..12]);
                let target = base.join(&dir_name);
                let _ = std::fs::remove_dir_all(&target);
                let (zip_for_task, target_for_task) = (zip_path.clone(), target.clone());
                tokio::task::spawn_blocking(move || extract(&zip_for_task, &target_for_task)).await.map_err(|e| Error::msg(e.to_string()))??;
                let _ = std::fs::remove_file(&zip_path);
                state = State { build: expected, dir: dir_name, prepared: false, resources: String::new(), checked: state.checked };
            }
            save_state(&state)?;
        }
        Err(e) => {
            if state.dir.is_empty() || !state.prepared {
                return Err(e);
            }
        }
    }
    let dir = base.join(&state.dir);
    if !state.prepared || force_prepare || !Path::new(&state.resources).is_dir() {
        let resources = prepare(&dir, reporter).await?;
        state.prepared = true;
        state.resources = resources.to_string_lossy().to_string();
        save_state(&state)?;
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("b-") && name != state.dir {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
    }
    let client = dir.join(format!("horizon-client{}", crate::platform::EXE));
    crate::platform::make_executable(&client)?;
    crate::platform::make_executable(&dir.join(format!("horizon-prepare{}", crate::platform::EXE)))?;
    Ok(Ready { client, resources: PathBuf::from(&state.resources) })
}

#[derive(Deserialize, Clone)]
pub struct ModManifest {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub versions: Vec<ModVersion>,
}

#[derive(Deserialize, Clone)]
pub struct ModVersion {
    pub version: String,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub game: String,
    pub file: ModFile,
    #[serde(default)]
    pub sha256: String,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum ModFile {
    Name(String),
    Full {
        name: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        sha256: String,
    },
}

impl ModFile {
    pub fn name(&self) -> &str {
        match self {
            ModFile::Name(n) => n,
            ModFile::Full { name, .. } => name,
        }
    }
}

impl ModVersion {
    fn normalize(&mut self) {
        if let ModFile::Full { sha256, .. } = &self.file {
            if self.sha256.is_empty() {
                self.sha256 = sha256.clone();
            }
        }
    }
}

#[derive(Deserialize)]
struct Listing {
    name: String,
    sha: String,
    download_url: Option<String>,
}

#[derive(Deserialize, Default)]
struct Revoked {
    #[serde(default)]
    revoked: Vec<RevokedEntry>,
}

#[derive(Deserialize)]
struct RevokedEntry {
    id: String,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Deserialize)]
struct SiteIndex {
    #[serde(default)]
    projects: Vec<ModManifest>,
    #[serde(default)]
    revoked: Vec<RevokedEntry>,
}

static INDEX: std::sync::Mutex<Option<(std::time::Instant, Vec<ModManifest>)>> = std::sync::Mutex::new(None);
const INDEX_TTL: std::time::Duration = std::time::Duration::from_secs(600);

fn finish(mut mods: Vec<ModManifest>, revoked: &[RevokedEntry]) -> Vec<ModManifest> {
    for manifest in &mut mods {
        for v in &mut manifest.versions {
            v.normalize();
        }
        manifest.versions.retain(|v| {
            !revoked.iter().any(|r| r.id == manifest.id && r.sha256.as_deref().map(|s| s.eq_ignore_ascii_case(&v.sha256)).unwrap_or(true))
        });
        if manifest.icon.is_empty() {
            manifest.icon = format!("icons/{}.png", manifest.id);
        }
        if !manifest.icon.starts_with("https://") {
            manifest.icon = format!("{MODS_SITE}/{}", manifest.icon.trim_start_matches('/'));
        }
    }
    mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    mods
}

async fn index_from_site() -> Result<Vec<ModManifest>> {
    let index: SiteIndex = net::get_json_cached(&format!("{MODS_SITE}/index.json"), "horizon-index", 900).await?;
    if index.projects.is_empty() {
        return Err(Error::msg("Horizon mod index is empty"));
    }
    Ok(finish(index.projects, &index.revoked))
}

async fn index_from_repo() -> Result<Vec<ModManifest>> {
    use futures_util::StreamExt;
    let listing: Vec<Listing> = net::get_json_cached(MODS_API, "horizon-mods", 3600).await?;
    let revoked: Revoked = match net::get_bytes(&format!("{MODS_RAW}/revoked.toml")).await {
        Ok(bytes) => toml::from_str(&String::from_utf8_lossy(&bytes)).unwrap_or_default(),
        Err(_) => Revoked::default(),
    };
    let texts: Vec<String> = futures_util::stream::iter(listing.into_iter().filter(|l| l.name.ends_with(".toml")))
        .map(|item| async move {
            let cache = storage::cache_dir().join("meta").join(format!("horizon-mod-{}.toml", item.sha));
            if let Ok(text) = std::fs::read_to_string(&cache) {
                return Some(text);
            }
            let bytes = net::get_bytes(item.download_url.as_deref()?).await.ok()?;
            let text = String::from_utf8_lossy(&bytes).to_string();
            let _ = storage::write_bytes_atomic(&cache, text.as_bytes());
            Some(text)
        })
        .buffer_unordered(12)
        .filter_map(|t| async move { t })
        .collect()
        .await;
    let mods = texts.iter().filter_map(|t| toml::from_str::<ModManifest>(t).ok()).collect();
    Ok(finish(mods, &revoked.revoked))
}

pub async fn mod_index() -> Result<Vec<ModManifest>> {
    if let Some((at, mods)) = INDEX.lock().ok().and_then(|g| g.clone()) {
        if at.elapsed() < INDEX_TTL {
            return Ok(mods);
        }
    }
    let mods = match index_from_site().await {
        Ok(mods) => mods,
        Err(e) => {
            crate::diagnostics::error(&format!("Horizon index.json unavailable, using repository listing: {}", e.0));
            index_from_repo().await?
        }
    };
    if let Ok(mut guard) = INDEX.lock() {
        *guard = Some((std::time::Instant::now(), mods.clone()));
    }
    Ok(mods)
}

pub async fn install_mod(game_dir: &Path, manifest: &ModManifest, chosen: &ModVersion, reporter: &Reporter) -> Result<String> {
    if manifest.kind != "mod" || !chosen.file.name().ends_with(".hmod") {
        return Err(Error::msg(format!("{} is a native mod. Native mods are compiled into the game by the Horizon launcher.", manifest.name)));
    }
    let name = Path::new(chosen.file.name()).file_name().map(|f| f.to_string_lossy().to_string()).ok_or_else(|| Error::msg("Invalid mod file name"))?;
    let target = storage::safe_join(game_dir, &format!("horizon-mods/{name}"))?;
    let url = match &chosen.file {
        ModFile::Full { url, .. } if url.starts_with(MODS_RELEASES) => url.clone(),
        _ => format!("{MODS_RELEASES}/{}-{}/{}", manifest.id, chosen.version, name),
    };
    reporter.phase(&format!("Downloading {name}"));
    net::download_all(vec![DownloadItem::new(url, target.clone(), None, None)], reporter.progress(), true).await?;
    if !sha256_file(&target)?.eq_ignore_ascii_case(&chosen.sha256) {
        let _ = std::fs::remove_file(&target);
        return Err(Error::msg(format!("{name} failed its checksum and was removed")));
    }
    let dir = target.parent().ok_or_else(|| Error::msg("Invalid mod folder"))?;
    for entry in std::fs::read_dir(dir)?.flatten() {
        let other = entry.file_name().to_string_lossy().to_string();
        if other != name && other.starts_with(&format!("{}-", manifest.id)) && (other.ends_with(".hmod") || other.ends_with(".hmod.disabled")) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(format!("horizon-mods/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::KeyPair;

    #[test]
    fn verifies_signed_checksums() {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let engine = base64::engine::general_purpose::STANDARD;
        let key = engine.encode(pair.public_key().as_ref());
        let sums = b"abc  horizon-windows-x64.zip\n";
        let mut message = b"horizon release nightly\n".to_vec();
        message.extend_from_slice(sums);
        let sig = engine.encode(pair.sign(&message).as_ref());
        assert!(verify_signature("nightly", sums, &sig, &[key.as_str()]));
        assert!(!verify_signature("other", sums, &sig, &[key.as_str()]));
        assert!(!verify_signature("nightly", b"tampered", &sig, &[key.as_str()]));
        assert!(!verify_signature("nightly", sums, &sig, &RELEASE_KEYS));
    }

    #[test]
    fn finds_checksums() {
        let sums = "aa  other.zip\n6b5b749a23bc96903141d4d256e7e14389de7bfa377de401827c437647cdfdd4  horizon-windows-x64.zip\n";
        assert_eq!(checksum_for(sums, "horizon-windows-x64.zip").unwrap(), "6b5b749a23bc96903141d4d256e7e14389de7bfa377de401827c437647cdfdd4");
        assert!(checksum_for(sums, "missing.zip").is_none());
    }

    #[test]
    fn parses_mod_manifest() {
        let text = "id = \"zoom\"\nkind = \"native-mod\"\nname = \"Zoom\"\nsummary = \"\"\"\nmulti\n\"\"\"\nauthors = [\"Horizon\"]\n\n[[versions]]\nversion = \"2.0.0\"\npublished = \"2026-10-29\"\ngame = \"26.3\"\nfile = \"zoom-2.0.0.hnmod\"\nsha256 = \"49fa\"\nchangelog = \"x\"\n";
        let m: ModManifest = toml::from_str(text).unwrap();
        assert_eq!(m.versions[0].file.name(), "zoom-2.0.0.hnmod");
        assert_eq!(m.kind, "native-mod");
        let json = r#"{"schema":1,"projects":[{"id":"lights","kind":"mod","name":"Lights","downloads":5,"versions":[{"version":"1.0","game":"26.3","file":{"name":"lights.hmod","url":"https://github.com/horizon-rust/horizon-mods/releases/download/lights-1.0/lights.hmod","size":1,"sha256":"ab"}}]}],"revoked":[{"id":"x","reason":"y"}]}"#;
        let index: SiteIndex = serde_json::from_str(json).unwrap();
        let mods = finish(index.projects, &index.revoked);
        assert_eq!(mods[0].versions[0].sha256, "ab");
        assert_eq!(mods[0].versions[0].file.name(), "lights.hmod");
        assert!(mods[0].icon.ends_with("icons/lights.png"));
    }
}
