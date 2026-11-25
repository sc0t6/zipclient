use super::install;
use super::version::{self, VersionJson};
use crate::error::{Error, Result};
use crate::net;
use crate::storage;
use crate::tasks::Reporter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const QUILT_META: &str = "https://meta.quiltmc.org/v3";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const FORGE_PROMOTIONS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";
const NEOFORGE_LEGACY_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/forge";

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LoaderVersion {
    pub version: String,
    pub minecraft: String,
    pub stable: bool,
    pub recommended: bool,
}

#[derive(Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoader,
}

#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

pub fn parse_maven_versions(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<version>") {
        rest = &rest[start + 9..];
        if let Some(end) = rest.find("</version>") {
            out.push(rest[..end].trim().to_string());
            rest = &rest[end + 10..];
        } else {
            break;
        }
    }
    out
}

pub fn neoforge_minecraft_version(neo: &str) -> Option<String> {
    let base = neo.split('-').next()?;
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major: u32 = parts[0].parse().ok()?;
    let minor: u32 = parts[1].parse().ok()?;
    if major >= 26 {
        let patch: u32 = parts[2].parse().ok()?;
        return Some(if patch == 0 { format!("{major}.{minor}") } else { format!("{major}.{minor}.{patch}") });
    }
    Some(if minor == 0 { format!("1.{major}") } else { format!("1.{major}.{minor}") })
}

pub fn forge_minecraft_version(forge: &str) -> String {
    forge.split('-').next().unwrap_or_default().to_string()
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<(u64, String)> {
        s.split(|c: char| c == '.' || c == '-' || c == '+')
            .map(|p| {
                let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                (digits.parse().unwrap_or(0), p[digits.len()..].to_string())
            })
            .collect()
    };
    let (pa, pb) = (parse(a), parse(b));
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).cloned().unwrap_or((0, String::new()));
        let y = pb.get(i).cloned().unwrap_or((0, String::new()));
        let ord = x.0.cmp(&y.0).then_with(|| match (x.1.is_empty(), y.1.is_empty()) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => x.1.cmp(&y.1),
        });
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

async fn maven_versions(url: &str, key: &str) -> Result<Vec<String>> {
    let path = storage::cache_dir().join("meta").join(format!("{key}.xml"));
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|age| age.as_secs() < 3600)
        .unwrap_or(false);
    if fresh {
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Ok(parse_maven_versions(&text));
        }
    }
    match net::get_bytes(url).await {
        Ok(bytes) => {
            let _ = storage::write_bytes_atomic(&path, &bytes);
            Ok(parse_maven_versions(&String::from_utf8_lossy(&bytes)))
        }
        Err(e) => std::fs::read_to_string(&path).map(|t| parse_maven_versions(&t)).map_err(|_| e),
    }
}

pub async fn list(loader: &str, minecraft: &str) -> Result<Vec<LoaderVersion>> {
    let mut out = match loader {
        "fabric" | "quilt" => {
            let base = if loader == "fabric" { FABRIC_META } else { QUILT_META };
            let entries: Vec<FabricLoaderEntry> =
                net::get_json_cached(&format!("{base}/versions/loader/{minecraft}"), &format!("{loader}-{minecraft}"), 3600).await?;
            entries
                .into_iter()
                .map(|e| {
                    let stable = e.loader.stable.unwrap_or(!e.loader.version.contains("beta") && !e.loader.version.contains("pre"));
                    LoaderVersion { version: e.loader.version, minecraft: minecraft.into(), stable, recommended: false }
                })
                .collect::<Vec<_>>()
        }
        "forge" => {
            let all = maven_versions(&format!("{FORGE_MAVEN}/maven-metadata.xml"), "forge-maven").await?;
            let promotions: HashMap<String, String> = net::get_json_cached::<serde_json::Value>(FORGE_PROMOTIONS, "forge-promotions", 3600)
                .await
                .ok()
                .and_then(|v| v.get("promos").cloned())
                .and_then(|p| serde_json::from_value(p).ok())
                .unwrap_or_default();
            let recommended = promotions.get(&format!("{minecraft}-recommended")).cloned();
            let latest = promotions.get(&format!("{minecraft}-latest")).cloned();
            let mut list: Vec<LoaderVersion> = all
                .into_iter()
                .filter(|v| forge_minecraft_version(v) == minecraft)
                .map(|full| {
                    let short = full.splitn(2, '-').nth(1).unwrap_or(&full).to_string();
                    let build = short.split('-').next().unwrap_or(&short).to_string();
                    LoaderVersion {
                        recommended: recommended.as_deref() == Some(build.as_str()),
                        stable: recommended.as_deref() == Some(build.as_str()) || latest.as_deref() == Some(build.as_str()) || !full.contains("pre"),
                        version: full,
                        minecraft: minecraft.into(),
                    }
                })
                .collect();
            list.sort_by(|a, b| compare_versions(&b.version, &a.version));
            list
        }
        "neoforge" => {
            if minecraft == "1.20.1" {
                let all = maven_versions(&format!("{NEOFORGE_LEGACY_MAVEN}/maven-metadata.xml"), "neoforge-legacy-maven").await?;
                let mut list: Vec<LoaderVersion> = all
                    .into_iter()
                    .filter(|v| v.starts_with("1.20.1-"))
                    .map(|v| LoaderVersion { stable: !v.contains("beta"), version: v, minecraft: minecraft.into(), recommended: false })
                    .collect();
                list.sort_by(|a, b| compare_versions(&b.version, &a.version));
                list
            } else {
                let all = maven_versions(&format!("{NEOFORGE_MAVEN}/maven-metadata.xml"), "neoforge-maven").await?;
                let mut list: Vec<LoaderVersion> = all
                    .into_iter()
                    .filter(|v| neoforge_minecraft_version(v).as_deref() == Some(minecraft))
                    .map(|v| LoaderVersion { stable: !v.contains("beta") && !v.contains("alpha"), version: v, minecraft: minecraft.into(), recommended: false })
                    .collect();
                list.sort_by(|a, b| compare_versions(&b.version, &a.version));
                list
            }
        }
        "vanilla" => Vec::new(),
        "horizon" => {
            if crate::horizon::minecraft_version().await? == minecraft {
                vec![LoaderVersion { version: "nightly".into(), minecraft: minecraft.into(), stable: true, recommended: true }]
            } else {
                Vec::new()
            }
        }
        other => return Err(Error::msg(format!("Unknown loader: {other}"))),
    };
    if let Some(first_stable) = out.iter().position(|v| v.stable) {
        if !out.iter().any(|v| v.recommended) {
            out[first_stable].recommended = true;
        }
    }
    Ok(out)
}

pub async fn supported_minecraft(loader: &str) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct GameEntry {
        version: String,
    }
    match loader {
        "fabric" | "quilt" => {
            let base = if loader == "fabric" { FABRIC_META } else { QUILT_META };
            let entries: Vec<GameEntry> = net::get_json_cached(&format!("{base}/versions/game"), &format!("{loader}-game"), 3600).await?;
            Ok(entries.into_iter().map(|e| e.version).collect())
        }
        "forge" => {
            let all = maven_versions(&format!("{FORGE_MAVEN}/maven-metadata.xml"), "forge-maven").await?;
            let mut seen = Vec::new();
            for v in all {
                let mc = forge_minecraft_version(&v);
                if !seen.contains(&mc) {
                    seen.push(mc);
                }
            }
            Ok(seen)
        }
        "neoforge" => {
            let all = maven_versions(&format!("{NEOFORGE_MAVEN}/maven-metadata.xml"), "neoforge-maven").await?;
            let mut seen = vec!["1.20.1".to_string()];
            for v in all {
                if let Some(mc) = neoforge_minecraft_version(&v) {
                    if !seen.contains(&mc) {
                        seen.push(mc);
                    }
                }
            }
            Ok(seen)
        }
        "horizon" => Ok(vec![crate::horizon::minecraft_version().await?]),
        _ => Ok(Vec::new()),
    }
}

pub async fn install(loader: &str, minecraft: &str, loader_version: &str, reporter: &Reporter) -> Result<String> {
    install::ensure_vanilla_json(minecraft).await?;
    match loader {
        "vanilla" => Ok(minecraft.to_string()),
        "fabric" | "quilt" => {
            let base = if loader == "fabric" { FABRIC_META } else { QUILT_META };
            reporter.phase(&format!("Installing {}", if loader == "fabric" { "Fabric" } else { "Quilt" }));
            let url = format!("{base}/versions/loader/{minecraft}/{loader_version}/profile/json");
            let bytes = net::get_bytes(&url).await?;
            let mut profile: VersionJson = serde_json::from_slice(&bytes)?;
            if profile.inherits_from.as_deref() != Some(minecraft) {
                profile.inherits_from = Some(minecraft.to_string());
            }
            let id = profile.id.clone();
            storage::write_json(&version::version_json_path(&storage::versions_dir(), &id), &profile)?;
            Ok(id)
        }
        "forge" | "neoforge" => super::forge::install(loader, minecraft, loader_version, reporter).await,
        other => Err(Error::msg(format!("Unknown loader: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_mapping() {
        assert_eq!(neoforge_minecraft_version("21.1.265").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_minecraft_version("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_minecraft_version("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_minecraft_version("20.2.3-beta").as_deref(), Some("1.20.2"));
        assert_eq!(neoforge_minecraft_version("26.3.0.99").as_deref(), Some("26.3"));
        assert_eq!(neoforge_minecraft_version("26.1.2.125").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_minecraft_version("bad"), None);
    }

    #[test]
    fn forge_mapping_and_maven_parse() {
        assert_eq!(forge_minecraft_version("1.20.1-47.2.0"), "1.20.1");
        assert_eq!(forge_minecraft_version("1.7.10-10.13.4.1614-1.7.10"), "1.7.10");
        let xml = "<metadata><versioning><versions><version>1.0</version><version> 2.0 </version></versions></versioning></metadata>";
        assert_eq!(parse_maven_versions(xml), vec!["1.0", "2.0"]);
    }

    #[test]
    fn version_ordering() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("1.20.1-47.2.10", "1.20.1-47.2.9"), Greater);
        assert_eq!(compare_versions("21.1.100", "21.1.99"), Greater);
        assert_eq!(compare_versions("20.2.3-beta", "20.2.3"), Less);
        assert_eq!(compare_versions("1.0", "1.0"), Equal);
    }
}
