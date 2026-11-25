use crate::error::Result;
use crate::minecraft::{install, java, version};
use crate::storage;
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub memory_min: u32,
    pub memory_max: u32,
    pub jvm_args: String,
    pub java_major: u32,
    pub system_mb: u32,
}

pub fn system_memory_mb() -> u32 {
    crate::platform::total_memory_mb()
}

pub fn recommend(java_major: u32, loader: &str, system_mb: u32) -> Preset {
    let wanted: u32 = match loader {
        "forge" | "neoforge" => 6144,
        "fabric" | "quilt" => 4096,
        _ if java_major <= 8 => 2048,
        _ => 3072,
    };
    let cap = system_mb.saturating_sub(4096).min(system_mb * 6 / 10).max(1024);
    let memory_max = (wanted.min(cap) / 512 * 512).max(1024);
    let memory_min = (memory_max / 2 / 512 * 512).max(512);
    let region = if memory_max >= 12288 { "16M" } else { "8M" };
    let mut args = vec![
        "-XX:+UseG1GC".to_string(),
        "-XX:+ParallelRefProcEnabled".into(),
        "-XX:MaxGCPauseMillis=200".into(),
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:+DisableExplicitGC".into(),
        "-XX:G1NewSizePercent=30".into(),
        "-XX:G1MaxNewSizePercent=40".into(),
        format!("-XX:G1HeapRegionSize={region}"),
        "-XX:G1ReservePercent=20".into(),
        "-XX:G1HeapWastePercent=5".into(),
        "-XX:G1MixedGCCountTarget=4".into(),
        "-XX:InitiatingHeapOccupancyPercent=15".into(),
        "-XX:G1MixedGCLiveThresholdPercent=90".into(),
        "-XX:SurvivorRatio=32".into(),
        "-XX:+PerfDisableSharedMem".into(),
        "-XX:MaxTenuringThreshold=1".into(),
    ];
    if java_major <= 17 {
        args.push("-XX:G1RSetUpdatingPauseTimePercent=5".into());
    }
    Preset { memory_min, memory_max, jvm_args: args.join(" "), java_major, system_mb }
}

pub async fn preset_for(minecraft: &str, loader: &str) -> Result<Preset> {
    install::ensure_vanilla_json(minecraft).await?;
    let local = version::load_local(&storage::versions_dir(), minecraft)?;
    let (_, major) = java::component_for(local.java_version.as_ref());
    Ok(recommend(major, loader, system_memory_mb()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommendations_scale_with_loader_and_system() {
        let big = recommend(21, "neoforge", 32768);
        assert_eq!(big.memory_max, 6144);
        assert_eq!(big.memory_min, 3072);
        assert!(!big.jvm_args.contains("G1RSetUpdatingPauseTimePercent"));
        let small = recommend(21, "forge", 8192);
        assert_eq!(small.memory_max, 4096);
        let tiny = recommend(8, "vanilla", 4096);
        assert_eq!(tiny.memory_max, 1024);
        assert!(tiny.jvm_args.contains("G1RSetUpdatingPauseTimePercent"));
        assert_eq!(recommend(21, "vanilla", 16384).memory_max, 3072);
        assert_eq!(recommend(17, "fabric", 16384).memory_max, 4096);
        assert!(system_memory_mb() > 0);
    }
}
