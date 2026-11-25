use crate::auth;
use crate::diagnostics;
use crate::error::{Error, Result};
use crate::instances::{self, Instance};
use crate::minecraft::{install, java, launch, loaders, version};
use crate::storage;
use crate::tasks::Reporter;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, oneshot};

const LOG_LIMIT: usize = 4000;
const RUNNING_MARKERS: [&str; 8] = ["Horizon: rest-ready", "Setting user:", "LWJGL Version", "Backend library:", "OpenAL initialized", "Sound engine started", "Created:", "Narrator library"];

struct GameHandle {
    state: String,
    kill: Option<oneshot::Sender<()>>,
}

static GAMES: LazyLock<Mutex<HashMap<String, GameHandle>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static LOGS: LazyLock<Mutex<HashMap<String, VecDeque<String>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
pub static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub instance_id: String,
    pub state: String,
    pub message: Option<String>,
    pub exit_code: Option<i32>,
    pub crash_report: Option<String>,
    pub log_file: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogBatch {
    instance_id: String,
    lines: Vec<String>,
}

fn set_state(app: &AppHandle, id: &str, state: &str) {
    if let Ok(mut games) = GAMES.lock() {
        if let Some(handle) = games.get_mut(id) {
            handle.state = state.to_string();
        }
    }
    emit_state(app, GameState { instance_id: id.into(), state: state.into(), message: None, exit_code: None, crash_report: None, log_file: None });
}

fn emit_state(app: &AppHandle, state: GameState) {
    let _ = app.emit("game-state", state);
}

pub fn states() -> Vec<GameState> {
    GAMES
        .lock()
        .map(|games| {
            games
                .iter()
                .map(|(id, h)| GameState { instance_id: id.clone(), state: h.state.clone(), message: None, exit_code: None, crash_report: None, log_file: None })
                .collect()
        })
        .unwrap_or_default()
}

pub fn logs(id: &str) -> Vec<String> {
    LOGS.lock().ok().and_then(|l| l.get(id).map(|q| q.iter().cloned().collect())).unwrap_or_default()
}

pub fn kill(id: &str) -> Result<()> {
    let mut games = GAMES.lock().map_err(|_| Error::msg("Game registry busy"))?;
    let handle = games.get_mut(id).ok_or_else(|| Error::msg("This instance is not running"))?;
    match handle.kill.take() {
        Some(sender) => {
            let _ = sender.send(());
            Ok(())
        }
        None => Err(Error::msg("The game is still preparing")),
    }
}

fn push_log(id: &str, lines: &[String]) {
    if let Ok(mut logs) = LOGS.lock() {
        let queue = logs.entry(id.to_string()).or_default();
        for line in lines {
            queue.push_back(line.clone());
        }
        while queue.len() > LOG_LIMIT {
            queue.pop_front();
        }
    }
}

fn memory_for(instance: &Instance) -> (u32, u32) {
    let settings = storage::settings();
    let max = if instance.memory_max > 0 { instance.memory_max } else { settings.minecraft.memory_max };
    let min = if instance.memory_min > 0 { instance.memory_min } else { settings.minecraft.memory_min };
    (min.min(max), max)
}

fn resolution_for(instance: &Instance) -> (u32, u32) {
    let settings = storage::settings();
    if instance.width > 0 && instance.height > 0 {
        (instance.width, instance.height)
    } else {
        (settings.minecraft.width, settings.minecraft.height)
    }
}

pub async fn ensure_installed(instance: &mut Instance, reporter: &Reporter, verify: bool) -> Result<version::VersionJson> {
    if instance.loader == "horizon" {
        crate::horizon::ensure(reporter, verify).await?;
        if instance.version_id != "horizon" {
            instance.version_id = "horizon".into();
            instances::save(instance)?;
        }
        return Ok(version::VersionJson::default());
    }
    let versions = storage::versions_dir();
    let needs_install = instance.version_id.is_empty() || !version::version_json_path(&versions, &instance.version_id).is_file() || verify;
    if needs_install {
        reporter.phase("Resolving version");
        let id = loaders::install(&instance.loader, &instance.minecraft, &instance.loader_version, reporter).await?;
        instance.version_id = id;
        instances::save(instance)?;
    } else {
        let _ = install::ensure_vanilla_json(&instance.minecraft).await;
    }
    let width_height = resolution_for(instance);
    let features = version::Features { custom_resolution: width_height.0 > 0 && width_height.1 > 0 };
    install::install_version(&instance.version_id, reporter, verify, &features).await
}

fn newest_crash_report(game_dir: &Path, since: std::time::SystemTime) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for dir in [game_dir.join("crash-reports"), game_dir.to_path_buf()] {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let relevant = dir.ends_with("crash-reports") || (name.starts_with("hs_err_pid") && name.ends_with(".log"));
            if !relevant {
                continue;
            }
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                if modified >= since && best.as_ref().map(|b| modified > b.0).unwrap_or(true) {
                    best = Some((modified, entry.path()));
                }
            }
        }
    }
    best.map(|b| b.1)
}

pub fn crash_reason(lines: &[String]) -> Option<String> {
    let pattern = |l: &str| {
        let lower = l.to_ascii_lowercase();
        ["error", "exception", "could not", "unrecognized", "unsupported", "failed", "invalid"].iter().any(|k| lower.contains(k)) && !l.trim_start().starts_with("at ")
    };
    let picked: Vec<&String> = lines.iter().rev().filter(|l| pattern(l)).take(3).collect();
    let chosen: Vec<String> = if picked.is_empty() {
        lines.iter().rev().filter(|l| !l.trim().is_empty()).take(2).cloned().collect()
    } else {
        picked.into_iter().cloned().collect()
    };
    let text = chosen.into_iter().rev().map(|l| l.chars().take(300).collect::<String>()).collect::<Vec<_>>().join("\n");
    (!text.is_empty()).then_some(text)
}

fn prune_launch_logs(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<_> = entries.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("launch-")).collect();
    files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    while files.len() > 10 {
        let _ = std::fs::remove_file(files.remove(0).path());
    }
}

pub async fn launch(app: AppHandle, instance_id: String, server: Option<String>) -> Result<()> {
    {
        let mut games = GAMES.lock().map_err(|_| Error::msg("Game registry busy"))?;
        if games.contains_key(&instance_id) {
            return Err(Error::msg("This instance is already running or preparing"));
        }
        games.insert(instance_id.clone(), GameHandle { state: "preparing".into(), kill: None });
    }
    set_state(&app, &instance_id, "preparing");
    match prepare_and_spawn(&app, &instance_id, server).await {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Ok(mut games) = GAMES.lock() {
                games.remove(&instance_id);
            }
            diagnostics::error(&format!("Launch of {instance_id} failed: {}", e.0));
            emit_state(&app, GameState {
                instance_id: instance_id.clone(),
                state: "idle".into(),
                message: Some(e.0.clone()),
                exit_code: None,
                crash_report: None,
                log_file: None,
            });
            Err(e)
        }
    }
}

pub struct Prepared {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub instance: Instance,
    pub token: String,
    pub stdin: Option<String>,
    pub memory_watch: Option<u32>,
}

async fn prepare_horizon(mut instance: Instance, identity: auth::LaunchIdentity, reporter: &Reporter, on_state: &(dyn Fn(&str) + Send + Sync)) -> Result<Prepared> {
    let ready = {
        let _install = INSTALL_LOCK.lock().await;
        on_state("downloading");
        let ready = crate::horizon::ensure(reporter, false).await?;
        if instance.version_id != "horizon" {
            instance.version_id = "horizon".into();
            instances::save(&instance)?;
        }
        ready
    };
    on_state("launching");
    reporter.phase("Launching");
    let game_dir = instances::game_dir(&instance)?;
    std::fs::create_dir_all(game_dir.join("horizon-mods"))?;
    let path = |p: &Path| p.to_string_lossy().to_string();
    let mut args = vec!["--resources".to_string(), path(&ready.resources), "--game-dir".into(), path(&game_dir), "--home".into(), path(&game_dir)];
    let (width, height) = resolution_for(&instance);
    if width > 0 && height > 0 {
        args.extend(["--width".into(), width.to_string(), "--height".into(), height.to_string()]);
    }
    let stdin = if identity.user_type == "msa" {
        args.push("--session-stdin".into());
        Some(serde_json::json!({"name": identity.name, "uuid": identity.uuid, "access_token": identity.token, "user_type": "msa", "xuid": "0"}).to_string())
    } else {
        args.extend(["--name".into(), identity.name.clone()]);
        None
    };
    Ok(Prepared { java: ready.client, args, game_dir, instance, token: identity.token, stdin, memory_watch: None })
}

pub async fn prepare(instance_id: &str, reporter: &Reporter, on_state: &(dyn Fn(&str) + Send + Sync)) -> Result<Prepared> {
    let mut instance = instances::get(instance_id)?;
    let identity = auth::launch_identity(Some(instance.account_id.as_str()).filter(|s| !s.is_empty())).await?;
    if instance.loader == "horizon" {
        return prepare_horizon(instance, identity, reporter, on_state).await;
    }
    let quick = if instance.fast_launch && instance.checked { quick_prepare(&instance) } else { None };
    let fast = quick.is_some();
    let (merged, java_path) = match quick {
        Some(ready) => ready,
        None => {
            let _install = INSTALL_LOCK.lock().await;
            on_state("downloading");
            let merged = ensure_installed(&mut instance, reporter, false).await?;
            reporter.phase("Preparing Java");
            let java_path = java::select(merged.java_version.as_ref(), Some(instance.java_path.as_str()), reporter.progress()).await?;
            (merged, java_path)
        }
    };
    let (_, required_major) = java::component_for(merged.java_version.as_ref());
    let found_java = java::java_exe_version(&java_path);
    let java_major = found_java.as_ref().map(|j| j.major).filter(|m| *m > 0).unwrap_or(required_major);
    if let Some(found) = found_java {
        if found.major > 0 && found.major < required_major {
            return Err(Error::msg(format!(
                "Minecraft {} needs Java {required_major}, but the selected Java is {}. Choose a newer Java or use the managed runtime.",
                instance.minecraft, found.major
            )));
        }
        let wrapper = merged.main_class.as_deref().map(|m| m.contains("launchwrapper")).unwrap_or(false);
        if wrapper && required_major == 8 && found.major > 8 {
            return Err(Error::msg(format!("This version requires Java 8, but the selected Java is {}.", found.major)));
        }
    }
    on_state("launching");
    reporter.phase("Launching");
    let game_dir = instances::game_dir(&instance)?;
    std::fs::create_dir_all(&game_dir)?;
    crate::servers::sync_into(&game_dir);
    let (width, height) = resolution_for(&instance);
    let features = version::Features { custom_resolution: width > 0 && height > 0 };
    let natives_dir = storage::versions_dir().join(&instance.version_id).join("natives");
    install::extract_natives(&merged, &natives_dir, &features)?;
    let legacy = install::prepare_legacy_assets(&merged, &game_dir)?;
    if let Err(e) = crate::zipclient::apply(&game_dir, instance.zip_client, &instance.minecraft, &instance.loader) {
        diagnostics::error(&format!("Zip Client step skipped: {}", e.0));
    }
    let libraries_dir = storage::libraries_dir();
    let mut classpath: Vec<PathBuf> = version::resolve_libraries(&merged, &libraries_dir, &features)?
        .into_iter()
        .filter(|l| l.classpath)
        .map(|l| l.path)
        .collect();
    let jar = install::client_jar_path(&instance.version_id);
    let jar = if jar.is_file() { jar } else { install::client_jar_path(merged.jar.as_deref().unwrap_or(&instance.minecraft)) };
    if !jar.is_file() {
        return Err(Error::msg("The Minecraft client jar is missing. Use Repair on this instance."));
    }
    classpath.push(jar);
    let settings = storage::settings();
    let mut extra_jvm = launch::split_args(&settings.minecraft.jvm_args);
    extra_jvm.extend(launch::split_args(&instance.jvm_args));
    let (memory_min, memory_max) = memory_for(&instance);
    if instance.memory_warning {
        extra_jvm.push(if java_major >= 9 { "-Xlog:gc".into() } else { "-XX:+PrintGC".into() });
    }
    let log_config = merged.logging.as_ref().and_then(|l| l.get("client")).map(|l| {
        (l.argument.clone(), storage::assets_dir().join("log_configs").join(&l.file.id))
    });
    let ctx = launch::LaunchContext {
        version: merged.clone(),
        game_dir: game_dir.clone(),
        natives_dir,
        classpath,
        libraries_dir,
        assets_root: storage::assets_dir(),
        virtual_assets: legacy.virtual_dir,
        player_name: identity.name.clone(),
        uuid: identity.uuid.clone(),
        access_token: identity.token.clone(),
        user_type: identity.user_type.clone(),
        memory_min,
        memory_max,
        width,
        height,
        extra_jvm,
        log_config,
        launcher_version: env!("CARGO_PKG_VERSION").into(),
    };
    let args = launch::build(&ctx);
    let memory_watch = instance.memory_warning.then_some(memory_max);
    if instance.fast_launch && !fast {
        instance.checked = true;
        let _ = instances::save(&instance);
    }
    Ok(Prepared { java: java_path, args, game_dir, instance, token: identity.token, stdin: None, memory_watch })
}

fn quick_prepare(instance: &Instance) -> Option<(version::VersionJson, PathBuf)> {
    if instance.version_id.is_empty() {
        return None;
    }
    let merged = version::load_merged(&storage::versions_dir(), &instance.version_id).ok()?;
    let jar = install::client_jar_path(merged.jar.as_deref().unwrap_or(&instance.version_id));
    if !jar.is_file() && !install::client_jar_path(&instance.version_id).is_file() {
        return None;
    }
    let java = java::quick_select(merged.java_version.as_ref(), Some(instance.java_path.as_str()))?;
    Some((merged, java))
}

pub fn join_args(minecraft: &str, server: &str) -> Vec<String> {
    let server = server.trim();
    if server.is_empty() {
        return Vec::new();
    }
    let legacy = minecraft
        .strip_prefix("1.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|minor| minor.parse::<u32>().ok())
        .map(|minor| minor < 20)
        .unwrap_or(false);
    if !legacy {
        return vec!["--quickPlayMultiplayer".into(), server.into()];
    }
    let (host, port) = match server.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h, p),
        _ => (server, "25565"),
    };
    vec!["--server".into(), host.into(), "--port".into(), port.into()]
}

async fn prepare_and_spawn(app: &AppHandle, instance_id: &str, server: Option<String>) -> Result<()> {
    let reporter = Reporter::new(app, instance_id);
    let state_app = app.clone();
    let state_id = instance_id.to_string();
    let on_state = move |state: &str| set_state(&state_app, &state_id, state);
    let Prepared { java: java_path, mut args, game_dir, instance, token, stdin, memory_watch } = prepare(instance_id, &reporter, &on_state).await?;
    if let Some(server) = server.filter(|_| instance.loader != "horizon") {
        args.extend(join_args(&instance.minecraft, &server));
    }
    let settings = storage::settings();
    let log_dir = instances::dir(instance_id)?.join("launcher-logs");
    std::fs::create_dir_all(&log_dir)?;
    prune_launch_logs(&log_dir);
    let started_wall = instances::now();
    let log_path = log_dir.join(format!("launch-{started_wall}.log"));
    let mut log_file = std::fs::File::create(&log_path)?;
    let shown = launch::redact(&args, &token);
    let _ = writeln!(log_file, "Zip {} launching {} ({}) with {}", env!("CARGO_PKG_VERSION"), instance.name, instance.version_id, java_path.display());
    let _ = writeln!(log_file, "Arguments: {shown}");
    diagnostics::info(&format!("Launching {} ({}) using {}", instance.name, instance.version_id, java_path.display()));

    let mut command = tokio::process::Command::new(&java_path);
    command.args(&args).current_dir(&game_dir).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::platform::hide_console(&mut command);
    let mut child = command.spawn().map_err(|e| Error::msg(format!("Could not start the game ({}): {e}", java_path.display())))?;
    if let (Some(session), Some(mut pipe)) = (stdin, child.stdin.take()) {
        use tokio::io::AsyncWriteExt;
        pipe.write_all(session.as_bytes()).await?;
        drop(pipe);
    }
    reporter.phase("");
    drop(reporter);

    if let Ok(mut logs) = LOGS.lock() {
        logs.insert(instance_id.to_string(), VecDeque::new());
    }
    let (kill_tx, kill_rx) = oneshot::channel();
    if let Ok(mut games) = GAMES.lock() {
        if let Some(handle) = games.get_mut(instance_id) {
            handle.kill = Some(kill_tx);
        }
    }
    let (line_tx, line_rx) = mpsc::unbounded_channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        let tx = line_tx.clone();
        tauri::async_runtime::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = line_tx.clone();
        tauri::async_runtime::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    drop(line_tx);

    let app_for_logs = app.clone();
    let id_for_logs = instance_id.to_string();
    let log_task = tauri::async_runtime::spawn(pump_logs(app_for_logs, id_for_logs, line_rx, log_file, memory_watch));

    let after_launch = settings.general.after_launch.clone();
    let app = app.clone();
    let id = instance_id.to_string();
    let started = Instant::now();
    let started_system = std::time::SystemTime::now();
    tauri::async_runtime::spawn(async move {
        let mut kill_rx = kill_rx;
        let mut killed = false;
        let status = tokio::select! {
            status = child.wait() => status.ok(),
            _ = &mut kill_rx => {
                killed = true;
                let _ = child.start_kill();
                child.wait().await.ok()
            }
        };
        let _ = log_task.await;
        let duration = started.elapsed().as_secs();
        let exit_code = status.and_then(|s| s.code());
        let _ = instances::record_session(&id, started_wall, duration, if killed { None } else { exit_code });
        if let Ok(mut games) = GAMES.lock() {
            games.remove(&id);
        }
        let crashed = !killed && exit_code.map(|c| c != 0).unwrap_or(true);
        let crash_report = if crashed { newest_crash_report(&game_dir, started_system).map(|p| p.to_string_lossy().to_string()) } else { None };
        let reason = if crashed { crash_reason(&logs(&id)) } else { None };
        diagnostics::info(&format!("{id} exited with {exit_code:?} after {duration}s"));
        emit_state(&app, GameState {
            instance_id: id.clone(),
            state: if crashed { "crashed".into() } else { "stopped".into() },
            message: reason,
            exit_code,
            crash_report,
            log_file: Some(log_path.to_string_lossy().to_string()),
        });
        if after_launch != "keep" {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
            }
        }
    });
    Ok(())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameWarning {
    instance_id: String,
    message: String,
}

pub fn gc_used_after_mb(line: &str) -> Option<u32> {
    if !(line.contains("[gc") || line.contains("[GC") || line.contains("[Full GC")) {
        return None;
    }
    let arrow = line.find("->")?;
    let rest = &line[arrow + 2..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let value: u64 = digits.parse().ok()?;
    let unit = rest[digits.len()..].chars().next()?;
    let mb = match unit {
        'K' => value / 1024,
        'M' => value,
        'G' => value * 1024,
        _ => return None,
    };
    Some(mb as u32)
}

async fn pump_logs(app: AppHandle, id: String, mut rx: mpsc::UnboundedReceiver<String>, mut file: std::fs::File, memory_watch: Option<u32>) {
    let mut running = false;
    let mut high_streak = 0u32;
    let mut last_warning: Option<Instant> = None;
    let started = Instant::now();
    let after_launch = storage::settings().general.after_launch;
    let mut batch: Vec<String> = Vec::new();
    let mut xml = Log4jXml::default();
    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    loop {
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(raw) => {
                        for line in xml.feed(&raw) {
                            if !running && RUNNING_MARKERS.iter().any(|m| line.contains(m)) {
                                running = true;
                                mark_running(&app, &id, &after_launch);
                            }
                            let _ = writeln!(file, "{line}");
                            if let Some(max) = memory_watch {
                                let oom = line.contains("java.lang.OutOfMemoryError");
                                if let Some(used) = gc_used_after_mb(&line) {
                                    high_streak = if used * 10 >= max * 9 { high_streak + 1 } else { 0 };
                                    if high_streak < 3 && !oom {
                                        continue;
                                    }
                                }
                                if (high_streak >= 3 || oom) && last_warning.map(|t| t.elapsed() > Duration::from_secs(600)).unwrap_or(true) {
                                    last_warning = Some(Instant::now());
                                    high_streak = 0;
                                    let message = if oom { format!("Minecraft ran out of memory ({max} MB). Give it more RAM in the instance settings.") } else { format!("Minecraft is close to its {max} MB memory limit. Give it more RAM in the instance settings.") };
                                    let _ = app.emit("game-warning", GameWarning { instance_id: id.clone(), message });
                                }
                                if gc_used_after_mb(&line).is_some() {
                                    continue;
                                }
                            }
                            batch.push(line);
                        }
                        if batch.len() >= 400 {
                            flush(&app, &id, &mut batch);
                        }
                    }
                    None => break,
                }
            }
            _ = ticker.tick() => {
                flush(&app, &id, &mut batch);
                if !running && started.elapsed() > Duration::from_secs(10) {
                    running = true;
                    mark_running(&app, &id, &after_launch);
                }
            }
        }
    }
    flush(&app, &id, &mut batch);
}

#[derive(Default)]
pub struct Log4jXml {
    buffer: String,
    active: bool,
}

fn attribute(text: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    text.find(&key)
        .and_then(|start| {
            let rest = &text[start + key.len()..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
        .unwrap_or_default()
}

fn cdata(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&format!("</{tag}>"))? + start;
    let inner = text[start..end].trim();
    Some(inner.strip_prefix("<![CDATA[").and_then(|s| s.strip_suffix("]]>")).unwrap_or(inner).to_string())
}

impl Log4jXml {
    pub fn feed(&mut self, line: &str) -> Vec<String> {
        if !self.active {
            if line.trim_start().starts_with("<log4j:Event") {
                self.active = true;
                self.buffer.clear();
            } else {
                return vec![line.to_string()];
            }
        }
        self.buffer.push_str(line);
        self.buffer.push('\n');
        if !line.contains("</log4j:Event>") {
            if self.buffer.len() > 256 * 1024 {
                self.active = false;
                return std::mem::take(&mut self.buffer).lines().map(String::from).collect();
            }
            return Vec::new();
        }
        self.active = false;
        let event = std::mem::take(&mut self.buffer);
        let level = attribute(&event, "level");
        let thread = attribute(&event, "thread");
        let message = cdata(&event, "log4j:Message").unwrap_or_default();
        let mut out: Vec<String> = Vec::new();
        for (i, part) in message.lines().enumerate() {
            out.push(if i == 0 { format!("[{thread}/{level}]: {part}") } else { part.to_string() });
        }
        if out.is_empty() {
            out.push(format!("[{thread}/{level}]:"));
        }
        if let Some(throwable) = cdata(&event, "log4j:Throwable") {
            out.extend(throwable.lines().map(String::from));
        }
        out
    }
}

fn mark_running(app: &AppHandle, id: &str, after_launch: &str) {
    let still_alive = GAMES.lock().map(|g| g.contains_key(id)).unwrap_or(false);
    if !still_alive {
        return;
    }
    set_state(app, id, "running");
    if let Some(window) = app.get_webview_window("main") {
        match after_launch {
            "minimize" => {
                let _ = window.minimize();
            }
            "close" => {
                let _ = window.hide();
            }
            _ => {}
        }
    }
}

fn flush(app: &AppHandle, id: &str, batch: &mut Vec<String>) {
    if batch.is_empty() {
        return;
    }
    let lines = std::mem::take(batch);
    push_log(id, &lines);
    let _ = app.emit("game-log", LogBatch { instance_id: id.to_string(), lines });
}

#[cfg(test)]
mod e2e {
    use super::*;

    const SUCCESS_MARKERS: [&str; 6] = ["Sound engine started", "OpenAL initialized", "Reloading ResourceManager", "Backend library: LWJGL", "Created: ", "Narrator library"];

    async fn run_case(minecraft: &str, loader: &str) -> std::result::Result<String, String> {
        let loader_version = if loader == "vanilla" {
            String::new()
        } else {
            let list = loaders::list(loader, minecraft).await.map_err(|e| e.0)?;
            list.iter().find(|v| v.recommended).or(list.first()).ok_or("no loader versions")?.version.clone()
        };
        let name = format!("e2e {loader} {minecraft}");
        let instance = instances::list()
            .into_iter()
            .find(|i| i.name == name && i.loader_version == loader_version)
            .map(Ok)
            .unwrap_or_else(|| {
                instances::create(instances::NewInstance {
                    name: name.clone(),
                    minecraft: minecraft.into(),
                    loader: loader.into(),
                    loader_version: loader_version.clone(),
                    group: "e2e".into(),
                })
            })
            .map_err(|e| e.0)?;
        let reporter = Reporter::silent(name.clone());
        let started = Instant::now();
        let prepared = prepare(&instance.id, &reporter, &|_| {}).await.map_err(|e| e.0)?;
        let install_secs = started.elapsed().as_secs();
        let mut command = tokio::process::Command::new(&prepared.java);
        command.args(&prepared.args).current_dir(&prepared.game_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        crate::platform::hide_console(&mut command);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>)].into_iter().flatten() {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(line);
                }
            });
        }
        drop(tx);
        let mut tail: VecDeque<String> = VecDeque::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
        let alive_pass = tokio::time::Instant::now() + Duration::from_secs(75);
        let outcome = loop {
            tokio::select! {
                line = rx.recv() => match line {
                    Some(line) => {
                        if let Some(marker) = SUCCESS_MARKERS.iter().find(|m| line.contains(*m)) {
                            break Ok(format!("{loader} {loader_version} on {minecraft} ({}): installed in {install_secs}s, reached \"{marker}\" after {}s", prepared.instance.version_id, started.elapsed().as_secs() - install_secs));
                        }
                        tail.push_back(line);
                        if tail.len() > 40 { tail.pop_front(); }
                    }
                    None => {
                        let status = child.wait().await.ok().and_then(|s| s.code());
                        break Err(format!("{loader} {minecraft}: game exited early ({status:?})\n{}", tail.iter().cloned().collect::<Vec<_>>().join("\n")));
                    }
                },
                _ = tokio::time::sleep_until(alive_pass) => {
                    break Ok(format!("{loader} {loader_version} on {minecraft} ({}): installed in {install_secs}s, still running after 75s without crashing (no known startup marker)
  last: {}", prepared.instance.version_id, tail.back().cloned().unwrap_or_default()));
                }
                _ = tokio::time::sleep_until(deadline) => {
                    break Err(format!("{loader} {minecraft}: no startup marker within timeout\n{}", tail.iter().cloned().collect::<Vec<_>>().join("\n")));
                }
            }
        };
        let _ = child.start_kill();
        let _ = child.wait().await;
        outcome
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn install_and_launch_loaders() {
        let dir = std::env::var("ZIP_E2E_DIR").unwrap_or_else(|_| std::env::temp_dir().join("zip-e2e").to_string_lossy().to_string());
        std::env::set_var("ZIP_DATA_DIR", &dir);
        storage::init_root().unwrap();
        storage::load_settings();
        auth::add_offline("ZipTester").unwrap();
        let cases = std::env::var("ZIP_E2E_CASES").unwrap_or_else(|_| "vanilla:1.21.1,fabric:1.21.1,quilt:1.21.1,forge:1.20.1,neoforge:1.21.1".into());
        let mut failures = Vec::new();
        for case in cases.split(',') {
            let (loader, minecraft) = case.split_once(':').unwrap();
            match run_case(minecraft, loader).await {
                Ok(report) => println!("PASS {report}"),
                Err(e) => {
                    println!("FAIL {e}");
                    failures.push(e);
                }
            }
        }
        assert!(failures.is_empty(), "{} case(s) failed", failures.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_arguments() {
        assert_eq!(join_args("1.21.1", "play.example.com"), vec!["--quickPlayMultiplayer", "play.example.com"]);
        assert_eq!(join_args("26.1", "a:25570"), vec!["--quickPlayMultiplayer", "a:25570"]);
        assert_eq!(join_args("1.8.9", "mc.host.net:25570"), vec!["--server", "mc.host.net", "--port", "25570"]);
        assert_eq!(join_args("1.12.2", "mc.host.net"), vec!["--server", "mc.host.net", "--port", "25565"]);
        assert!(join_args("1.20.1", " ").is_empty());
    }

    #[test]
    fn parses_gc_lines() {
        assert_eq!(gc_used_after_mb("[12.3s][info][gc] GC(4) Pause Young (Normal) (G1 Evacuation Pause) 1500M->1200M(2048M) 4.1ms"), Some(1200));
        assert_eq!(gc_used_after_mb("[GC pause (G1 Evacuation Pause) (young) 512M->123M(2048M), 0.0123 secs]"), Some(123));
        assert_eq!(gc_used_after_mb("[Full GC (Allocation Failure)  3G->2G(4G), 1.2 secs]"), Some(2048));
        assert_eq!(gc_used_after_mb("[main/INFO]: x->y"), None);
    }

    #[test]
    fn crash_reason_picks_error_lines() {
        let lines: Vec<String> = ["Starting", "Unrecognized VM option 'Foo'", "Error: Could not create the Java Virtual Machine.", "Error: A fatal exception has occurred. Program will exit."].iter().map(|s| s.to_string()).collect();
        let reason = crash_reason(&lines).unwrap();
        assert!(reason.starts_with("Unrecognized VM option"));
        assert_eq!(reason.lines().count(), 3);
        assert_eq!(crash_reason(&["".to_string()]), None);
        assert_eq!(crash_reason(&["bye".to_string()]).as_deref(), Some("bye"));
    }

    #[test]
    fn parses_log4j_xml_events() {
        let mut p = Log4jXml::default();
        assert_eq!(p.feed("plain line"), vec!["plain line"]);
        assert!(p.feed(r#"<log4j:Event logger="net.minecraft.class_1059" timestamp="1" level="INFO" thread="Render thread">"#).is_empty());
        assert!(p.feed("  <log4j:Message><![CDATA[Created: 256x256x4 atlas]]></log4j:Message>").is_empty());
        let out = p.feed("</log4j:Event>");
        assert_eq!(out, vec!["[Render thread/INFO]: Created: 256x256x4 atlas"]);
        p.feed(r#"<log4j:Event logger="x" timestamp="1" level="ERROR" thread="main">"#);
        p.feed("<log4j:Message><![CDATA[Boom]]></log4j:Message>");
        p.feed("<log4j:Throwable><![CDATA[java.lang.RuntimeException: x");
        p.feed("\tat a.b(c.java:1)]]></log4j:Throwable>");
        let out = p.feed("</log4j:Event>");
        assert_eq!(out[0], "[main/ERROR]: Boom");
        assert_eq!(out[1], "java.lang.RuntimeException: x");
        assert_eq!(out.len(), 3);
    }
}
