use crate::error::{Error, Result};
use crate::storage;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const INSTANCE_FILE: &str = "instance.json";
pub const EXPORT_MANIFEST: &str = "zip-instance.json";
pub const PRESET_ICONS: [&str; 8] = ["grass", "pickaxe", "sword", "gem", "tree", "flame", "star", "potion"];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct PlaySession {
    pub started: u64,
    pub duration: u64,
    pub exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ModpackInfo {
    pub source: String,
    pub name: String,
    pub version: String,
    pub project_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub group: String,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: String,
    pub version_id: String,
    pub java_path: String,
    pub memory_min: u32,
    pub memory_max: u32,
    pub width: u32,
    pub height: u32,
    pub jvm_args: String,
    pub game_dir: String,
    pub account_id: String,
    pub zip_client: bool,
    pub memory_warning: bool,
    pub fast_launch: bool,
    pub checked: bool,
    pub created: u64,
    pub last_played: u64,
    pub playtime: u64,
    pub launches: u32,
    pub history: Vec<PlaySession>,
    pub modpack: Option<ModpackInfo>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewInstance {
    pub name: String,
    pub minecraft: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub group: String,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').chars().take(32).collect::<String>();
    if trimmed.is_empty() {
        "instance".into()
    } else {
        trimmed
    }
}

fn unique_id(name: &str) -> String {
    let base = slug(name);
    let root = storage::instances_root();
    if !root.join(&base).exists() {
        return base;
    }
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let mut n = nanos % 0xffff;
    loop {
        let candidate = format!("{base}-{n:04x}");
        if !root.join(&candidate).exists() {
            return candidate;
        }
        n = (n + 1) % 0xffff;
    }
}

pub fn validate_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::msg("Instance name cannot be empty"));
    }
    if trimmed.chars().count() > 64 {
        return Err(Error::msg("Instance name is too long (64 characters max)"));
    }
    Ok(trimmed.to_string())
}

pub fn validate_loader(loader: &str) -> Result<()> {
    if matches!(loader, "vanilla" | "fabric" | "forge" | "neoforge" | "quilt" | "horizon") {
        Ok(())
    } else {
        Err(Error::msg(format!("Unsupported loader: {loader}")))
    }
}

pub fn dir(id: &str) -> Result<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\', ':']) || id.contains("..") {
        return Err(Error::msg("Invalid instance id"));
    }
    Ok(storage::instances_root().join(id))
}

pub fn game_dir(instance: &Instance) -> Result<PathBuf> {
    if !instance.game_dir.trim().is_empty() {
        return Ok(PathBuf::from(instance.game_dir.trim()));
    }
    Ok(dir(&instance.id)?.join("minecraft"))
}

pub fn list() -> Vec<Instance> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(storage::instances_root()) {
        for entry in entries.flatten() {
            let path = entry.path().join(INSTANCE_FILE);
            if let Ok(Some(mut instance)) = storage::read_json::<Instance>(&path) {
                instance.id = entry.file_name().to_string_lossy().to_string();
                out.push(instance);
            }
        }
    }
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

pub fn get(id: &str) -> Result<Instance> {
    let mut instance: Instance = storage::read_json(&dir(id)?.join(INSTANCE_FILE))?
        .ok_or_else(|| Error::msg("Instance not found"))?;
    instance.id = id.to_string();
    Ok(instance)
}

pub fn save(instance: &Instance) -> Result<()> {
    storage::write_json(&dir(&instance.id)?.join(INSTANCE_FILE), instance)
}

pub fn create(request: NewInstance) -> Result<Instance> {
    let name = validate_name(&request.name)?;
    validate_loader(&request.loader)?;
    if request.minecraft.trim().is_empty() {
        return Err(Error::msg("Choose a Minecraft version"));
    }
    if request.loader != "vanilla" && request.loader_version.trim().is_empty() {
        return Err(Error::msg("Choose a loader version"));
    }
    let id = unique_id(&name);
    let instance = Instance {
        id: id.clone(),
        name,
        group: request.group.trim().to_string(),
        minecraft: request.minecraft.trim().to_string(),
        loader: request.loader.clone(),
        loader_version: if request.loader == "vanilla" { String::new() } else { request.loader_version.trim().to_string() },
        created: now(),
        ..Default::default()
    };
    std::fs::create_dir_all(dir(&id)?.join("minecraft"))?;
    save(&instance)?;
    Ok(instance)
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstancePatch {
    pub name: Option<String>,
    pub group: Option<String>,
    pub minecraft: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub java_path: Option<String>,
    pub memory_min: Option<u32>,
    pub memory_max: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub jvm_args: Option<String>,
    pub game_dir: Option<String>,
    pub account_id: Option<String>,
    pub zip_client: Option<bool>,
    pub memory_warning: Option<bool>,
    pub fast_launch: Option<bool>,
    pub icon: Option<String>,
}

pub fn update(id: &str, patch: InstancePatch) -> Result<Instance> {
    let mut instance = get(id)?;
    if let Some(name) = patch.name {
        instance.name = validate_name(&name)?;
    }
    if let Some(group) = patch.group {
        instance.group = group.trim().chars().take(40).collect();
    }
    let mut version_changed = false;
    if let Some(minecraft) = patch.minecraft {
        if minecraft.trim().is_empty() {
            return Err(Error::msg("Minecraft version cannot be empty"));
        }
        version_changed |= minecraft != instance.minecraft;
        instance.minecraft = minecraft;
    }
    if let Some(loader) = patch.loader {
        validate_loader(&loader)?;
        version_changed |= loader != instance.loader;
        instance.loader = loader;
    }
    if let Some(lv) = patch.loader_version {
        version_changed |= lv != instance.loader_version;
        instance.loader_version = lv;
    }
    if instance.loader == "vanilla" {
        instance.loader_version.clear();
    } else if instance.loader_version.is_empty() {
        return Err(Error::msg("Choose a loader version"));
    }
    if version_changed {
        instance.version_id.clear();
        instance.checked = false;
    }
    if let Some(java) = patch.java_path {
        let java = java.trim().to_string();
        if !java.is_empty() && !Path::new(&java).is_file() {
            return Err(Error::msg("That Java executable does not exist"));
        }
        instance.java_path = java;
    }
    if let Some(v) = patch.memory_max {
        instance.memory_max = if v == 0 { 0 } else { v.clamp(512, 65536) };
    }
    if let Some(v) = patch.memory_min {
        instance.memory_min = if v == 0 { 0 } else { v.clamp(256, 65536) };
    }
    if instance.memory_max > 0 && instance.memory_min > instance.memory_max {
        instance.memory_min = instance.memory_max;
    }
    if let Some(v) = patch.width {
        instance.width = v.min(16384);
    }
    if let Some(v) = patch.height {
        instance.height = v.min(16384);
    }
    if let Some(v) = patch.jvm_args {
        instance.jvm_args = v.trim().to_string();
    }
    if let Some(v) = patch.game_dir {
        let v = v.trim().to_string();
        if !v.is_empty() && !Path::new(&v).is_absolute() {
            return Err(Error::msg("Game directory must be an absolute path"));
        }
        instance.game_dir = v;
    }
    if let Some(v) = patch.account_id {
        instance.account_id = v;
    }
    if let Some(warn) = patch.memory_warning {
        instance.memory_warning = warn;
    }
    if let Some(icon) = patch.icon.map(|i| i.trim().to_string()) {
        if !(icon.is_empty() || icon.strip_prefix("preset:").map(|n| PRESET_ICONS.contains(&n)).unwrap_or(false)) {
            return Err(Error::msg("Unknown icon"));
        }
        let _ = std::fs::remove_file(dir(id)?.join("icon.png"));
        instance.icon = icon;
    }
    if let Some(fast) = patch.fast_launch.filter(|f| *f != instance.fast_launch) {
        instance.fast_launch = fast;
        instance.checked = false;
    }
    if let Some(enabled) = patch.zip_client {
        if enabled && !crate::zipclient::status_for(&instance.minecraft, &instance.loader).available {
            return Err(Error::msg("Zip Client is not available for this instance yet"));
        }
        instance.zip_client = enabled;
    }
    save(&instance)?;
    Ok(instance)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub fn duplicate(id: &str) -> Result<Instance> {
    let source = get(id)?;
    let name = format!("{} (copy)", source.name).chars().take(64).collect::<String>();
    let new_id = unique_id(&name);
    copy_dir(&dir(id)?, &dir(&new_id)?)?;
    let mut copy = Instance {
        id: new_id,
        name,
        created: now(),
        last_played: 0,
        playtime: 0,
        launches: 0,
        history: Vec::new(),
        ..source.clone()
    };
    if !source.game_dir.is_empty() {
        let target = dir(&copy.id)?.join("minecraft");
        copy_dir(Path::new(&source.game_dir), &target)?;
        copy.game_dir.clear();
    }
    save(&copy)?;
    Ok(copy)
}

pub fn delete(id: &str) -> Result<()> {
    let path = dir(id)?;
    if !path.join(INSTANCE_FILE).is_file() {
        return Err(Error::msg("Instance not found"));
    }
    std::fs::remove_dir_all(path)?;
    Ok(())
}

pub fn set_icon(id: &str, source: Option<&str>) -> Result<Instance> {
    let mut instance = get(id)?;
    let target = dir(id)?.join("icon.png");
    match source {
        Some(path) => {
            let bytes = std::fs::read(path)?;
            if bytes.len() > 2 * 1024 * 1024 {
                return Err(Error::msg("Icons must be smaller than 2 MB"));
            }
            if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) && !bytes.starts_with(&[0xFF, 0xD8]) {
                return Err(Error::msg("Icons must be PNG or JPEG images"));
            }
            storage::write_bytes_atomic(&target, &bytes)?;
            instance.icon = format!("custom:{}", now());
        }
        None => {
            let _ = std::fs::remove_file(&target);
            instance.icon.clear();
        }
    }
    save(&instance)?;
    Ok(instance)
}

pub fn icon_data(id: &str) -> Option<String> {
    use base64::Engine;
    let bytes = std::fs::read(dir(id).ok()?.join("icon.png")).ok()?;
    let mime = if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

pub fn record_session(id: &str, started: u64, duration: u64, exit_code: Option<i32>) -> Result<Instance> {
    let mut instance = get(id)?;
    instance.playtime += duration;
    instance.launches += 1;
    instance.last_played = started;
    instance.history.insert(0, PlaySession { started, duration, exit_code });
    instance.history.truncate(25);
    save(&instance)?;
    Ok(instance)
}

const EXPORT_SKIP: [&str; 5] = ["logs", "crash-reports", "natives", ".zip-content-cache", "screenshots"];

fn add_dir_to_zip(writer: &mut zip::ZipWriter<std::fs::File>, base: &Path, current: &Path, prefix: &str) -> Result<()> {
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(base).map_err(|_| Error::msg("Path error"))?.to_string_lossy().replace('\\', "/");
        if current == base && EXPORT_SKIP.contains(&relative.as_str()) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            add_dir_to_zip(writer, base, &path, prefix)?;
        } else {
            writer.start_file(format!("{prefix}{relative}"), options)?;
            let mut file = std::fs::File::open(&path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            writer.write_all(&buffer)?;
        }
    }
    Ok(())
}

pub fn export(id: &str, target: &Path) -> Result<()> {
    let instance = get(id)?;
    let game = game_dir(&instance)?;
    let tmp = target.with_extension("zip.part");
    let file = std::fs::File::create(&tmp)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let manifest = Instance {
        id: String::new(),
        version_id: String::new(),
        java_path: String::new(),
        game_dir: String::new(),
        account_id: String::new(),
        history: Vec::new(),
        ..instance.clone()
    };
    writer.start_file(EXPORT_MANIFEST, options)?;
    writer.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    if let Some(icon) = std::fs::read(dir(id)?.join("icon.png")).ok() {
        writer.start_file("icon.png", options)?;
        writer.write_all(&icon)?;
    }
    if game.is_dir() {
        add_dir_to_zip(&mut writer, &game, &game, "minecraft/")?;
    }
    writer.finish()?;
    std::fs::rename(&tmp, target)?;
    Ok(())
}

pub fn create_from(mut template: Instance) -> Result<Instance> {
    template.name = validate_name(&template.name)?;
    validate_loader(&template.loader)?;
    template.id = unique_id(&template.name);
    template.created = now();
    template.version_id.clear();
    template.game_dir.clear();
    template.zip_client = false;
    template.checked = false;
    std::fs::create_dir_all(dir(&template.id)?.join("minecraft"))?;
    save(&template)?;
    Ok(template)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_safe() {
        assert_eq!(slug("My Cool Pack!"), "my-cool-pack");
        assert_eq!(slug("../../etc"), "etc");
        assert_eq!(slug("???"), "instance");
        assert!(dir("../x").is_err());
        assert!(dir("a\\b").is_err());
        assert!(dir("").is_err());
    }

    #[test]
    fn name_validation() {
        assert!(validate_name("  ").is_err());
        assert_eq!(validate_name("  Survival ").unwrap(), "Survival");
        assert!(validate_name(&"x".repeat(65)).is_err());
        assert!(validate_loader("fabric").is_ok());
        assert!(validate_loader("liteloader").is_err());
    }
}
