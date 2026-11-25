use crate::storage;
use serde::Serialize;
use std::io::Write;
use std::sync::Mutex;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn log(level: &str, message: &str) {
    if level == "debug" && !storage::settings().advanced.debug_logging {
        return;
    }
    let Ok(_guard) = WRITE_LOCK.lock() else { return };
    let dir = storage::logs_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("launcher.log");
    if std::fs::metadata(&path).map(|m| m.len() > 2 * 1024 * 1024).unwrap_or(false) {
        let _ = std::fs::rename(&path, dir.join("launcher.old.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "[{}] [{}] {}", crate::instances::now(), level.to_uppercase(), message);
    }
}

pub fn info(message: &str) {
    log("info", message);
}

pub fn error(message: &str) {
    log("error", message);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub launcher_version: String,
    pub os: String,
    pub arch: String,
    pub data_dir: String,
    pub game_dir: String,
    pub instances_dir: String,
    pub portable: bool,
    pub cache_bytes: u64,
    pub java: Vec<crate::minecraft::java::JavaInstall>,
    pub instance_count: usize,
    pub ms_auth_configured: bool,
    pub curseforge_configured: bool,
    pub update_source_configured: bool,
}

pub fn system_info() -> SystemInfo {
    let settings = storage::settings();
    SystemInfo {
        launcher_version: env!("CARGO_PKG_VERSION").into(),
        os: crate::platform::os_name(),
        arch: std::env::consts::ARCH.into(),
        data_dir: storage::root().to_string_lossy().to_string(),
        game_dir: storage::game_root().to_string_lossy().to_string(),
        instances_dir: storage::instances_root().to_string_lossy().to_string(),
        portable: storage::is_portable(),
        cache_bytes: crate::net::dir_size(&storage::cache_dir()),
        java: crate::minecraft::java::detect(),
        instance_count: crate::instances::list().len(),
        ms_auth_configured: crate::auth::client_id().is_some(),
        curseforge_configured: crate::content::curseforge_key().is_some(),
        update_source_configured: !settings.general.update_endpoint.trim().is_empty() || option_env!("ZIP_UPDATE_ENDPOINT").is_some(),
    }
}
