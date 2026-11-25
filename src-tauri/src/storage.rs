use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::sync::{OnceLock, RwLock};

pub const SETTINGS_SCHEMA: u32 = 3;
pub const THEMES: [&str; 6] = ["dark", "oled", "midnight", "ember", "forest", "light"];

static ROOT: OnceLock<(PathBuf, bool)> = OnceLock::new();
static SETTINGS: OnceLock<RwLock<Settings>> = OnceLock::new();

pub fn init_root() -> Result<()> {
    let exe_dir = match std::env::var_os("APPIMAGE") {
        Some(image) => Path::new(&image).parent().map(Path::to_path_buf).unwrap_or_default(),
        None => std::env::current_exe()?.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let portable_dir = exe_dir.join("ZipData");
    let (root, portable) = if let Ok(custom) = std::env::var("ZIP_DATA_DIR") {
        (PathBuf::from(custom), false)
    } else if portable_dir.is_dir() || exe_dir.join("zip-portable.txt").is_file() {
        (portable_dir, true)
    } else {
        let base = dirs::data_dir().ok_or_else(|| Error::msg("Could not locate the user data folder"))?;
        (base.join("Zip"), false)
    };
    std::fs::create_dir_all(&root)?;
    let _ = ROOT.set((root, portable));
    Ok(())
}

pub fn root() -> &'static Path {
    &ROOT.get().expect("root initialised").0
}

pub fn is_portable() -> bool {
    ROOT.get().map(|r| r.1).unwrap_or(false)
}

pub fn game_root() -> PathBuf {
    let custom = settings().minecraft.game_dir.clone();
    if custom.trim().is_empty() {
        root().join("minecraft")
    } else {
        PathBuf::from(custom)
    }
}

pub fn instances_root() -> PathBuf {
    let custom = settings().general.instances_dir.clone();
    if custom.trim().is_empty() {
        root().join("instances")
    } else {
        PathBuf::from(custom)
    }
}

pub fn libraries_dir() -> PathBuf {
    game_root().join("libraries")
}

pub fn versions_dir() -> PathBuf {
    game_root().join("versions")
}

pub fn assets_dir() -> PathBuf {
    game_root().join("assets")
}

pub fn runtimes_dir() -> PathBuf {
    game_root().join("runtimes")
}

pub fn cache_dir() -> PathBuf {
    root().join("cache")
}

pub fn logs_dir() -> PathBuf {
    root().join("logs")
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|e| {
            Error::msg(format!("{} is corrupted: {e}", path.display()))
        })?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp{}",
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        std::process::id()
    ));
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    write_bytes_atomic(path, &serde_json::to_vec_pretty(value)?)
}

pub fn safe_join(base: &Path, relative: &str) -> Result<PathBuf> {
    let rel = Path::new(relative);
    let mut out = base.to_path_buf();
    for component in rel.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err(Error::msg(format!("Rejected unsafe path: {relative}"))),
        }
    }
    if out == base {
        return Err(Error::msg(format!("Rejected empty path: {relative}")));
    }
    Ok(out)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct SetupState {
    pub completed: bool,
    pub step: u8,
    pub legal: String,
}

impl Default for SetupState {
    fn default() -> Self {
        SetupState { completed: false, step: 1, legal: String::new() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct GeneralSettings {
    pub language: String,
    pub startup_page: String,
    pub after_launch: String,
    pub check_updates: bool,
    pub update_endpoint: String,
    pub skipped_update: String,
    pub notifications: bool,
    pub confirm_exit: bool,
    pub instances_dir: String,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        GeneralSettings {
            language: "en".into(),
            startup_page: "home".into(),
            after_launch: "keep".into(),
            check_updates: true,
            update_endpoint: option_env!("ZIP_UPDATE_ENDPOINT").unwrap_or("").into(),
            skipped_update: String::new(),
            notifications: true,
            confirm_exit: true,
            instances_dir: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: String,
    pub accent: String,
    pub scale: f64,
    pub animations: String,
    pub background: String,
    pub background_dim: f64,
    pub follow_system_motion: bool,
    pub nav: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        AppearanceSettings {
            theme: "dark".into(),
            accent: "#7fcf8b".into(),
            scale: 1.25,
            animations: "full".into(),
            background: "image".into(),
            background_dim: 0.78,
            follow_system_motion: true,
            nav: "top".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct MinecraftSettings {
    pub java_mode: String,
    pub java_path: String,
    pub memory_min: u32,
    pub memory_max: u32,
    pub jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub game_dir: String,
    pub default_account: String,
}

impl Default for MinecraftSettings {
    fn default() -> Self {
        MinecraftSettings {
            java_mode: "managed".into(),
            java_path: String::new(),
            memory_min: 512,
            memory_max: 4096,
            jvm_args: String::new(),
            width: 0,
            height: 0,
            game_dir: String::new(),
            default_account: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct DownloadSettings {
    pub concurrency: u32,
    pub retries: u32,
    pub timeout_secs: u32,
    pub cache_limit_mb: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        DownloadSettings { concurrency: 12, retries: 3, timeout_secs: 30, cache_limit_mb: 200 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AdvancedSettings {
    pub debug_logging: bool,
    pub last_page: String,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        AdvancedSettings {
            debug_logging: false,
            last_page: "home".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub schema: u32,
    pub setup: SetupState,
    pub general: GeneralSettings,
    pub appearance: AppearanceSettings,
    pub minecraft: MinecraftSettings,
    pub downloads: DownloadSettings,
    pub advanced: AdvancedSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: SETTINGS_SCHEMA,
            setup: SetupState::default(),
            general: GeneralSettings::default(),
            appearance: AppearanceSettings::default(),
            minecraft: MinecraftSettings::default(),
            downloads: DownloadSettings::default(),
            advanced: AdvancedSettings::default(),
        }
    }
}

impl Settings {
    pub fn validated(mut self) -> Self {
        let s = &mut self;
        s.schema = SETTINGS_SCHEMA;
        s.setup.step = s.setup.step.clamp(1, 5);
        if !matches!(s.general.startup_page.as_str(), "home" | "last") {
            s.general.startup_page = "home".into();
        }
        if !matches!(s.general.after_launch.as_str(), "keep" | "minimize" | "close") {
            s.general.after_launch = "keep".into();
        }
        if !THEMES.contains(&s.appearance.theme.as_str()) {
            s.appearance.theme = "dark".into();
        }
        if !is_hex_color(&s.appearance.accent) {
            s.appearance.accent = AppearanceSettings::default().accent;
        }
        if !s.appearance.scale.is_finite() {
            s.appearance.scale = 1.0;
        }
        s.appearance.scale = s.appearance.scale.clamp(0.75, 1.5);
        if !matches!(s.appearance.animations.as_str(), "full" | "reduced" | "off") {
            s.appearance.animations = "full".into();
        }
        if !matches!(s.appearance.background.as_str(), "image" | "none") {
            s.appearance.background = "image".into();
        }
        s.appearance.background_dim = s.appearance.background_dim.clamp(0.0, 0.95);
        if !matches!(s.appearance.nav.as_str(), "top" | "side" | "icons") {
            s.appearance.nav = "top".into();
        }
        if !LANGUAGES.contains(&s.general.language.as_str()) {
            s.general.language = "en".into();
        }
        if !matches!(s.minecraft.java_mode.as_str(), "managed" | "system" | "custom") {
            s.minecraft.java_mode = "managed".into();
        }
        s.minecraft.memory_max = s.minecraft.memory_max.clamp(512, 65536);
        s.minecraft.memory_min = s.minecraft.memory_min.clamp(256, s.minecraft.memory_max);
        s.minecraft.width = s.minecraft.width.min(16384);
        s.minecraft.height = s.minecraft.height.min(16384);
        s.downloads.concurrency = s.downloads.concurrency.clamp(1, 32);
        s.downloads.retries = s.downloads.retries.min(10);
        s.downloads.timeout_secs = s.downloads.timeout_secs.clamp(5, 300);
        s.downloads.cache_limit_mb = s.downloads.cache_limit_mb.clamp(20, 4096);
        self
    }
}

pub const LANGUAGES: [&str; 7] = ["en", "es", "pt-BR", "de", "fr", "pl", "ru"];

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

pub fn migrate_settings(mut raw: serde_json::Value) -> Settings {
    let schema = raw.get("schema").and_then(|v| v.as_u64()).unwrap_or(1);
    if schema < 2 {
        if let Some(obj) = raw.as_object_mut() {
            if let Some(theme) = obj.remove("theme") {
                let appearance = obj
                    .entry("appearance")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(a) = appearance.as_object_mut() {
                    a.insert("theme".into(), theme);
                }
            }
            if let Some(ram) = obj.remove("ram_mb") {
                let minecraft = obj.entry("minecraft").or_insert_with(|| serde_json::json!({}));
                if let Some(m) = minecraft.as_object_mut() {
                    m.insert("memory_max".into(), ram);
                }
            }
        }
    }
    if schema < 3 {
        if let Some(appearance) = raw.get_mut("appearance").and_then(|a| a.as_object_mut()) {
            if appearance.get("scale").and_then(|v| v.as_f64()) == Some(1.0) {
                appearance.insert("scale".into(), serde_json::json!(1.25));
            }
        }
    }
    serde_json::from_value::<Settings>(raw).unwrap_or_default().validated()
}

fn settings_path() -> PathBuf {
    root().join("settings.json")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    let loaded = match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(raw) => migrate_settings(raw),
            Err(_) => {
                let _ = std::fs::copy(&path, path.with_extension("json.corrupt"));
                Settings::default()
            }
        },
        Err(_) => Settings::default(),
    };
    let _ = SETTINGS.set(RwLock::new(loaded.clone()));
    loaded
}

pub fn settings() -> Settings {
    SETTINGS
        .get()
        .map(|s| s.read().map(|g| g.clone()).unwrap_or_default())
        .unwrap_or_default()
}

pub fn save_settings(next: Settings) -> Result<Settings> {
    let next = next.validated();
    write_json(&settings_path(), &next)?;
    if let Some(lock) = SETTINGS.get() {
        if let Ok(mut guard) = lock.write() {
            *guard = next.clone();
        }
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_rejects_traversal() {
        let base = Path::new("C:/base");
        assert!(safe_join(base, "../evil").is_err());
        assert!(safe_join(base, "a/../../evil").is_err());
        if cfg!(windows) {
            assert!(safe_join(base, "C:/Windows/evil").is_err());
        }
        assert!(safe_join(base, "/abs").is_err());
        assert!(safe_join(base, "").is_err());
        assert_eq!(safe_join(base, "mods/a.jar").unwrap(), base.join("mods").join("a.jar"));
        assert_eq!(safe_join(base, "./config/x.toml").unwrap(), base.join("config").join("x.toml"));
    }

    #[test]
    fn settings_validation_clamps_values() {
        let mut s = Settings::default();
        s.minecraft.memory_max = 10;
        s.minecraft.memory_min = 99999;
        s.downloads.concurrency = 0;
        s.appearance.accent = "red".into();
        s.appearance.scale = 9.0;
        s.appearance.theme = "neon".into();
        let v = s.validated();
        assert_eq!(v.minecraft.memory_max, 512);
        assert_eq!(v.minecraft.memory_min, 512);
        assert_eq!(v.downloads.concurrency, 1);
        assert_eq!(v.appearance.accent, "#7fcf8b");
        assert_eq!(v.appearance.scale, 1.5);
        let mut o = Settings::default();
        o.appearance.theme = "oled".into();
        assert_eq!(o.validated().appearance.theme, "oled");
        assert_eq!(v.appearance.theme, "dark");
    }

    #[test]
    fn settings_migrate_from_v1() {
        let raw = serde_json::json!({"schema": 1, "theme": "light", "ram_mb": 6144, "general": {"confirm_exit": false}});
        let s = migrate_settings(raw);
        assert_eq!(s.schema, SETTINGS_SCHEMA);
        assert_eq!(s.appearance.theme, "light");
        assert_eq!(s.minecraft.memory_max, 6144);
        assert!(!s.general.confirm_exit);
        assert!(s.general.check_updates);
        assert_eq!(s.appearance.scale, 1.25);
        let kept = migrate_settings(serde_json::json!({"schema": 2, "appearance": {"scale": 0.9}}));
        assert_eq!(kept.appearance.scale, 0.9);
    }

    #[test]
    fn settings_unknown_fields_and_garbage_fall_back() {
        let s = migrate_settings(serde_json::json!({"schema": 2, "nonsense": true, "downloads": {"retries": 99}}));
        assert_eq!(s.downloads.retries, 10);
        let d = migrate_settings(serde_json::json!("not an object"));
        assert_eq!(d.schema, SETTINGS_SCHEMA);
    }

    #[test]
    fn atomic_write_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("x.json");
        write_json(&p, &serde_json::json!({"a": 1})).unwrap();
        write_json(&p, &serde_json::json!({"a": 2})).unwrap();
        let v: serde_json::Value = read_json(&p).unwrap().unwrap();
        assert_eq!(v["a"], 2);
        assert!(read_json::<serde_json::Value>(&dir.path().join("missing.json")).unwrap().is_none());
    }
}
