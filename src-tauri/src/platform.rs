use crate::error::{Error, Result};
use std::path::Path;

pub const OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "osx"
} else {
    "linux"
};

pub const EXE: &str = if cfg!(windows) { ".exe" } else { "" };

pub const CLASSPATH_SEPARATOR: &str = if cfg!(windows) { ";" } else { ":" };

pub const MOJANG_RUNTIME_PLATFORM: &str = if cfg!(windows) { "windows-x64" } else { "linux" };

pub fn hide_console(command: &mut tokio::process::Command) {
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    #[cfg(not(windows))]
    let _ = command;
}

pub fn open(path: &Path) -> Result<()> {
    let program = if cfg!(windows) { "explorer" } else { "xdg-open" };
    std::process::Command::new(program).arg(path).spawn().map_err(|e| Error::msg(format!("Could not open {}: {e}", path.display())))?;
    Ok(())
}

pub fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        std::fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub fn restrict_to_user(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub fn os_name() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .creation_flags(0x0800_0000)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Windows".into())
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|text| text.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
            .unwrap_or_else(|| std::env::consts::OS.to_string())
    }
}

pub fn total_memory_mb() -> u32 {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut status) == 0 {
            return 8192;
        }
        (status.ullTotalPhys / 1024 / 1024) as u32
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| meminfo_total_mb(&text))
            .unwrap_or(8192)
    }
}

pub fn meminfo_total_mb(text: &str) -> Option<u32> {
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some((kb / 1024) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_constants_match_target() {
        assert_eq!(EXE.is_empty(), !cfg!(windows));
        assert_eq!(CLASSPATH_SEPARATOR == ";", cfg!(windows));
        assert!(total_memory_mb() > 0);
        assert!(!os_name().is_empty());
        assert_eq!(meminfo_total_mb("MemTotal:       16384000 kB\nMemFree: 1 kB\n"), Some(16000));
        assert_eq!(meminfo_total_mb("garbage"), None);
    }
}
