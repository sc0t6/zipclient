use crate::error::{Error, Result};
use crate::storage;
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub configured: bool,
    pub available: bool,
    pub current: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub portable: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

fn self_managed_elsewhere() -> bool {
    storage::is_portable() || std::env::var_os("FLATPAK_ID").is_some()
}

fn endpoint() -> Option<tauri::Url> {
    let configured = storage::settings().general.update_endpoint.trim().to_string();
    let raw = if configured.is_empty() { option_env!("ZIP_UPDATE_ENDPOINT").unwrap_or("").to_string() } else { configured };
    if raw.is_empty() {
        return None;
    }
    tauri::Url::parse(&raw).ok().filter(|u| u.scheme() == "https")
}

fn base(configured: bool) -> UpdateStatus {
    UpdateStatus {
        configured,
        available: false,
        current: env!("CARGO_PKG_VERSION").into(),
        version: None,
        notes: None,
        portable: self_managed_elsewhere(),
        skipped: false,
        error: None,
    }
}

async fn find(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>> {
    let url = endpoint().ok_or_else(|| Error::msg("No update source is configured"))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| Error::msg(e.to_string()))?
        .build()
        .map_err(|e| Error::msg(e.to_string()))?;
    updater.check().await.map_err(|e| Error::msg(format!("Update check failed: {e}")))
}

fn friendly(error: &str) -> String {
    if error.contains("release JSON") {
        "No update information is published on GitHub yet.".into()
    } else if error.contains("error sending request") || error.contains("dns") || error.contains("connect") {
        "Couldn't reach GitHub. Check your internet connection.".into()
    } else {
        error.to_string()
    }
}

pub async fn check(app: &AppHandle) -> UpdateStatus {
    if endpoint().is_none() {
        return base(false);
    }
    let mut status = base(true);
    match find(app).await {
        Ok(Some(update)) => {
            status.available = true;
            status.skipped = storage::settings().general.skipped_update == update.version;
            status.version = Some(update.version.clone());
            status.notes = update.body.clone();
        }
        Ok(None) => {}
        Err(e) => status.error = Some(friendly(&e.0)),
    }
    status
}

pub async fn install(app: &AppHandle) -> Result<()> {
    if std::env::var_os("FLATPAK_ID").is_some() {
        return Err(Error::msg("Update the Flatpak with your software center or flatpak update."));
    }
    if storage::is_portable() {
        return Err(Error::msg("Portable builds update by replacing the executable with the newer portable release."));
    }
    let update = find(app).await?.ok_or_else(|| Error::msg("No update is available"))?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| Error::msg(format!("Update could not be installed: {e}")))?;
    app.restart();
}

#[cfg(test)]
mod tests {
    #[test]
    fn friendly_update_errors() {
        assert!(super::friendly("Update check failed: Could not fetch a valid release JSON from the remote").starts_with("No update information"));
        assert!(super::friendly("error sending request for url").starts_with("Couldn"));
        assert_eq!(super::friendly("other"), "other");
    }
}
