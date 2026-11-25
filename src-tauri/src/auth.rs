use crate::error::{Error, Result};
use crate::net;
use crate::storage;
use base64::Engine;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";
#[cfg(windows)]
const ENTROPY: &[u8] = b"zip-launcher-account-secret";

static LOCK: Mutex<()> = Mutex::new(());
static FLOW_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub skin_url: Option<String>,
    #[serde(default)]
    pub skin_variant: Option<String>,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub needs_relogin: bool,
    #[serde(default)]
    pub added: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct AccountsFile {
    #[serde(default)]
    accounts: Vec<Account>,
    #[serde(default)]
    active: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct Secret {
    ms_refresh: String,
    mc_token: String,
    mc_expires: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub skin_url: Option<String>,
    pub skin_variant: Option<String>,
    pub active: bool,
    pub needs_relogin: bool,
}

pub struct LaunchIdentity {
    pub name: String,
    pub uuid: String,
    pub token: String,
    pub user_type: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn path() -> std::path::PathBuf {
    storage::root().join("accounts.json")
}

fn load() -> AccountsFile {
    storage::read_json(&path()).ok().flatten().unwrap_or_default()
}

fn save(file: &AccountsFile) -> Result<()> {
    storage::write_json(&path(), file)?;
    crate::platform::restrict_to_user(&path())
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let entropy = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output);
        if ok == 0 {
            return Err(Error::msg("Windows refused to encrypt the account credentials"));
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Ok(bytes)
    }
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let entropy = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output);
        if ok == 0 {
            return Err(Error::msg("Stored credentials could not be decrypted on this Windows account. Sign in again."));
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Ok(bytes)
    }
}

#[cfg(not(windows))]
fn protect(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}

#[cfg(not(windows))]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}

fn seal(secret: &Secret) -> Result<String> {
    let plain = serde_json::to_vec(secret)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(protect(&plain)?))
}

fn open(sealed: &str) -> Result<Secret> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(sealed)
        .map_err(|_| Error::msg("Stored credentials are damaged. Sign in again."))?;
    Ok(serde_json::from_slice(&unprotect(&raw)?)?)
}

pub fn offline_uuid(name: &str) -> String {
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    hash[6] = (hash[6] & 0x0f) | 0x30;
    hash[8] = (hash[8] & 0x3f) | 0x80;
    hex::encode(hash)
}

pub fn valid_offline_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn client_id() -> Option<String> {
    std::env::var("ZIP_MS_CLIENT_ID").ok().or(option_env!("ZIP_MS_CLIENT_ID").map(str::to_string)).filter(|s| !s.trim().is_empty())
}

pub fn list() -> Vec<PublicAccount> {
    let file = load();
    file.accounts
        .iter()
        .map(|a| PublicAccount {
            id: a.id.clone(),
            name: a.name.clone(),
            kind: a.kind.clone(),
            skin_url: a.skin_url.clone(),
            skin_variant: a.skin_variant.clone(),
            active: a.id == file.active,
            needs_relogin: a.needs_relogin,
        })
        .collect()
}

pub fn get(id: &str) -> Option<Account> {
    load().accounts.into_iter().find(|a| a.id == id)
}

pub fn active_id() -> Option<String> {
    let file = load();
    if file.accounts.iter().any(|a| a.id == file.active) {
        Some(file.active)
    } else {
        file.accounts.first().map(|a| a.id.clone())
    }
}

fn upsert(account: Account, make_active: bool) -> Result<()> {
    let _guard = LOCK.lock().map_err(|_| Error::msg("Account store busy"))?;
    let mut file = load();
    let id = account.id.clone();
    if let Some(existing) = file.accounts.iter_mut().find(|a| a.id == account.id) {
        let added = existing.added;
        *existing = Account { added, ..account };
    } else {
        file.accounts.push(account);
    }
    if make_active || file.active.is_empty() {
        file.active = id;
    }
    save(&file)
}

pub fn add_offline(name: &str) -> Result<PublicAccount> {
    let name = name.trim();
    if !valid_offline_name(name) {
        return Err(Error::msg("Offline names must be 3-16 characters using letters, numbers or underscores"));
    }
    let id = offline_uuid(name);
    upsert(
        Account { id: id.clone(), name: name.to_string(), kind: "offline".into(), added: now(), ..Default::default() },
        true,
    )?;
    list().into_iter().find(|a| a.id == id).ok_or_else(|| Error::msg("Account was not saved"))
}

pub fn set_active(id: &str) -> Result<()> {
    let _guard = LOCK.lock().map_err(|_| Error::msg("Account store busy"))?;
    let mut file = load();
    if !file.accounts.iter().any(|a| a.id == id) {
        return Err(Error::msg("Account not found"));
    }
    file.active = id.to_string();
    save(&file)
}

pub fn remove(id: &str) -> Result<()> {
    let _guard = LOCK.lock().map_err(|_| Error::msg("Account store busy"))?;
    let mut file = load();
    file.accounts.retain(|a| a.id != id);
    if file.active == id {
        file.active = file.accounts.first().map(|a| a.id.clone()).unwrap_or_default();
    }
    save(&file)
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    #[serde(default)]
    pub message: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

#[derive(Serialize, Clone)]
pub struct AuthEvent {
    pub status: String,
    pub message: String,
}

fn emit(app: &AppHandle, status: &str, message: impl Into<String>) {
    let _ = app.emit("auth-event", AuthEvent { status: status.into(), message: message.into() });
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
    #[serde(default)]
    error_description: String,
}

pub async fn begin_device_flow(app: AppHandle) -> Result<DeviceCodeInfo> {
    let client_id = client_id().ok_or_else(|| {
        Error::msg("Microsoft sign-in is not available in this build of Zip.")
    })?;
    let response = net::client()
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", client_id.as_str()), ("scope", SCOPE)])
        .send()
        .await?;
    if !response.status().is_success() {
        let body: TokenError = response.json().await.unwrap_or(TokenError { error: "unknown".into(), error_description: String::new() });
        return Err(Error::msg(format!("Microsoft rejected the sign-in request: {} {}", body.error, body.error_description.lines().next().unwrap_or(""))));
    }
    let code: DeviceCode = response.json().await?;
    let generation = FLOW_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let info = DeviceCodeInfo { user_code: code.user_code.clone(), verification_uri: code.verification_uri.clone(), expires_in: code.expires_in };
    tauri::async_runtime::spawn(async move {
        match poll_device_code(&app, &client_id, &code, generation).await {
            Ok(Some(account)) => emit(&app, "success", account.name),
            Ok(None) => {}
            Err(e) => emit(&app, "error", e.0),
        }
    });
    Ok(info)
}

pub fn cancel_device_flow() {
    FLOW_GENERATION.fetch_add(1, Ordering::SeqCst);
}

async fn poll_device_code(app: &AppHandle, client_id: &str, code: &DeviceCode, generation: u64) -> Result<Option<PublicAccount>> {
    let deadline = now() + code.expires_in;
    let mut interval = code.interval.max(1);
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if FLOW_GENERATION.load(Ordering::SeqCst) != generation {
            return Ok(None);
        }
        if now() > deadline {
            return Err(Error::msg("The login code expired. Start the sign-in again."));
        }
        let response = net::client()
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", code.device_code.as_str()),
            ])
            .send()
            .await?;
        if response.status().is_success() {
            let token: TokenResponse = response.json().await?;
            emit(app, "progress", "Signing in to Xbox Live");
            let account = complete_login(&token.access_token, token.refresh_token.unwrap_or_default(), Some(app)).await?;
            return Ok(Some(account));
        }
        let error: TokenError = response.json().await.unwrap_or(TokenError { error: "unknown".into(), error_description: String::new() });
        match error.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "authorization_declined" => return Err(Error::msg("Sign-in was declined.")),
            "expired_token" => return Err(Error::msg("The login code expired. Start the sign-in again.")),
            "bad_verification_code" => return Err(Error::msg("Microsoft did not recognise the login code.")),
            other => return Err(Error::msg(format!("Microsoft sign-in failed: {other}"))),
        }
    }
}

async fn xbox_chain(ms_access: &str) -> Result<(String, u64)> {
    let user: serde_json::Value = net::client()
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}")},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()
        .map_err(|_| Error::msg("Xbox Live rejected the Microsoft account."))?
        .json()
        .await?;
    let user_token = user["Token"].as_str().ok_or_else(|| Error::msg("Xbox Live returned no token"))?;
    let xsts_response = net::client()
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [user_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    let status = xsts_response.status();
    let xsts: serde_json::Value = xsts_response.json().await.unwrap_or_default();
    if !status.is_success() {
        let reason = match xsts["XErr"].as_u64() {
            Some(2148916233) => "This Microsoft account has no Xbox profile. Create one at xbox.com and try again.",
            Some(2148916235) => "Xbox Live is not available in your country or region.",
            Some(2148916236) | Some(2148916237) => "This account needs adult verification on xbox.com before it can play.",
            Some(2148916238) => "This is a child account. An adult must add it to a Microsoft family first.",
            _ => "Xbox Live security token request failed.",
        };
        return Err(Error::msg(reason));
    }
    let xsts_token = xsts["Token"].as_str().ok_or_else(|| Error::msg("XSTS returned no token"))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or_else(|| Error::msg("XSTS returned no user hash"))?;
    let mc: serde_json::Value = net::client()
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken": format!("XBL3.0 x={uhs};{xsts_token}")}))
        .send()
        .await?
        .error_for_status()
        .map_err(|e| Error::msg(format!("Minecraft services rejected the login ({}).", e.status().map(|s| s.as_u16()).unwrap_or(0))))?
        .json()
        .await?;
    let token = mc["access_token"].as_str().ok_or_else(|| Error::msg("Minecraft services returned no token"))?.to_string();
    let expires = now() + mc["expires_in"].as_u64().unwrap_or(86400);
    Ok((token, expires))
}

#[derive(Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<ProfileSkin>,
}

#[derive(Deserialize)]
pub struct ProfileSkin {
    pub state: String,
    pub url: String,
    #[serde(default)]
    pub variant: String,
}

pub async fn fetch_profile(mc_token: &str) -> Result<Option<Profile>> {
    let response = net::client().get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(mc_token).send().await?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    Ok(Some(response.error_for_status()?.json().await?))
}

async fn owns_game(mc_token: &str) -> Result<bool> {
    let response: serde_json::Value = net::client()
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .bearer_auth(mc_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(response["items"]
        .as_array()
        .map(|items| items.iter().any(|i| matches!(i["name"].as_str(), Some("game_minecraft") | Some("product_minecraft"))))
        .unwrap_or(false))
}

fn apply_profile(account: &mut Account, profile: &Profile) {
    account.name = profile.name.clone();
    let active = profile.skins.iter().find(|s| s.state == "ACTIVE");
    account.skin_url = active.map(|s| s.url.replace("http://", "https://"));
    account.skin_variant = active.map(|s| s.variant.to_ascii_lowercase());
}

async fn complete_login(ms_access: &str, ms_refresh: String, app: Option<&AppHandle>) -> Result<PublicAccount> {
    let (mc_token, mc_expires) = xbox_chain(ms_access).await?;
    if let Some(app) = app {
        emit(app, "progress", "Checking game ownership");
    }
    let profile = fetch_profile(&mc_token).await?;
    let Some(profile) = profile else {
        if owns_game(&mc_token).await.unwrap_or(false) {
            return Err(Error::msg("You own Minecraft but have no profile yet. Pick a name at minecraft.net first."));
        }
        return Err(Error::msg("This Microsoft account does not own Minecraft: Java Edition."));
    };
    let mut account = Account { id: profile.id.clone(), kind: "microsoft".into(), added: now(), ..Default::default() };
    apply_profile(&mut account, &profile);
    account.secret = seal(&Secret { ms_refresh, mc_token, mc_expires })?;
    upsert(account, true)?;
    list().into_iter().find(|a| a.id == profile.id).ok_or_else(|| Error::msg("Account was not saved"))
}

async fn refresh_account(account: &Account) -> Result<Secret> {
    let mut secret = open(&account.secret)?;
    let client_id = client_id().ok_or_else(|| Error::msg("Microsoft sign-in is not configured."))?;
    let response = net::client()
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("refresh_token", secret.ms_refresh.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        let error: TokenError = response.json().await.unwrap_or(TokenError { error: "unknown".into(), error_description: String::new() });
        if error.error == "invalid_grant" || error.error == "interaction_required" {
            let mut stale = account.clone();
            stale.needs_relogin = true;
            let _ = upsert(stale, false);
            return Err(Error::msg(format!("The session for {} expired. Sign in again.", account.name)));
        }
        return Err(Error::msg(format!("Could not refresh the Microsoft session: {}", error.error)));
    }
    let token: TokenResponse = response.json().await?;
    if let Some(refresh) = token.refresh_token {
        secret.ms_refresh = refresh;
    }
    let (mc_token, mc_expires) = xbox_chain(&token.access_token).await?;
    secret.mc_token = mc_token;
    secret.mc_expires = mc_expires;
    let mut updated = account.clone();
    if let Ok(Some(profile)) = fetch_profile(&secret.mc_token).await {
        apply_profile(&mut updated, &profile);
    }
    updated.needs_relogin = false;
    updated.secret = seal(&secret)?;
    upsert(updated, false)?;
    Ok(secret)
}

pub async fn access_token(account_id: &str) -> Result<String> {
    let account = get(account_id).ok_or_else(|| Error::msg("Account not found"))?;
    if account.kind != "microsoft" {
        return Err(Error::msg("Offline accounts cannot use Minecraft services"));
    }
    let secret = open(&account.secret)?;
    if secret.mc_expires > now() + 300 {
        return Ok(secret.mc_token);
    }
    Ok(refresh_account(&account).await?.mc_token)
}

pub async fn refresh_profile(account_id: &str) -> Result<()> {
    let token = access_token(account_id).await?;
    let mut account = get(account_id).ok_or_else(|| Error::msg("Account not found"))?;
    if let Some(profile) = fetch_profile(&token).await? {
        apply_profile(&mut account, &profile);
        upsert(account, false)?;
    }
    Ok(())
}

pub async fn launch_identity(account_id: Option<&str>) -> Result<LaunchIdentity> {
    let id = account_id
        .map(str::to_string)
        .filter(|s| !s.is_empty() && get(s).is_some())
        .or_else(active_id)
        .ok_or_else(|| Error::msg("Add an account before playing."))?;
    let account = get(&id).ok_or_else(|| Error::msg("Account not found"))?;
    if account.kind == "offline" {
        return Ok(LaunchIdentity { name: account.name, uuid: account.id, token: "0".into(), user_type: "legacy".into() });
    }
    let token = access_token(&id).await?;
    let account = get(&id).unwrap_or(account);
    Ok(LaunchIdentity { name: account.name, uuid: account.id, token, user_type: "msa".into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java_name_uuid() {
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
        assert_eq!(offline_uuid("jeb_"), "a762f5604fce3236812ab80efff0b62b");
    }

    #[test]
    fn offline_name_validation() {
        assert!(valid_offline_name("Steve"));
        assert!(valid_offline_name("a_b_c"));
        assert!(!valid_offline_name("ab"));
        assert!(!valid_offline_name("this_name_is_way_too_long"));
        assert!(!valid_offline_name("bad name"));
        assert!(!valid_offline_name("ünï"));
    }

    #[test]
    fn dpapi_roundtrip() {
        let secret = Secret { ms_refresh: "r".into(), mc_token: "t".into(), mc_expires: 42 };
        let sealed = seal(&secret).unwrap();
        assert!(!sealed.contains("\"t\""));
        let opened = open(&sealed).unwrap();
        assert_eq!(opened.mc_token, "t");
        assert_eq!(opened.mc_expires, 42);
        assert!(open("not-base64!").is_err());
    }
}
