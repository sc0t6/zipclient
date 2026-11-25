use crate::auth;
use crate::error::{Error, Result};
use crate::net;
use crate::storage;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub const STEVE: &str = "https://textures.minecraft.net/texture/31f477eb1a7beee631c2ca64d06f8f68fa93a3386d04452ab27f43acdf1b60cb";
pub const ALEX: &str = "https://textures.minecraft.net/texture/46acd06e8483b176e8ea39fc12fe105eb3a2a4970f5100057e9d84d4b60bdfa7";

#[derive(Serialize, Deserialize, Clone)]
struct SkinRecord {
    id: String,
    name: String,
    variant: String,
    added: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct SkinIndex {
    #[serde(default)]
    skins: Vec<SkinRecord>,
    #[serde(default)]
    selected: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinEntry {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub data_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinLibrary {
    pub skins: Vec<SkinEntry>,
    pub selected: HashMap<String, String>,
}

fn dir() -> PathBuf {
    storage::root().join("skins")
}

fn load() -> SkinIndex {
    storage::read_json(&dir().join("index.json")).ok().flatten().unwrap_or_default()
}

fn save(index: &SkinIndex) -> Result<()> {
    storage::write_json(&dir().join("index.json"), index)
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn validate_skin(bytes: &[u8]) -> Result<()> {
    match png_size(bytes) {
        Some((64, 64)) | Some((64, 32)) => Ok(()),
        Some((w, h)) => Err(Error::msg(format!("Skins must be 64x64 or 64x32 pixels (this image is {w}x{h})"))),
        None => Err(Error::msg("Skins must be PNG images")),
    }
}

pub fn library() -> SkinLibrary {
    let index = load();
    let skins = index
        .skins
        .iter()
        .filter_map(|s| {
            let bytes = std::fs::read(dir().join(format!("{}.png", s.id))).ok()?;
            Some(SkinEntry { id: s.id.clone(), name: s.name.clone(), variant: s.variant.clone(), data_url: data_url(&bytes) })
        })
        .collect();
    SkinLibrary { skins, selected: index.selected }
}

pub fn import(path: &str, name: &str, variant: &str) -> Result<SkinEntry> {
    let bytes = std::fs::read(path)?;
    if bytes.len() > 512 * 1024 {
        return Err(Error::msg("Skin file is too large"));
    }
    validate_skin(&bytes)?;
    let id = net::sha1_bytes(&bytes)[..16].to_string();
    let variant = if variant == "slim" { "slim" } else { "classic" }.to_string();
    let name = {
        let n = name.trim();
        if n.is_empty() {
            std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Skin".into())
        } else {
            n.chars().take(32).collect()
        }
    };
    storage::write_bytes_atomic(&dir().join(format!("{id}.png")), &bytes)?;
    let mut index = load();
    index.skins.retain(|s| s.id != id);
    index.skins.push(SkinRecord { id: id.clone(), name: name.clone(), variant: variant.clone(), added: crate::instances::now() });
    save(&index)?;
    Ok(SkinEntry { id, name, variant, data_url: data_url(&bytes) })
}

pub fn remove(id: &str) -> Result<()> {
    let mut index = load();
    index.skins.retain(|s| s.id != id);
    index.selected.retain(|_, v| v != id);
    save(&index)?;
    let _ = std::fs::remove_file(dir().join(format!("{id}.png")));
    Ok(())
}

pub fn set_variant(id: &str, variant: &str) -> Result<()> {
    let mut index = load();
    let skin = index.skins.iter_mut().find(|s| s.id == id).ok_or_else(|| Error::msg("Skin not found"))?;
    skin.variant = if variant == "slim" { "slim".into() } else { "classic".into() };
    save(&index)
}

pub fn select_local(account_id: &str, skin_id: Option<&str>) -> Result<()> {
    let mut index = load();
    match skin_id {
        Some(id) => {
            index.selected.insert(account_id.to_string(), id.to_string());
        }
        None => {
            index.selected.remove(account_id);
        }
    }
    save(&index)
}

pub async fn texture(url: &str) -> Result<String> {
    if !url.starts_with("https://textures.minecraft.net/texture/") && !url.starts_with("http://textures.minecraft.net/texture/") {
        return Err(Error::msg("Untrusted skin texture host"));
    }
    let url = url.replace("http://", "https://");
    let cache = storage::cache_dir().join("skins").join(format!("{}.png", net::sha1_bytes(url.as_bytes())));
    if let Ok(bytes) = std::fs::read(&cache) {
        return Ok(data_url(&bytes));
    }
    let bytes = net::get_bytes(&url).await?;
    if png_size(&bytes).is_none() {
        return Err(Error::msg("Skin server returned an invalid image"));
    }
    let _ = storage::write_bytes_atomic(&cache, &bytes);
    Ok(data_url(&bytes))
}

pub async fn upload(account_id: &str, skin_id: &str) -> Result<()> {
    let index = load();
    let record = index.skins.iter().find(|s| s.id == skin_id).cloned().ok_or_else(|| Error::msg("Skin not found"))?;
    let bytes = std::fs::read(dir().join(format!("{skin_id}.png")))?;
    validate_skin(&bytes)?;
    let token = auth::access_token(account_id).await?;
    let part = reqwest::multipart::Part::bytes(bytes).file_name("skin.png").mime_str("image/png").map_err(|e| Error::msg(e.to_string()))?;
    let form = reqwest::multipart::Form::new().text("variant", record.variant.clone()).part("file", part);
    let response = net::client()
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        let detail = body["errorMessage"].as_str().or(body["error"].as_str()).unwrap_or("");
        return Err(Error::msg(format!("Minecraft rejected the skin upload (HTTP {status}) {detail}")));
    }
    auth::refresh_profile(account_id).await?;
    select_local(account_id, Some(skin_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_dimension_check() {
        let mut header = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        header.extend_from_slice(&64u32.to_be_bytes());
        header.extend_from_slice(&64u32.to_be_bytes());
        assert_eq!(png_size(&header), Some((64, 64)));
        assert!(validate_skin(&header).is_ok());
        let mut wide = header.clone();
        wide[20..24].copy_from_slice(&128u32.to_be_bytes());
        assert!(validate_skin(&wide).is_err());
        assert!(validate_skin(b"GIF89a").is_err());
    }
}
