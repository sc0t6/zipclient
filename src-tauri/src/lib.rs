mod auth;
mod content;
mod crash;
mod diagnostics;
mod error;
mod game;
mod horizon;
mod instances;
mod minecraft;
mod modcheck;
mod modpack;
mod nbt;
mod net;
mod news;
mod optimize;
mod platform;
mod servers;
mod skins;
mod storage;
mod tasks;
mod updater;
mod zipclient;

use error::{Error, Result};
use minecraft::{install, java, loaders};
use serde::Serialize;
use std::path::PathBuf;
use tasks::Reporter;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    platform: String,
    portable: bool,
    data_dir: String,
    ms_configured: bool,
    curseforge_configured: bool,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        portable: storage::is_portable(),
        data_dir: storage::root().to_string_lossy().to_string(),
        ms_configured: auth::client_id().is_some(),
        curseforge_configured: content::curseforge_key().is_some(),
    }
}

#[tauri::command]
fn app_exit(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn settings_get() -> storage::Settings {
    storage::settings()
}

#[tauri::command]
fn settings_save(settings: storage::Settings) -> Result<storage::Settings> {
    storage::save_settings(settings)
}

#[tauri::command]
fn settings_export(path: String) -> Result<()> {
    storage::write_json(&PathBuf::from(path), &storage::settings())
}

#[tauri::command]
fn settings_import(path: String) -> Result<storage::Settings> {
    let bytes = std::fs::read(&path)?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| Error::msg("That file is not a Zip settings export"))?;
    if !raw.is_object() || raw.get("appearance").is_none() {
        return Err(Error::msg("That file is not a Zip settings export"));
    }
    let mut imported = storage::migrate_settings(raw);
    let current = storage::settings();
    imported.setup = current.setup;
    storage::save_settings(imported)
}

#[tauri::command]
fn settings_reset() -> Result<storage::Settings> {
    let current = storage::settings();
    let mut fresh = storage::Settings::default();
    fresh.setup = current.setup;
    storage::save_settings(fresh)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionList {
    latest_release: String,
    latest_snapshot: String,
    versions: Vec<install::ManifestVersion>,
}

#[tauri::command]
async fn minecraft_versions(force: bool) -> Result<VersionList> {
    let manifest = install::manifest(force).await?;
    Ok(VersionList { latest_release: manifest.latest.release, latest_snapshot: manifest.latest.snapshot, versions: manifest.versions })
}

#[tauri::command]
async fn loader_versions(loader: String, minecraft: String) -> Result<Vec<loaders::LoaderVersion>> {
    loaders::list(&loader, &minecraft).await
}

#[tauri::command]
async fn loader_supported(loader: String) -> Result<Vec<String>> {
    loaders::supported_minecraft(&loader).await
}

#[tauri::command]
async fn java_detect() -> Vec<java::JavaInstall> {
    tokio::task::spawn_blocking(java::detect).await.unwrap_or_default()
}

#[tauri::command]
fn java_check(path: String) -> Result<java::JavaInstall> {
    java::java_exe_version(&PathBuf::from(&path)).ok_or_else(|| Error::msg("Not a recognised Java installation (missing release file next to bin)"))
}

#[tauri::command]
async fn java_install_runtime(app: AppHandle, component: String) -> Result<String> {
    let reporter = Reporter::new(&app, format!("java:{component}"));
    let _lock = game::INSTALL_LOCK.lock().await;
    reporter.phase(&format!("Downloading {component}"));
    let path = java::ensure_runtime(&component, reporter.progress(), true).await?;
    Ok(path.to_string_lossy().to_string())
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::msg(e.to_string()))?
}

#[tauri::command]
async fn instances_list() -> Vec<instances::Instance> {
    tokio::task::spawn_blocking(instances::list).await.unwrap_or_default()
}

#[tauri::command]
fn instance_get(id: String) -> Result<instances::Instance> {
    instances::get(&id)
}

#[tauri::command]
fn instance_create(request: instances::NewInstance) -> Result<instances::Instance> {
    instances::create(request)
}

#[tauri::command]
fn instance_update(id: String, patch: instances::InstancePatch) -> Result<instances::Instance> {
    instances::update(&id, patch)
}

#[tauri::command]
async fn instance_duplicate(id: String) -> Result<instances::Instance> {
    blocking(move || instances::duplicate(&id)).await
}

#[tauri::command]
async fn instance_delete(id: String) -> Result<()> {
    if game::states().iter().any(|s| s.instance_id == id) {
        return Err(Error::msg("Stop the game before deleting this instance"));
    }
    blocking(move || instances::delete(&id)).await
}

#[tauri::command]
async fn instance_set_icon(id: String, path: Option<String>) -> Result<instances::Instance> {
    blocking(move || instances::set_icon(&id, path.as_deref())).await
}

#[tauri::command]
async fn instance_icon(id: String) -> Option<String> {
    tokio::task::spawn_blocking(move || instances::icon_data(&id)).await.ok().flatten()
}

#[tauri::command]
async fn instance_export(id: String, path: String) -> Result<()> {
    tokio::task::spawn_blocking(move || instances::export(&id, &PathBuf::from(path))).await.map_err(|e| Error::msg(e.to_string()))?
}

#[tauri::command]
async fn instance_import(app: AppHandle, path: String) -> Result<modpack::ImportResult> {
    let reporter = Reporter::new(&app, "import");
    let result = modpack::import(&PathBuf::from(path), &reporter).await;
    reporter.phase("");
    result
}

#[tauri::command]
async fn modpack_install(app: AppHandle, project_id: String, version_id: Option<String>) -> Result<modpack::ImportResult> {
    let reporter = Reporter::new(&app, "import");
    let result = modpack::install_modrinth_pack(&project_id, version_id, &reporter).await;
    reporter.phase("");
    result
}

fn open_in_explorer(path: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    platform::open(path)
}

#[tauri::command]
async fn instance_open_folder(id: String, sub: Option<String>) -> Result<()> {
    let instance = instances::get(&id)?;
    let mut path = instances::game_dir(&instance)?;
    if let Some(sub) = sub.filter(|s| !s.is_empty()) {
        path = storage::safe_join(&path, &sub)?;
    }
    open_in_explorer(&path)
}

#[tauri::command]
async fn open_folder(kind: String) -> Result<()> {
    let path = match kind.as_str() {
        "data" => storage::root().to_path_buf(),
        "logs" => storage::logs_dir(),
        "instances" => storage::instances_root(),
        "game" => storage::game_root(),
        "zipclient" => zipclient::packages_dir(),
        _ => return Err(Error::msg("Unknown folder")),
    };
    open_in_explorer(&path)
}

#[tauri::command]
async fn open_file(path: String) -> Result<()> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err(Error::msg("File not found"));
    }
    let allowed = [storage::root().to_path_buf(), storage::instances_root()];
    if !allowed.iter().any(|root| path.starts_with(root)) && !instances::list().iter().any(|i| instances::game_dir(i).map(|g| path.starts_with(g)).unwrap_or(false)) {
        return Err(Error::msg("Refusing to open files outside Zip folders"));
    }
    if cfg!(windows) {
        std::process::Command::new("notepad").arg(&path).spawn().map_err(|e| Error::msg(e.to_string()))?;
        Ok(())
    } else {
        platform::open(&path)
    }
}

#[tauri::command]
async fn instance_install(app: AppHandle, id: String, verify: bool) -> Result<instances::Instance> {
    let mut instance = instances::get(&id)?;
    let reporter = Reporter::new(&app, id.clone());
    let _lock = game::INSTALL_LOCK.lock().await;
    if verify {
        let natives = storage::versions_dir().join(&instance.version_id).join("natives");
        let _ = std::fs::remove_file(natives.join(".zip-natives"));
        if instance.loader == "forge" || instance.loader == "neoforge" {
            let registry_path = storage::game_root().join("loader-installs.json");
            if let Ok(Some(mut registry)) = storage::read_json::<std::collections::HashMap<String, String>>(&registry_path) {
                registry.remove(&format!("{}:{}", instance.loader, instance.loader_version));
                storage::write_json(&registry_path, &registry)?;
            }
        }
    }
    let result = game::ensure_installed(&mut instance, &reporter, verify).await;
    reporter.phase("");
    result?;
    Ok(instance)
}

#[tauri::command]
async fn game_launch(app: AppHandle, id: String, server: Option<String>) -> Result<()> {
    game::launch(app, id, server).await
}

#[tauri::command]
fn game_kill(id: String) -> Result<()> {
    game::kill(&id)
}

#[tauri::command]
fn game_states() -> Vec<game::GameState> {
    game::states()
}

#[tauri::command]
fn game_logs(id: String) -> Vec<String> {
    game::logs(&id)
}

#[tauri::command]
fn accounts_list() -> Vec<auth::PublicAccount> {
    auth::list()
}

#[tauri::command]
fn account_add_offline(name: String) -> Result<auth::PublicAccount> {
    auth::add_offline(&name)
}

#[tauri::command]
async fn account_begin_microsoft(app: AppHandle) -> Result<auth::DeviceCodeInfo> {
    auth::begin_device_flow(app).await
}

#[tauri::command]
fn account_cancel_microsoft() {
    auth::cancel_device_flow();
}

#[tauri::command]
fn account_set_active(id: String) -> Result<()> {
    auth::set_active(&id)
}

#[tauri::command]
fn account_remove(id: String) -> Result<()> {
    auth::remove(&id)
}

#[tauri::command]
async fn account_refresh(id: String) -> Result<Vec<auth::PublicAccount>> {
    auth::refresh_profile(&id).await?;
    Ok(auth::list())
}

#[tauri::command]
async fn content_search(query: content::SearchQuery) -> Result<content::SearchResult> {
    content::search(query).await
}

#[tauri::command]
async fn content_categories(source: String, kind: String) -> Result<Vec<content::Category>> {
    content::categories(&source, &kind).await
}

#[tauri::command]
async fn content_versions(source: String, project_id: String, kind: String, minecraft: String, loader: String) -> Result<Vec<content::ContentVersion>> {
    content::versions(&source, &project_id, &kind, &minecraft, &loader).await
}

#[tauri::command]
async fn content_project(source: String, project_id: String) -> Result<content::ProjectDetails> {
    content::project(&source, &project_id).await
}

fn ui_background_file() -> std::path::PathBuf {
    storage::root().join("ui-background")
}

#[tauri::command]
async fn ui_background_set(path: String) -> Result<String> {
    blocking(move || {
        if std::fs::metadata(&path)?.len() > 15 * 1024 * 1024 {
            return Err(Error::msg("Pick an image smaller than 15 MB."));
        }
        let bytes = std::fs::read(&path)?;
        let url = content::data_url(&bytes).ok_or_else(|| Error::msg("Pick a PNG, JPEG, GIF or WebP image."))?;
        std::fs::write(ui_background_file(), &bytes)?;
        Ok(url)
    })
    .await
}

#[tauri::command]
async fn ui_background_get() -> Option<String> {
    tokio::task::spawn_blocking(|| std::fs::read(ui_background_file()).ok().and_then(|b| content::data_url(&b))).await.ok().flatten()
}

#[tauri::command]
async fn ui_background_clear() -> Result<()> {
    match std::fs::remove_file(ui_background_file()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

#[tauri::command]
async fn content_install(
    app: AppHandle,
    instance_id: String,
    source: String,
    project_id: String,
    version_id: Option<String>,
    kind: String,
) -> Result<content::InstallOutcome> {
    let reporter = Reporter::new(&app, format!("content:{project_id}"));
    let result = content::install(&instance_id, &source, &project_id, version_id, &kind, &reporter).await;
    reporter.phase("");
    result
}

#[tauri::command]
async fn content_list(instance_id: String, kind: String, identify: bool) -> Result<Vec<content::InstalledFile>> {
    content::list_installed(&instance_id, &kind, identify).await
}

#[tauri::command]
async fn content_toggle(instance_id: String, path: String, enabled: bool) -> Result<String> {
    content::set_enabled(&instance_id, &path, enabled)
}

#[tauri::command]
async fn content_remove(instance_id: String, path: String) -> Result<()> {
    blocking(move || content::remove(&instance_id, &path)).await
}

#[tauri::command]
async fn content_import(instance_id: String, kind: String, paths: Vec<String>) -> Result<Vec<String>> {
    blocking(move || content::import_local(&instance_id, &kind, paths)).await
}

#[tauri::command]
async fn content_updates(instance_id: String, kind: String) -> Result<Vec<content::UpdateInfo>> {
    content::check_updates(&instance_id, &kind).await
}

#[tauri::command]
async fn content_update(app: AppHandle, instance_id: String, path: String, version_id: String) -> Result<()> {
    let reporter = Reporter::new(&app, format!("update:{path}"));
    let result = content::update_file(&instance_id, &path, &version_id, &reporter).await;
    reporter.phase("");
    result
}

#[tauri::command]
async fn skins_library() -> skins::SkinLibrary {
    tokio::task::spawn_blocking(skins::library).await.unwrap_or(skins::SkinLibrary { skins: Vec::new(), selected: Default::default() })
}

#[tauri::command]
async fn skin_import(path: String, name: String, variant: String) -> Result<skins::SkinEntry> {
    skins::import(&path, &name, &variant)
}

#[tauri::command]
fn skin_remove(id: String) -> Result<()> {
    skins::remove(&id)
}

#[tauri::command]
fn skin_set_variant(id: String, variant: String) -> Result<()> {
    skins::set_variant(&id, &variant)
}

#[tauri::command]
fn skin_select_local(account_id: String, skin_id: Option<String>) -> Result<()> {
    skins::select_local(&account_id, skin_id.as_deref())
}

#[tauri::command]
async fn skin_texture(url: String) -> Result<String> {
    skins::texture(&url).await
}

#[tauri::command]
async fn skin_default(variant: String) -> Result<String> {
    skins::texture(if variant == "slim" { skins::ALEX } else { skins::STEVE }).await
}

#[tauri::command]
async fn skin_upload(account_id: String, skin_id: String) -> Result<Vec<auth::PublicAccount>> {
    skins::upload(&account_id, &skin_id).await?;
    Ok(auth::list())
}

#[tauri::command]
async fn news_list(source: String) -> Result<Vec<news::NewsItem>> {
    if source == "zip" {
        news::zip_news().await
    } else {
        news::minecraft_news().await
    }
}

#[tauri::command]
async fn optimize_preset(minecraft: String, loader: String) -> Result<optimize::Preset> {
    optimize::preset_for(&minecraft, &loader).await
}

#[tauri::command]
async fn update_check(app: AppHandle) -> updater::UpdateStatus {
    updater::check(&app).await
}

#[tauri::command]
async fn update_install(app: AppHandle) -> Result<()> {
    updater::install(&app).await
}

#[tauri::command]
fn zipclient_status(minecraft: String, loader: String) -> zipclient::ZipClientStatus {
    zipclient::status_for(&minecraft, &loader)
}

#[tauri::command]
fn servers_saved() -> Vec<servers::SavedServer> {
    servers::saved()
}

#[tauri::command]
fn server_save(server: servers::SavedServer) -> Result<servers::SavedServer> {
    servers::save_server(server)
}

#[tauri::command]
fn server_remove(id: String) -> Result<()> {
    servers::remove_saved(&id)
}

#[tauri::command]
async fn server_ping(address: String) -> servers::ServerStatus {
    servers::ping(&address).await
}

#[tauri::command]
async fn hosted_list() -> Vec<servers::HostedServer> {
    tokio::task::spawn_blocking(servers::hosted_list).await.unwrap_or_default()
}

#[tauri::command]
fn hosted_create(request: servers::NewHostedServer) -> Result<servers::HostedServer> {
    servers::hosted_create(request)
}

#[tauri::command]
async fn hosted_delete(id: String) -> Result<()> {
    blocking(move || servers::hosted_delete(&id)).await
}

#[tauri::command]
async fn hosted_start(app: AppHandle, id: String) -> Result<()> {
    servers::start(app, id).await
}

#[tauri::command]
fn hosted_stop(app: AppHandle, id: String) -> Result<()> {
    servers::stop(&app, &id)
}

#[tauri::command]
fn hosted_command(id: String, line: String) -> Result<()> {
    servers::command(&id, &line)
}

#[tauri::command]
fn hosted_logs(id: String) -> Vec<String> {
    servers::logs(&id)
}

#[tauri::command]
async fn hosted_open_folder(id: String) -> Result<()> {
    open_in_explorer(&servers::hosted_dir_path(&id)?)
}

#[tauri::command]
async fn lan_address() -> Option<String> {
    servers::lan_address()
}

#[tauri::command]
async fn cache_info() -> u64 {
    tokio::task::spawn_blocking(|| net::dir_size(&storage::cache_dir())).await.unwrap_or(0)
}

#[tauri::command]
fn cache_clear() -> Result<()> {
    net::clear_cache()
}

#[tauri::command]
async fn diagnostics_info() -> Result<diagnostics::SystemInfo> {
    tokio::task::spawn_blocking(diagnostics::system_info).await.map_err(|e| Error::msg(e.to_string()))
}

#[tauri::command]
fn log_frontend_error(message: String) {
    diagnostics::error(&format!("UI: {}", message.chars().take(2000).collect::<String>()));
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            storage::init_root().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            let settings = storage::load_settings();
            for dir in [storage::instances_root(), storage::game_root(), storage::cache_dir(), storage::logs_dir()] {
                let _ = std::fs::create_dir_all(dir);
            }
            diagnostics::info(&format!("Zip {} started (portable: {})", env!("CARGO_PKG_VERSION"), storage::is_portable()));
            let limit = settings.downloads.cache_limit_mb as u64 * 1024 * 1024;
            std::thread::spawn(move || {
                net::prune_cache(limit);
            });
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if storage::settings().general.confirm_exit {
                    api.prevent_close();
                    let _ = window.emit("close-requested", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            app_exit,
            settings_get,
            settings_save,
            settings_export,
            settings_import,
            settings_reset,
            minecraft_versions,
            loader_versions,
            loader_supported,
            java_detect,
            java_check,
            java_install_runtime,
            instances_list,
            instance_get,
            instance_create,
            instance_update,
            instance_duplicate,
            instance_delete,
            instance_set_icon,
            instance_icon,
            instance_export,
            instance_import,
            modpack_install,
            instance_open_folder,
            open_folder,
            open_file,
            instance_install,
            game_launch,
            game_kill,
            game_states,
            game_logs,
            accounts_list,
            account_add_offline,
            account_begin_microsoft,
            account_cancel_microsoft,
            account_set_active,
            account_remove,
            account_refresh,
            content_search,
            content_categories,
            content_versions,
            content_project,
            ui_background_set,
            ui_background_get,
            ui_background_clear,
            content_install,
            content_list,
            content_toggle,
            content_remove,
            content_import,
            content_updates,
            content_update,
            skins_library,
            skin_import,
            skin_remove,
            skin_set_variant,
            skin_select_local,
            skin_texture,
            skin_default,
            skin_upload,
            news_list,
            optimize_preset,
            update_check,
            update_install,
            zipclient_status,
            cache_info,
            cache_clear,
            diagnostics_info,
            log_frontend_error,
            servers_saved,
            server_save,
            server_remove,
            server_ping,
            hosted_list,
            hosted_create,
            hosted_delete,
            hosted_start,
            hosted_stop,
            hosted_command,
            hosted_logs,
            hosted_open_folder,
            lan_address
        ])
        .build(tauri::generate_context!())
        .expect("error while running Zip")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                servers::shutdown();
            }
        });
}
