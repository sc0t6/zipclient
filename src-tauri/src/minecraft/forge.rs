use super::install;
use super::java;
use super::version::{self, Features, Library, VersionJson};
use crate::error::{Error, Result};
use crate::net::{self, DownloadItem};
use crate::storage;
use crate::tasks::Reporter;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct InstallProfile {
    #[serde(default)]
    json: Option<String>,
    #[serde(default)]
    libraries: Vec<Library>,
    #[serde(default)]
    data: HashMap<String, SidedValue>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default, rename = "versionInfo")]
    version_info: Option<VersionJson>,
    #[serde(default)]
    install: Option<LegacyInstall>,
}

#[derive(Deserialize)]
struct LegacyInstall {
    path: String,
    #[serde(rename = "filePath")]
    file_path: String,
}

#[derive(Deserialize)]
struct SidedValue {
    client: String,
}

#[derive(Deserialize)]
struct Processor {
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
    #[serde(default)]
    sides: Option<Vec<String>>,
}

fn installer_url(loader: &str, v: &str) -> String {
    match loader {
        "neoforge" if v.starts_with("1.20.1-") => {
            format!("https://maven.neoforged.net/releases/net/neoforged/forge/{v}/forge-{v}-installer.jar")
        }
        "neoforge" => format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar"),
        _ => format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{v}/forge-{v}-installer.jar"),
    }
}

fn registry_path() -> PathBuf {
    storage::game_root().join("loader-installs.json")
}

fn read_registry() -> HashMap<String, String> {
    storage::read_json(&registry_path()).ok().flatten().unwrap_or_default()
}

fn already_installed(key: &str) -> Option<String> {
    let id = read_registry().get(key)?.clone();
    let merged = version::load_merged(&storage::versions_dir(), &id).ok()?;
    let libs = version::resolve_libraries(&merged, &storage::libraries_dir(), &Features::default()).ok()?;
    libs.iter().all(|l| l.path.is_file()).then_some(id)
}

fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name.trim_start_matches('/')).ok()?;
    let mut buffer = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buffer).ok()?;
    Some(buffer)
}

fn fix_legacy_url(url: &str) -> String {
    url.replace("http://files.minecraftforge.net/maven", "https://maven.minecraftforge.net")
        .replace("https://files.minecraftforge.net/maven", "https://maven.minecraftforge.net")
        .replace("http://", "https://")
}

pub async fn install(loader: &str, minecraft: &str, loader_version: &str, reporter: &Reporter) -> Result<String> {
    let key = format!("{loader}:{loader_version}");
    if let Some(id) = already_installed(&key) {
        return Ok(id);
    }
    let label = if loader == "forge" { "Forge" } else { "NeoForge" };
    reporter.phase(&format!("Downloading {label} installer"));
    let installer_path = storage::game_root().join("installers").join(format!("{loader}-{loader_version}-installer.jar"));
    let url = installer_url(loader, loader_version);
    net::download_all(vec![DownloadItem::new(url.clone(), installer_path.clone(), None, None)], reporter.progress(), false)
        .await
        .map_err(|e| Error::msg(format!("{label} {loader_version} has no installer available ({e})")))?;
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&installer_path)?)
        .map_err(|_| {
            let _ = std::fs::remove_file(&installer_path);
            Error::msg(format!("The {label} installer is corrupted; it will be downloaded again next time"))
        })?;
    let profile_bytes = read_entry(&mut archive, "install_profile.json").ok_or_else(|| Error::msg("Installer has no install_profile.json"))?;
    let profile: InstallProfile = serde_json::from_slice(&profile_bytes)?;
    let versions = storage::versions_dir();
    let libraries = storage::libraries_dir();

    let id = if let Some(mut info) = profile.version_info {
        reporter.phase(&format!("Installing {label}"));
        if info.inherits_from.is_none() {
            info.inherits_from = Some(minecraft.to_string());
        }
        if info.jar.is_none() {
            info.jar = Some(minecraft.to_string());
        }
        for lib in info.libraries.iter_mut() {
            if let Some(url) = &lib.url {
                lib.url = Some(fix_legacy_url(url));
            }
            lib.checksums = None;
        }
        if let Some(legacy) = &profile.install {
            let target = libraries.join(version::maven_path(&legacy.path)?);
            let bytes = read_entry(&mut archive, &legacy.file_path).ok_or_else(|| Error::msg("Installer is missing the Forge universal jar"))?;
            storage::write_bytes_atomic(&target, &bytes)?;
            for lib in info.libraries.iter_mut() {
                if lib.name == legacy.path {
                    lib.url = None;
                    lib.downloads = Some(version::LibraryDownloads {
                        artifact: Some(version::Artifact {
                            path: Some(version::maven_path(&legacy.path)?),
                            url: String::new(),
                            sha1: None,
                            size: None,
                        }),
                        classifiers: None,
                    });
                }
            }
        }
        let id = info.id.clone();
        storage::write_json(&version::version_json_path(&versions, &id), &info)?;
        id
    } else {
        let json_name = profile.json.clone().unwrap_or_else(|| "/version.json".into());
        let version_bytes = read_entry(&mut archive, &json_name).ok_or_else(|| Error::msg("Installer has no version.json"))?;
        let mut version_json: VersionJson = serde_json::from_slice(&version_bytes)?;
        if version_json.inherits_from.is_none() {
            version_json.inherits_from = Some(minecraft.to_string());
        }
        let id = version_json.id.clone();

        reporter.phase("Preparing Minecraft");
        let vanilla = install::install_version(minecraft, reporter, false, &Features::default()).await?;

        reporter.phase(&format!("Downloading {label} libraries"));
        let mut items = Vec::new();
        for lib in profile.libraries.iter().chain(version_json.libraries.iter()) {
            let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.clone());
            let path = artifact.as_ref().and_then(|a| a.path.clone()).map(Ok).unwrap_or_else(|| version::maven_path(&lib.name))?;
            let target = libraries.join(&path);
            let url = artifact.as_ref().map(|a| a.url.clone()).unwrap_or_default();
            if url.is_empty() {
                if let Some(bytes) = read_entry(&mut archive, &format!("maven/{path}")) {
                    let matches = artifact
                        .as_ref()
                        .and_then(|a| a.sha1.clone())
                        .map(|sha| net::sha1_bytes(&bytes) == sha)
                        .unwrap_or(true);
                    if !matches {
                        return Err(Error::msg(format!("Installer copy of {} failed verification", lib.name)));
                    }
                    storage::write_bytes_atomic(&target, &bytes)?;
                }
                continue;
            }
            items.push(DownloadItem::new(url, target, artifact.as_ref().and_then(|a| a.sha1.clone()), artifact.as_ref().and_then(|a| a.size)));
        }
        net::download_all(items, reporter.progress(), false).await?;

        reporter.phase(&format!("Running {label} installer"));
        let temp = storage::game_root().join("installers").join(format!("tmp-{loader}-{loader_version}"));
        if temp.exists() {
            let _ = std::fs::remove_dir_all(&temp);
        }
        std::fs::create_dir_all(&temp)?;
        let mut data: HashMap<String, String> = HashMap::new();
        for (key, value) in &profile.data {
            let resolved = resolve_data(&value.client, &mut archive, &temp, &libraries)?;
            data.insert(key.clone(), resolved);
        }
        data.insert("SIDE".into(), "client".into());
        data.insert("MINECRAFT_JAR".into(), install::client_jar_path(minecraft).to_string_lossy().to_string());
        data.insert("MINECRAFT_VERSION".into(), minecraft.to_string());
        data.insert("ROOT".into(), storage::game_root().to_string_lossy().to_string());
        data.insert("INSTALLER".into(), installer_path.to_string_lossy().to_string());
        data.insert("LIBRARY_DIR".into(), libraries.to_string_lossy().to_string());

        let java_path = java::select(vanilla.java_version.as_ref(), None, reporter.progress()).await?;
        let processors: Vec<&Processor> = profile
            .processors
            .iter()
            .filter(|p| p.sides.as_ref().map(|s| s.iter().any(|x| x == "client")).unwrap_or(true))
            .collect();
        let total = processors.len();
        for (index, processor) in processors.into_iter().enumerate() {
            reporter.phase(&format!("Running {label} installer ({}/{total})", index + 1));
            run_processor(processor, &data, &libraries, &java_path).await?;
        }
        let _ = std::fs::remove_dir_all(&temp);
        storage::write_json(&version::version_json_path(&versions, &id), &version_json)?;
        id
    };

    let merged = version::load_merged(&versions, &id)?;
    reporter.phase(&format!("Downloading {label} libraries"));
    let mut items = Vec::new();
    for lib in version::resolve_libraries(&merged, &libraries, &Features::default())? {
        if lib.url.is_empty() || lib.path.is_file() {
            continue;
        }
        let relative = lib.path.strip_prefix(&libraries).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        let mut item = DownloadItem::new(lib.url.clone(), lib.path.clone(), lib.sha1.clone(), lib.size);
        item.fallback_urls = version::fallback_urls(&relative, &lib.url);
        items.push(item);
    }
    net::download_all(items, reporter.progress(), false).await?;
    let mut registry = read_registry();
    registry.insert(key, id.clone());
    storage::write_json(&registry_path(), &registry)?;
    Ok(id)
}

fn resolve_data(value: &str, archive: &mut zip::ZipArchive<std::fs::File>, temp: &Path, libraries: &Path) -> Result<String> {
    if value.starts_with('[') && value.ends_with(']') {
        return Ok(libraries.join(version::maven_path(&value[1..value.len() - 1])?).to_string_lossy().to_string());
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return Ok(value[1..value.len() - 1].to_string());
    }
    if value.starts_with('/') {
        let target = storage::safe_join(temp, value.trim_start_matches('/'))?;
        let bytes = read_entry(archive, value).ok_or_else(|| Error::msg(format!("Installer is missing {value}")))?;
        storage::write_bytes_atomic(&target, &bytes)?;
        return Ok(target.to_string_lossy().to_string());
    }
    Ok(value.to_string())
}

pub fn substitute(arg: &str, data: &HashMap<String, String>, libraries: &Path) -> Result<String> {
    if arg.starts_with('[') && arg.ends_with(']') {
        return Ok(libraries.join(version::maven_path(&arg[1..arg.len() - 1])?).to_string_lossy().to_string());
    }
    let mut out = String::new();
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match data.get(key) {
                    Some(v) => out.push_str(v),
                    None => return Err(Error::msg(format!("Installer references unknown value {{{key}}}"))),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn main_class(jar: &Path) -> Result<String> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(jar).map_err(|_| Error::msg(format!("Processor jar missing: {}", jar.display())))?)?;
    let mut entry = archive.by_name("META-INF/MANIFEST.MF")?;
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    text.lines()
        .find_map(|l| l.strip_prefix("Main-Class:").map(|m| m.trim().to_string()))
        .ok_or_else(|| Error::msg(format!("Processor {} has no Main-Class", jar.display())))
}

fn outputs_valid(processor: &Processor, data: &HashMap<String, String>, libraries: &Path) -> bool {
    if processor.outputs.is_empty() {
        return false;
    }
    processor.outputs.iter().all(|(file, sha)| {
        let (Ok(file), Ok(sha)) = (substitute(file, data, libraries), substitute(sha, data, libraries)) else { return false };
        net::sha1_file(Path::new(&file)).map(|h| h.eq_ignore_ascii_case(sha.trim_matches('\''))).unwrap_or(false)
    })
}

async fn run_processor(processor: &Processor, data: &HashMap<String, String>, libraries: &Path, java: &Path) -> Result<()> {
    if outputs_valid(processor, data, libraries) {
        return Ok(());
    }
    let jar = libraries.join(version::maven_path(&processor.jar)?);
    let class = main_class(&jar)?;
    let mut classpath = vec![jar.to_string_lossy().to_string()];
    for entry in &processor.classpath {
        classpath.push(libraries.join(version::maven_path(entry)?).to_string_lossy().to_string());
    }
    let args: Vec<String> = processor.args.iter().map(|a| substitute(a, data, libraries)).collect::<Result<_>>()?;
    let mut command = tokio::process::Command::new(java);
    command.arg("-cp").arg(classpath.join(crate::platform::CLASSPATH_SEPARATOR)).arg(&class).args(&args);
    command.current_dir(storage::game_root());
    crate::platform::hide_console(&mut command);
    let output = command.output().await.map_err(|e| Error::msg(format!("Could not start Java for installer: {e}")))?;
    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stderr).to_string() + &String::from_utf8_lossy(&output.stdout);
        let tail: Vec<&str> = text.lines().rev().take(12).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(Error::msg(format!("Installer step {class} failed:\n{}", tail.join("\n"))));
    }
    for (file, sha) in &processor.outputs {
        let file = substitute(file, data, libraries)?;
        let sha = substitute(sha, data, libraries)?;
        let actual = net::sha1_file(Path::new(&file)).map_err(|_| Error::msg(format!("Installer did not produce {file}")))?;
        if !actual.eq_ignore_ascii_case(sha.trim_matches('\'')) {
            return Err(Error::msg(format!("Installer output {file} failed verification")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_processor_arguments() {
        let mut data = HashMap::new();
        data.insert("MINECRAFT_JAR".to_string(), "C:/mc/1.20.1.jar".to_string());
        data.insert("SIDE".to_string(), "client".to_string());
        let libs = Path::new("L");
        assert_eq!(substitute("{MINECRAFT_JAR}", &data, libs).unwrap(), "C:/mc/1.20.1.jar");
        assert_eq!(substitute("--side={SIDE}", &data, libs).unwrap(), "--side=client");
        assert_eq!(substitute("--plain", &data, libs).unwrap(), "--plain");
        assert!(substitute("{MISSING}", &data, libs).is_err());
        let p = substitute("[net.minecraft:client:1.20.1:slim]", &data, libs).unwrap();
        assert!(p.replace('\\', "/").ends_with("net/minecraft/client/1.20.1/client-1.20.1-slim.jar"));
    }

    #[test]
    fn installer_urls() {
        assert!(installer_url("forge", "1.20.1-47.2.0").ends_with("forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar"));
        assert!(installer_url("neoforge", "21.1.77").ends_with("neoforge/21.1.77/neoforge-21.1.77-installer.jar"));
        assert!(installer_url("neoforge", "1.20.1-47.1.106").contains("/net/neoforged/forge/"));
    }
}
