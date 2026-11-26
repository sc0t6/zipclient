use std::cmp::Ordering;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct FabricMod {
    pub id: String,
    pub name: String,
    pub version: String,
    pub provides: Vec<String>,
    pub depends: Vec<(String, Vec<String>)>,
    pub breaks: Vec<(String, Vec<String>)>,
}

fn predicates(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec!["*".into()],
    }
}

fn parse_meta(text: &str) -> Option<FabricMod> {
    let cleaned: String = text.trim_start_matches('\u{feff}').chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let json: serde_json::Value = serde_json::from_str(&cleaned).ok()?;
    let map = |key: &str| json[key].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), predicates(v))).collect()).unwrap_or_default();
    Some(FabricMod {
        id: json["id"].as_str()?.to_string(),
        name: json["name"].as_str().unwrap_or_default().to_string(),
        version: json["version"].as_str().unwrap_or_default().to_string(),
        provides: json["provides"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        depends: map("depends"),
        breaks: map("breaks"),
    })
}

fn read_archive<R: Read + std::io::Seek>(archive: &mut zip::ZipArchive<R>, depth: u8, out: &mut Vec<FabricMod>) {
    let mut text = String::new();
    if let Ok(mut entry) = archive.by_name("fabric.mod.json") {
        if entry.size() < 1024 * 1024 && entry.read_to_string(&mut text).is_ok() {
            out.extend(parse_meta(&text));
        }
    }
    if depth >= 2 {
        return;
    }
    let nested: Vec<String> = archive.file_names().filter(|n| n.starts_with("META-INF/jars/") && n.ends_with(".jar")).map(String::from).collect();
    for name in nested {
        let mut bytes = Vec::new();
        if let Ok(mut entry) = archive.by_name(&name) {
            if entry.size() > 64 * 1024 * 1024 || entry.read_to_end(&mut bytes).is_err() {
                continue;
            }
        }
        if let Ok(mut inner) = zip::ZipArchive::new(Cursor::new(bytes)) {
            read_archive(&mut inner, depth + 1, out);
        }
    }
}

pub fn read_jar(path: &Path) -> Vec<FabricMod> {
    let mut out = Vec::new();
    if let Some(mut archive) = std::fs::File::open(path).ok().and_then(|f| zip::ZipArchive::new(f).ok()) {
        read_archive(&mut archive, 0, &mut out);
    }
    out
}

fn split(version: &str) -> (Vec<String>, Vec<String>) {
    let core = version.trim_start_matches(['v', 'V']).split('+').next().unwrap_or_default();
    let (main, pre) = core.split_once('-').unwrap_or((core, ""));
    let parts = |s: &str| s.split('.').filter(|p| !p.is_empty()).map(String::from).collect::<Vec<_>>();
    (parts(main), parts(pre))
}

fn part_cmp(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

pub fn compare(a: &str, b: &str) -> Ordering {
    let (am, ap) = split(a);
    let (bm, bp) = split(b);
    for i in 0..am.len().max(bm.len()) {
        let c = part_cmp(am.get(i).map_or("0", |s| s), bm.get(i).map_or("0", |s| s));
        if c != Ordering::Equal {
            return c;
        }
    }
    match (ap.is_empty(), bp.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => {
            for i in 0..ap.len().max(bp.len()) {
                match (ap.get(i), bp.get(i)) {
                    (None, _) => return Ordering::Less,
                    (_, None) => return Ordering::Greater,
                    (Some(x), Some(y)) => {
                        let c = part_cmp(x, y);
                        if c != Ordering::Equal {
                            return c;
                        }
                    }
                }
            }
            Ordering::Equal
        }
    }
}

fn bump(version: &str, index: usize) -> String {
    let mut nums: Vec<u64> = split(version).0.iter().map(|p| p.parse().unwrap_or(0)).collect();
    while nums.len() <= index {
        nums.push(0);
    }
    nums[index] += 1;
    nums[..=index].iter().map(u64::to_string).collect::<Vec<_>>().join(".")
}

fn single(version: &str, predicate: &str) -> bool {
    let p = predicate.trim();
    if p.is_empty() || p == "*" {
        return true;
    }
    let (op, target) = [">=", "<=", ">", "<", "=", "~", "^"].iter().find_map(|op| p.strip_prefix(op).map(|t| (*op, t.trim()))).unwrap_or(("", p));
    if target.split('.').any(|s| matches!(s, "x" | "X" | "*")) {
        let fixed: Vec<&str> = target.split('.').take_while(|s| !matches!(*s, "x" | "X" | "*")).collect();
        if fixed.is_empty() {
            return true;
        }
        let base = fixed.join(".");
        return compare(version, &base) != Ordering::Less && compare(version, &bump(&base, fixed.len() - 1)) == Ordering::Less;
    }
    let c = compare(version, target);
    match op {
        ">=" => c != Ordering::Less,
        "<=" => c != Ordering::Greater,
        ">" => c == Ordering::Greater,
        "<" => c == Ordering::Less,
        "~" => c != Ordering::Less && compare(version, &bump(target, 1)) == Ordering::Less,
        "^" => c != Ordering::Less && compare(version, &bump(target, 0)) == Ordering::Less,
        _ => c == Ordering::Equal,
    }
}

pub fn satisfies(version: &str, predicates: &[String]) -> bool {
    predicates.iter().any(|p| p.split_whitespace().all(|part| single(version, part)))
}

fn short(version: &str) -> &str {
    version.split('+').next().unwrap_or(version)
}

fn title(m: &FabricMod) -> &str {
    if m.name.is_empty() {
        &m.id
    } else {
        &m.name
    }
}

fn label(m: &FabricMod) -> String {
    format!("{} {}", title(m), short(&m.version))
}

pub fn conflict(mods_dir: &Path, new_jar: &Path, replaced: &[PathBuf]) -> Option<String> {
    let added = read_jar(new_jar);
    let incoming = added.first()?;
    let mut installed: Vec<FabricMod> = Vec::new();
    for entry in std::fs::read_dir(mods_dir).ok()?.flatten() {
        let path = entry.path();
        if path == new_jar || replaced.contains(&path) || path.extension().and_then(|e| e.to_str()) != Some("jar") {
            continue;
        }
        installed.extend(read_jar(&path).into_iter().take(1));
    }
    let ids: Vec<&str> = std::iter::once(incoming.id.as_str()).chain(incoming.provides.iter().map(String::as_str)).collect();
    for m in &installed {
        let mine: Vec<&str> = std::iter::once(m.id.as_str()).chain(m.provides.iter().map(String::as_str)).collect();
        let blocked = incoming.breaks.iter().any(|(dep, p)| mine.contains(&dep.as_str()) && satisfies(&m.version, p))
            || m.breaks.iter().any(|(dep, p)| ids.contains(&dep.as_str()) && satisfies(&incoming.version, p));
        if blocked {
            return Some(format!("{} does not work with {}, which is installed. Keep the current version or update {} first.", label(incoming), label(m), title(m)));
        }
        if let Some((_, p)) = m.depends.iter().find(|(dep, p)| ids.contains(&dep.as_str()) && !satisfies(&incoming.version, p)) {
            return Some(format!("{} needs {} {}, not {}. Keep the current version or update {} first.", label(m), incoming.id, p.join(" or "), short(&incoming.version), title(m)));
        }
        if let Some((_, p)) = incoming.depends.iter().find(|(dep, p)| mine.contains(&dep.as_str()) && !satisfies(&m.version, p)) {
            return Some(format!("{} needs {} {}, but {} is installed. Update {} first.", label(incoming), title(m), p.join(" or "), short(&m.version), title(m)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn version_rules() {
        assert!(satisfies("1.11.7+mc26.3", &s(&["<=1.12.0"])));
        assert!(!satisfies("1.12.1+mc26.3", &s(&["<=1.12.0"])));
        assert!(satisfies("0.9.2+mc26.3", &s(&["<=0.9.3-beta.1"])));
        assert!(!satisfies("0.9.3+mc26.3", &s(&["<=0.9.3-beta.1"])));
        assert!(satisfies("0.9.3+mc26.3", &s(&["0.9.3+mc26.3"])));
        assert!(!satisfies("0.9.4-beta.2+mc26.3", &s(&["=0.9.3+mc26.3"])));
        assert!(satisfies("1.21.11", &s(&[">=1.21.10 <1.22"])));
        assert!(satisfies("26.3", &s(&["~26.3"])));
        assert!(!satisfies("26.4", &s(&["~26.3"])));
        assert!(satisfies("1.21.4", &s(&["1.21.x"])));
        assert!(satisfies("2.0.0", &s(&["<1.0", ">=2.0"])));
        assert!(satisfies("0.5.5", &s(&["*"])));
        assert_eq!(compare("0.9.3-beta.3", "0.9.3"), Ordering::Less);
        assert_eq!(compare("1.10.0", "1.9.9"), Ordering::Greater);
    }

    fn jar(dir: &Path, name: &str, meta: &str) -> PathBuf {
        use std::io::Write;
        let path = dir.join(name);
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        writer.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(meta.as_bytes()).unwrap();
        writer.finish().unwrap();
        path
    }

    #[test]
    fn finds_breaking_updates() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path();
        jar(mods, "iris.jar", r#"{"id":"iris","name":"Iris","version":"1.11.7+mc26.3"}"#);
        let old = jar(mods, "sodium-old.jar", r#"{"id":"sodium","name":"Sodium","version":"0.9.2+mc26.3"}"#);
        let new = jar(mods, "sodium-new.jar", r#"{"id":"sodium","name":"Sodium","version":"0.9.3+mc26.3","breaks":{"iris":"<=1.12.0"}}"#);
        let msg = conflict(mods, &new, std::slice::from_ref(&old)).unwrap();
        assert!(msg.starts_with("Sodium 0.9.3 does not work with Iris 1.11.7"), "{msg}");
        std::fs::remove_file(mods.join("iris.jar")).unwrap();
        jar(mods, "extra.jar", r#"{"id":"sodium-extra","name":"Sodium Extra","version":"0.9.5","depends":{"sodium":"0.9.2+mc26.3"}}"#);
        let msg = conflict(mods, &new, std::slice::from_ref(&old)).unwrap();
        assert!(msg.starts_with("Sodium Extra 0.9.5 needs sodium 0.9.2+mc26.3, not 0.9.3"), "{msg}");
        std::fs::remove_file(mods.join("extra.jar")).unwrap();
        assert_eq!(conflict(mods, &new, std::slice::from_ref(&old)), None);
        let bbe = jar(mods, "bbe.jar", r#"{"id":"betterblockentities","name":"Better Block Entities","version":"1.4.1","depends":{"sodium":">=0.9.3-beta.3","fabric-api":"*"}}"#);
        std::fs::remove_file(&new).unwrap();
        let msg = conflict(mods, &bbe, &[]).unwrap();
        assert!(msg.starts_with("Better Block Entities 1.4.1 needs Sodium >=0.9.3-beta.3, but 0.9.2 is installed"), "{msg}");
    }
}
