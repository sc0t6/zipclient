use crate::error::{Error, Result};
use crate::storage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const MANIFEST_NAME: &str = "zip-client.json";
const STATE_NAME: &str = ".zip-client-state.json";

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct PackageFile {
    pub source: String,
    pub target: String,
    pub sha256: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct PackageManifest {
    pub id: String,
    pub version: String,
    pub minecraft: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<PackageFile>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ZipClientStatus {
    pub installed: bool,
    pub available: bool,
    pub version: Option<String>,
    pub label: String,
}

#[derive(Serialize, Deserialize, Default)]
struct InstanceState {
    version: String,
    files: Vec<String>,
}

pub fn packages_dir() -> PathBuf {
    storage::root().join("zipclient").join("packages")
}

fn file_sha256(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(hex::encode(Sha256::digest(&bytes)))
}

pub fn validate(dir: &Path) -> Result<PackageManifest> {
    let manifest: PackageManifest = storage::read_json(&dir.join(MANIFEST_NAME))?.ok_or_else(|| Error::msg("Missing zip-client.json"))?;
    if manifest.id != "zip-client" || manifest.version.trim().is_empty() || manifest.files.is_empty() {
        return Err(Error::msg("Invalid Zip Client manifest"));
    }
    for file in &manifest.files {
        let source = storage::safe_join(dir, &file.source)?;
        storage::safe_join(Path::new("game"), &file.target)?;
        if !file.target.starts_with("mods/") {
            return Err(Error::msg("Zip Client files may only be installed into the mods folder"));
        }
        match file_sha256(&source) {
            Some(hash) if hash.eq_ignore_ascii_case(&file.sha256) => {}
            _ => return Err(Error::msg(format!("Zip Client file {} failed verification", file.source))),
        }
    }
    Ok(manifest)
}

fn version_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit()).filter_map(|p| p.parse().ok()).collect()
}

pub fn discover_in(base: &Path) -> Option<(PathBuf, PackageManifest)> {
    let mut best: Option<(PathBuf, PackageManifest)> = None;
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let dir = entry.path();
        if let Ok(manifest) = validate(&dir) {
            let newer = best.as_ref().map(|(_, b)| version_key(&manifest.version) > version_key(&b.version)).unwrap_or(true);
            if newer {
                best = Some((dir, manifest));
            }
        }
    }
    best
}

fn compatible(manifest: &PackageManifest, minecraft: &str, loader: &str) -> bool {
    manifest.minecraft.iter().any(|m| m == minecraft) && manifest.loaders.iter().any(|l| l == loader)
}

pub fn status_in(base: &Path, minecraft: &str, loader: &str) -> ZipClientStatus {
    match discover_in(base) {
        None => ZipClientStatus { installed: false, available: false, version: None, label: "Zip Client — Coming Soon".into() },
        Some((_, manifest)) if !compatible(&manifest, minecraft, loader) => ZipClientStatus {
            installed: true,
            available: false,
            version: Some(manifest.version.clone()),
            label: format!("Zip Client {} does not support {loader} {minecraft}", manifest.version),
        },
        Some((_, manifest)) => ZipClientStatus {
            installed: true,
            available: true,
            version: Some(manifest.version.clone()),
            label: format!("Zip Client {}", manifest.version),
        },
    }
}

pub fn status_for(minecraft: &str, loader: &str) -> ZipClientStatus {
    status_in(&packages_dir(), minecraft, loader)
}

fn remove_tracked(game_dir: &Path) -> Result<()> {
    let state_path = game_dir.join(STATE_NAME);
    let Some(state) = storage::read_json::<InstanceState>(&state_path)? else { return Ok(()) };
    for target in &state.files {
        let path = storage::safe_join(game_dir, target)?;
        if path.is_file() {
            std::fs::remove_file(path)?;
        }
    }
    std::fs::remove_file(state_path)?;
    Ok(())
}

pub fn apply_in(base: &Path, game_dir: &Path, enabled: bool, minecraft: &str, loader: &str) -> Result<()> {
    let found = discover_in(base).filter(|(_, m)| compatible(m, minecraft, loader));
    let Some((dir, manifest)) = found.filter(|_| enabled) else {
        return remove_tracked(game_dir);
    };
    let state_path = game_dir.join(STATE_NAME);
    let current: InstanceState = storage::read_json(&state_path)?.unwrap_or_default();
    if current.version == manifest.version && current.files.iter().all(|f| game_dir.join(f).is_file()) {
        return Ok(());
    }
    remove_tracked(game_dir)?;
    let mut installed = Vec::new();
    for file in &manifest.files {
        let target = storage::safe_join(game_dir, &file.target)?;
        if target.exists() {
            return Err(Error::msg(format!("{} already exists and is not managed by Zip Client", file.target)));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(storage::safe_join(&dir, &file.source)?, &target)?;
        installed.push(file.target.clone());
    }
    storage::write_json(&state_path, &InstanceState { version: manifest.version, files: installed })
}

pub fn apply(game_dir: &Path, enabled: bool, minecraft: &str, loader: &str) -> Result<()> {
    apply_in(&packages_dir(), game_dir, enabled, minecraft, loader)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_package(base: &Path, version: &str, body: &[u8]) {
        let dir = base.join(format!("zip-client-{version}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("client.jar"), body).unwrap();
        let manifest = serde_json::json!({
            "id": "zip-client", "version": version, "minecraft": ["1.21.1"], "loaders": ["fabric"],
            "files": [{"source": "client.jar", "target": "mods/zip-client.jar", "sha256": hex::encode(Sha256::digest(body))}]
        });
        std::fs::write(dir.join(MANIFEST_NAME), serde_json::to_vec(&manifest).unwrap()).unwrap();
    }

    #[test]
    fn unavailable_without_package() {
        let base = tempfile::tempdir().unwrap();
        let status = status_in(base.path(), "1.21.1", "fabric");
        assert!(!status.available && !status.installed);
        let game = tempfile::tempdir().unwrap();
        apply_in(base.path(), game.path(), true, "1.21.1", "fabric").unwrap();
        assert!(!game.path().join("mods").exists());
    }

    #[test]
    fn installs_enables_and_removes_only_tracked_files() {
        let base = tempfile::tempdir().unwrap();
        make_package(base.path(), "1.0.0", b"v1");
        make_package(base.path(), "1.2.0", b"v2");
        assert!(status_in(base.path(), "1.21.1", "fabric").available);
        assert!(!status_in(base.path(), "1.21.1", "forge").available);
        let game = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(game.path().join("mods")).unwrap();
        std::fs::write(game.path().join("mods").join("other.jar"), b"x").unwrap();
        apply_in(base.path(), game.path(), true, "1.21.1", "fabric").unwrap();
        assert_eq!(std::fs::read(game.path().join("mods/zip-client.jar")).unwrap(), b"v2");
        apply_in(base.path(), game.path(), false, "1.21.1", "fabric").unwrap();
        assert!(!game.path().join("mods/zip-client.jar").exists());
        assert!(game.path().join("mods/other.jar").exists());
    }

    #[test]
    fn rejects_tampered_package() {
        let base = tempfile::tempdir().unwrap();
        make_package(base.path(), "1.0.0", b"v1");
        std::fs::write(base.path().join("zip-client-1.0.0").join("client.jar"), b"tampered").unwrap();
        assert!(!status_in(base.path(), "1.21.1", "fabric").installed);
    }
}
