use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<HashMap<String, Artifact>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersionRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<HashMap<String, LoggingEntry>>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_time: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<ArgValue>,
    #[serde(default)]
    pub jvm: Vec<ArgValue>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ArgValue {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: StringOrList },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

impl StringOrList {
    pub fn items(&self) -> Vec<String> {
        match self {
            StringOrList::One(s) => vec![s.clone()],
            StringOrList::Many(v) => v.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Rule {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct OsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Artifact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LibraryDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Library {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<Extract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clientreq: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksums: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub total_size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionRef {
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub major_version: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LoggingEntry {
    #[serde(default)]
    pub argument: String,
    pub file: LoggingFile,
    #[serde(default, rename = "type")]
    pub kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LoggingFile {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Default, Clone)]
pub struct Features {
    pub custom_resolution: bool,
}

pub fn rules_allow(rules: &Option<Vec<Rule>>, features: &Features) -> bool {
    let Some(rules) = rules else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, features) {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, features: &Features) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != crate::platform::OS {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            if arch != "x86_64" && arch != "x64" && arch != "amd64" {
                return false;
            }
        }
    }
    if let Some(wanted) = &rule.features {
        for (key, value) in wanted {
            let actual = match key.as_str() {
                "has_custom_resolution" => features.custom_resolution,
                _ => false,
            };
            if actual != *value {
                return false;
            }
        }
    }
    true
}

pub struct Coordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl Coordinate {
    pub fn parse(name: &str) -> Result<Coordinate> {
        let (main, extension) = match name.split_once('@') {
            Some((m, e)) => (m, e.to_string()),
            None => (name, "jar".to_string()),
        };
        let parts: Vec<&str> = main.split(':').collect();
        if parts.len() < 3 {
            return Err(Error::msg(format!("Invalid library name: {name}")));
        }
        Ok(Coordinate {
            group: parts[0].to_string(),
            artifact: parts[1].to_string(),
            version: parts[2].to_string(),
            classifier: parts.get(3).map(|s| s.to_string()),
            extension,
        })
    }

    pub fn path(&self) -> String {
        let file = match &self.classifier {
            Some(c) => format!("{}-{}-{}.{}", self.artifact, self.version, c, self.extension),
            None => format!("{}-{}.{}", self.artifact, self.version, self.extension),
        };
        format!("{}/{}/{}/{}", self.group.replace('.', "/"), self.artifact, self.version, file)
    }

    pub fn dedupe_key(&self) -> String {
        format!("{}:{}:{}", self.group, self.artifact, self.classifier.clone().unwrap_or_default())
    }
}

pub fn maven_path(name: &str) -> Result<String> {
    Ok(Coordinate::parse(name)?.path())
}

pub fn join_url(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path)
}

pub struct ResolvedLibrary {
    pub name: String,
    pub path: PathBuf,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub native: bool,
    pub classpath: bool,
    pub exclude: Vec<String>,
}

const LEGACY_MAVEN_FALLBACKS: [&str; 3] = [
    "https://libraries.minecraft.net/",
    "https://maven.minecraftforge.net/",
    "https://repo1.maven.org/maven2/",
];

pub fn fallback_urls(path: &str, primary: &str) -> Vec<String> {
    LEGACY_MAVEN_FALLBACKS
        .iter()
        .map(|base| join_url(base, path))
        .filter(|u| u != primary)
        .collect()
}

fn is_foreign_native(classifier: &str) -> bool {
    let c = classifier.to_ascii_lowercase();
    let ours = match crate::platform::OS {
        "windows" => c.contains("windows"),
        "osx" => c.contains("macos") || c.contains("osx"),
        _ => c.contains("linux"),
    };
    !ours || c.contains("arm64") || c.contains("aarch64") || c.contains("arm32") || c.ends_with("-x86")
}

pub fn resolve_libraries(version: &VersionJson, libraries_dir: &Path, features: &Features) -> Result<Vec<ResolvedLibrary>> {
    let mut out = Vec::new();
    for lib in &version.libraries {
        if !rules_allow(&lib.rules, features) || lib.clientreq == Some(false) {
            continue;
        }
        let coordinate = Coordinate::parse(&lib.name)?;
        if let Some(classifier) = &coordinate.classifier {
            if classifier.starts_with("natives-") && is_foreign_native(classifier) {
                continue;
            }
        }
        if let Some(natives) = &lib.natives {
            let Some(classifier) = natives.get(crate::platform::OS) else { continue };
            let classifier = classifier.replace("${arch}", "64");
            let artifact = lib
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.as_ref())
                .and_then(|c| c.get(&classifier))
                .cloned();
            let path = artifact
                .as_ref()
                .and_then(|a| a.path.clone())
                .unwrap_or_else(|| Coordinate { classifier: Some(classifier.clone()), ..Coordinate::parse(&lib.name).unwrap() }.path());
            let url = artifact
                .as_ref()
                .map(|a| a.url.clone())
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| join_url(lib.url.as_deref().unwrap_or("https://libraries.minecraft.net/"), &path));
            out.push(ResolvedLibrary {
                name: lib.name.clone(),
                path: libraries_dir.join(&path),
                url,
                sha1: artifact.as_ref().and_then(|a| a.sha1.clone()),
                size: artifact.as_ref().and_then(|a| a.size),
                native: true,
                classpath: false,
                exclude: lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default(),
            });
            if lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()).is_none() {
                continue;
            }
        }
        let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.clone());
        if lib.downloads.is_some() && artifact.is_none() {
            continue;
        }
        let path = artifact.as_ref().and_then(|a| a.path.clone()).unwrap_or_else(|| coordinate.path());
        let url = match &artifact {
            Some(a) => a.url.clone(),
            None => join_url(lib.url.as_deref().unwrap_or("https://libraries.minecraft.net/"), &path),
        };
        let is_native = coordinate.classifier.as_deref().map(|c| c.starts_with("natives-")).unwrap_or(false);
        out.push(ResolvedLibrary {
            name: lib.name.clone(),
            path: libraries_dir.join(&path),
            url,
            sha1: artifact.as_ref().and_then(|a| a.sha1.clone()).or_else(|| lib.sha1.clone()),
            size: artifact.as_ref().and_then(|a| a.size).or(lib.size),
            native: is_native,
            classpath: true,
            exclude: lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub fn merge(child: VersionJson, parent: VersionJson) -> VersionJson {
    let key_of = |lib: &Library| match Coordinate::parse(&lib.name) {
        Ok(c) => format!("{}{}", c.dedupe_key(), if lib.natives.is_some() { ":natives" } else { "" }),
        Err(_) => lib.name.clone(),
    };
    let overridden: std::collections::HashSet<String> = child.libraries.iter().filter(|l| l.rules.is_none()).map(key_of).collect();
    let mut libraries = child.libraries;
    libraries.extend(parent.libraries.into_iter().filter(|lib| !overridden.contains(&key_of(lib))));
    let arguments = match (parent.arguments, child.arguments) {
        (Some(p), Some(c)) => Some(Arguments {
            game: p.game.into_iter().chain(c.game).collect(),
            jvm: p.jvm.into_iter().chain(c.jvm).collect(),
        }),
        (p, c) => c.or(p),
    };
    VersionJson {
        id: child.id,
        inherits_from: None,
        main_class: child.main_class.or(parent.main_class),
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        libraries,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        logging: child.logging.or(parent.logging),
        kind: child.kind.or(parent.kind),
        jar: child.jar.or(parent.jar).or(Some(parent.id)),
        release_time: child.release_time.or(parent.release_time),
    }
}

pub fn version_json_path(versions_dir: &Path, id: &str) -> PathBuf {
    versions_dir.join(id).join(format!("{id}.json"))
}

pub fn load_local(versions_dir: &Path, id: &str) -> Result<VersionJson> {
    let path = version_json_path(versions_dir, id);
    let bytes = std::fs::read(&path).map_err(|_| Error::msg(format!("Version {id} is not installed")))?;
    Ok(serde_json::from_slice(&bytes).map_err(|e| Error::msg(format!("Version file for {id} is corrupted: {e}")))?)
}

pub fn load_merged(versions_dir: &Path, id: &str) -> Result<VersionJson> {
    let mut chain = vec![load_local(versions_dir, id)?];
    while let Some(parent_id) = chain.last().and_then(|v| v.inherits_from.clone()) {
        if chain.len() > 8 {
            return Err(Error::msg("Version inheritance is too deep"));
        }
        chain.push(load_local(versions_dir, &parent_id)?);
    }
    let mut merged = chain.pop().ok_or_else(|| Error::msg("Empty version chain"))?;
    while let Some(child) = chain.pop() {
        merged = merge(child, merged);
    }
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_paths() {
        assert_eq!(maven_path("org.lwjgl:lwjgl:3.3.3").unwrap(), "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar");
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(
            maven_path("net.minecraft:client:1.20.1-20230612.114412:mappings@txt").unwrap(),
            "net/minecraft/client/1.20.1-20230612.114412/client-1.20.1-20230612.114412-mappings.txt"
        );
        assert_eq!(maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap(), "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip");
        assert!(maven_path("broken").is_err());
    }

    #[test]
    fn rules_evaluate_for_this_os_x64() {
        let f = Features::default();
        let allow_win = Some(vec![Rule { action: "allow".into(), os: Some(OsRule { name: Some(crate::platform::OS.into()), ..Default::default() }), features: None }]);
        let allow_osx = Some(vec![Rule { action: "allow".into(), os: Some(OsRule { name: Some("osx".into()), ..Default::default() }), features: None }]);
        let disallow_osx = Some(vec![
            Rule { action: "allow".into(), os: None, features: None },
            Rule { action: "disallow".into(), os: Some(OsRule { name: Some("osx".into()), ..Default::default() }), features: None },
        ]);
        let x86_only = Some(vec![Rule { action: "allow".into(), os: Some(OsRule { arch: Some("x86".into()), ..Default::default() }), features: None }]);
        assert!(rules_allow(&None, &f));
        assert!(rules_allow(&allow_win, &f));
        assert!(!rules_allow(&allow_osx, &f));
        assert!(rules_allow(&disallow_osx, &f));
        assert!(!rules_allow(&x86_only, &f));
        let mut feat = HashMap::new();
        feat.insert("has_custom_resolution".to_string(), true);
        let res = Some(vec![Rule { action: "allow".into(), os: None, features: Some(feat) }]);
        assert!(!rules_allow(&res, &f));
        assert!(rules_allow(&res, &Features { custom_resolution: true }));
    }

    #[test]
    fn parses_modern_and_legacy_version_json() {
        let modern = r#"{"id":"1.21.1","mainClass":"net.minecraft.client.main.Main","arguments":{"game":["--username","${auth_player_name}",{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}"]}],"jvm":[{"rules":[{"action":"allow","os":{"name":"osx"}}],"value":"-XstartOnFirstThread"},"-cp","${classpath}"]},"libraries":[{"name":"org.lwjgl:lwjgl:3.3.3:natives-windows","downloads":{"artifact":{"path":"org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar","url":"https://libraries.minecraft.net/x.jar","sha1":"aa","size":1}},"rules":[{"action":"allow","os":{"name":"windows"}}]},{"name":"org.lwjgl:lwjgl:3.3.3:natives-windows-arm64","downloads":{"artifact":{"path":"p","url":"u","sha1":"bb","size":1}},"rules":[{"action":"allow","os":{"name":"windows"}}]},{"name":"org.lwjgl:lwjgl:3.3.3:natives-linux","downloads":{"artifact":{"path":"p2","url":"u2"}},"rules":[{"action":"allow","os":{"name":"linux"}}]}],"javaVersion":{"component":"java-runtime-delta","majorVersion":21},"assetIndex":{"id":"17","url":"https://x/17.json","sha1":"cc","size":1,"totalSize":2}}"#;
        let v: VersionJson = serde_json::from_str(modern).unwrap();
        let libs = resolve_libraries(&v, Path::new("L"), &Features::default()).unwrap();
        assert_eq!(libs.len(), 1);
        assert!(libs[0].native);
        let legacy = r#"{"id":"1.8.9","minecraftArguments":"--username ${auth_player_name}","mainClass":"net.minecraft.client.main.Main","libraries":[{"name":"org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209","natives":{"windows":"natives-windows-${arch}","linux":"natives-linux"},"extract":{"exclude":["META-INF/"]},"downloads":{"classifiers":{"natives-windows-64":{"path":"n64.jar","url":"https://x/n64.jar","sha1":"dd","size":5},"natives-windows-32":{"path":"n32.jar","url":"https://x/n32.jar"},"natives-linux":{"path":"nlinux.jar","url":"https://x/nlinux.jar","sha1":"ab","size":5}}}},{"name":"com.mojang:netty:1.8.8","downloads":{"artifact":{"path":"netty.jar","url":"https://x/netty.jar","sha1":"ee","size":9}}}]}"#;
        let v: VersionJson = serde_json::from_str(legacy).unwrap();
        let libs = resolve_libraries(&v, Path::new("L"), &Features::default()).unwrap();
        assert_eq!(libs.len(), 2);
        assert!(libs[0].native && libs[0].path.ends_with(if cfg!(windows) { "n64.jar" } else { "nlinux.jar" }));
        assert_eq!(libs[0].exclude, vec!["META-INF/".to_string()]);
        assert!(!libs[1].native);
    }

    #[test]
    fn maven_style_libraries_use_repository_url() {
        let fabric = r#"{"id":"fabric-loader-0.16.9-1.21.1","inheritsFrom":"1.21.1","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","libraries":[{"name":"net.fabricmc:fabric-loader:0.16.9","url":"https://maven.fabricmc.net/","sha1":"ff","size":10}]}"#;
        let v: VersionJson = serde_json::from_str(fabric).unwrap();
        let libs = resolve_libraries(&v, Path::new("L"), &Features::default()).unwrap();
        assert_eq!(libs[0].url, "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
        assert_eq!(libs[0].sha1.as_deref(), Some("ff"));
    }

    #[test]
    fn merge_prefers_child_and_dedupes_libraries() {
        let parent: VersionJson = serde_json::from_str(r#"{"id":"1.20.1","mainClass":"a.Main","libraries":[{"name":"org.ow2.asm:asm:9.3"},{"name":"com.google:gson:2.10"}],"arguments":{"game":["--a"],"jvm":["-cp","${classpath}"]},"assetIndex":{"id":"5","url":"u"},"assets":"5"}"#).unwrap();
        let child: VersionJson = serde_json::from_str(r#"{"id":"fabric","inheritsFrom":"1.20.1","mainClass":"b.Knot","libraries":[{"name":"org.ow2.asm:asm:9.7"}],"arguments":{"game":["--b"],"jvm":["-Dx=1"]}}"#).unwrap();
        let merged = merge(child, parent.clone());
        let mut dup_parent = parent;
        dup_parent.libraries.push(Library { name: "org.ow2.asm:asm:9.0".into(), rules: Some(vec![Rule { action: "allow".into(), os: Some(OsRule { name: Some("osx".into()), ..Default::default() }), features: None }]), ..Default::default() });
        let plain: VersionJson = serde_json::from_str(r#"{"id":"plain","inheritsFrom":"1.20.1","libraries":[]}"#).unwrap();
        assert_eq!(merge(plain, dup_parent).libraries.len(), 3);
        assert_eq!(merged.main_class.as_deref(), Some("b.Knot"));
        assert_eq!(merged.libraries.len(), 2);
        assert_eq!(merged.libraries[0].name, "org.ow2.asm:asm:9.7");
        let args = merged.arguments.unwrap();
        assert_eq!(args.game.len(), 2);
        assert_eq!(args.jvm.len(), 3);
        assert_eq!(merged.jar.as_deref(), Some("1.20.1"));
        assert_eq!(merged.assets.as_deref(), Some("5"));
    }
}
