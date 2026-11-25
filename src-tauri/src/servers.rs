use crate::error::{Error, Result};
use crate::minecraft::{install, java, version};
use crate::net::{self, DownloadItem};
use crate::storage;
use crate::tasks::Reporter;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use crate::nbt::{self, Tag};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

const LOG_LIMIT: usize = 2000;
const CONFIG_FILE: &str = "zip-server.json";

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedServer {
    pub id: String,
    pub name: String,
    pub address: String,
    pub instance_id: String,
    pub added: u64,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct SavedFile {
    servers: Vec<SavedServer>,
}

static SAVED_LOCK: Mutex<()> = Mutex::new(());

fn saved_path() -> PathBuf {
    storage::root().join("servers.json")
}

fn load_saved() -> SavedFile {
    storage::read_json(&saved_path()).ok().flatten().unwrap_or_default()
}

pub fn saved() -> Vec<SavedServer> {
    load_saved().servers
}

fn valid_address(address: &str) -> bool {
    !address.is_empty() && address.len() <= 255 && address.chars().all(|c| c.is_ascii_alphanumeric() || ".-_:[]".contains(c))
}

pub fn save_server(mut server: SavedServer) -> Result<SavedServer> {
    let _guard = SAVED_LOCK.lock().map_err(|_| Error::msg("Server list busy"))?;
    server.address = server.address.trim().trim_start_matches("minecraft://").to_string();
    server.name = server.name.trim().chars().take(48).collect();
    if !valid_address(&server.address) {
        return Err(Error::msg("Enter a valid server address, like play.example.com or 127.0.0.1:25565"));
    }
    if server.name.is_empty() {
        server.name = server.address.clone();
    }
    let mut file = load_saved();
    let mut previous = None;
    match file.servers.iter_mut().find(|s| !server.id.is_empty() && s.id == server.id) {
        Some(existing) => {
            previous = Some(existing.address.clone());
            *existing = server.clone();
        }
        None => {
            server.id = format!("s{}", now_millis());
            server.added = crate::instances::now();
            file.servers.push(server.clone());
        }
    }
    storage::write_json(&saved_path(), &file)?;
    let (name, address) = (server.name.clone(), server.address.clone());
    std::thread::spawn(move || {
        for dir in game_dirs() {
            let _ = edit_dat(&dir, |list| upsert(list, previous.as_deref(), &name, &address));
        }
    });
    Ok(server)
}

pub fn remove_saved(id: &str) -> Result<()> {
    let _guard = SAVED_LOCK.lock().map_err(|_| Error::msg("Server list busy"))?;
    let mut file = load_saved();
    let removed: Vec<String> = file.servers.iter().filter(|s| s.id == id).map(|s| s.address.clone()).collect();
    file.servers.retain(|s| s.id != id);
    storage::write_json(&saved_path(), &file)?;
    std::thread::spawn(move || {
        for dir in game_dirs() {
            let _ = edit_dat(&dir, |list| {
                let before = list.len();
                list.retain(|entry| !removed.iter().any(|a| entry_ip(entry).map(|ip| ip.eq_ignore_ascii_case(a)).unwrap_or(false)));
                list.len() != before
            });
        }
    });
    Ok(())
}

fn game_dirs() -> Vec<PathBuf> {
    crate::instances::list().iter().filter(|i| i.loader != "horizon").filter_map(|i| crate::instances::game_dir(i).ok()).collect()
}

fn entry_ip(entry: &Tag) -> Option<&str> {
    entry.get("ip").and_then(Tag::as_str)
}

fn upsert(list: &mut Vec<Tag>, previous: Option<&str>, name: &str, address: &str) -> bool {
    let matches = |entry: &Tag, ip: &str| entry_ip(entry).map(|e| e.eq_ignore_ascii_case(ip)).unwrap_or(false);
    let index = previous.and_then(|old| list.iter().position(|e| matches(e, old))).or_else(|| list.iter().position(|e| matches(e, address)));
    match index {
        Some(i) => {
            let entry = &mut list[i];
            if entry_ip(entry) == Some(address) && entry.get("name").and_then(Tag::as_str) == Some(name) {
                return false;
            }
            entry.set("ip", Tag::String(address.into()));
            entry.set("name", Tag::String(name.into()));
        }
        None => list.push(Tag::Compound(vec![("ip".into(), Tag::String(address.into())), ("name".into(), Tag::String(name.into()))])),
    }
    true
}

fn edit_dat(game_dir: &Path, change: impl FnOnce(&mut Vec<Tag>) -> bool) -> Result<()> {
    let path = game_dir.join("servers.dat");
    let mut root = match std::fs::read(&path) {
        Ok(bytes) => nbt::read(&bytes)?,
        Err(_) => Tag::Compound(Vec::new()),
    };
    if !matches!(root.get("servers"), Some(Tag::List(..))) {
        root.set("servers", Tag::List(10, Vec::new()));
    }
    let Some(Tag::List(_, list)) = root.get_mut("servers") else { return Ok(()) };
    if !change(list) {
        return Ok(());
    }
    std::fs::create_dir_all(game_dir)?;
    storage::write_bytes_atomic(&path, &nbt::write(&root))
}

pub fn sync_into(game_dir: &Path) {
    let servers = saved();
    if servers.is_empty() {
        return;
    }
    let _ = edit_dat(game_dir, |list| {
        let mut changed = false;
        for server in &servers {
            if !list.iter().any(|e| entry_ip(e).map(|ip| ip.eq_ignore_ascii_case(&server.address)).unwrap_or(false)) {
                list.push(Tag::Compound(vec![("ip".into(), Tag::String(server.address.clone())), ("name".into(), Tag::String(server.name.clone()))]));
                changed = true;
            }
        }
        changed
    });
}

fn now_millis() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub online: bool,
    pub motd: String,
    pub players_online: u32,
    pub players_max: u32,
    pub version: String,
    pub favicon: String,
    pub latency: u32,
    pub address: String,
    pub error: String,
}

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7F == 0 {
            buf.push(v as u8);
            return;
        }
        buf.push((v & 0x7F | 0x80) as u8);
        v >>= 7;
    }
}

async fn read_varint<R: AsyncRead + Unpin>(reader: &mut R) -> Result<i32> {
    let mut out = 0u32;
    for i in 0..5 {
        let byte = reader.read_u8().await?;
        out |= ((byte & 0x7F) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(out as i32);
        }
    }
    Err(Error::msg("Malformed server response"))
}

pub fn parse_address(address: &str) -> (String, Option<u16>) {
    let address = address.trim();
    if let Some(rest) = address.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            return (host.to_string(), tail.strip_prefix(':').and_then(|p| p.parse().ok()));
        }
    }
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host.to_string(), port.parse().ok()),
        _ => (address.to_string(), None),
    }
}

static RESOLVER: OnceLock<Option<hickory_resolver::TokioResolver>> = OnceLock::new();

async fn resolve_srv(host: &str) -> Option<(String, u16)> {
    if host.parse::<std::net::IpAddr>().is_ok() || host.eq_ignore_ascii_case("localhost") || !host.contains('.') {
        return None;
    }
    let resolver = RESOLVER.get_or_init(|| hickory_resolver::TokioResolver::builder_tokio().ok().map(|b| b.build())).as_ref()?;
    let lookup = tokio::time::timeout(Duration::from_secs(3), resolver.srv_lookup(format!("_minecraft._tcp.{host}."))).await.ok()?.ok()?;
    let record = lookup.iter().min_by_key(|r| (r.priority(), std::cmp::Reverse(r.weight())))?;
    let target = record.target().to_utf8().trim_end_matches('.').to_string();
    (!target.is_empty()).then(|| (target, record.port()))
}

pub async fn resolve(address: &str) -> (String, u16) {
    let (host, port) = parse_address(address);
    match port {
        Some(port) => (host, port),
        None => resolve_srv(&host).await.unwrap_or((host, 25565)),
    }
}

fn flatten_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items.iter().map(flatten_text).collect(),
        serde_json::Value::Object(map) => {
            let mut out = map.get("text").map(flatten_text).unwrap_or_default();
            if let Some(extra) = map.get("extra") {
                out.push_str(&flatten_text(extra));
            }
            out
        }
        _ => String::new(),
    }
}

pub fn strip_codes(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

async fn query_legacy(host: &str, port: u16) -> Result<ServerStatus> {
    let started = Instant::now();
    let mut stream = tokio::net::TcpStream::connect((host, port)).await?;
    let latency = started.elapsed().as_millis() as u32;
    stream.write_all(&[0xFE, 0x01]).await?;
    if stream.read_u8().await? != 0xFF {
        return Err(Error::msg("Unexpected legacy response"));
    }
    let chars = stream.read_u16().await? as usize;
    let mut raw = vec![0u8; chars * 2];
    stream.read_exact(&mut raw).await?;
    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    let text = String::from_utf16_lossy(&units);
    Ok(parse_legacy(&text, latency))
}

pub fn parse_legacy(text: &str, latency: u32) -> ServerStatus {
    let (motd, version, online, max) = if let Some(rest) = text.strip_prefix("\u{a7}1\0") {
        let parts: Vec<&str> = rest.split('\0').collect();
        (parts.get(2).copied().unwrap_or(""), parts.get(1).copied().unwrap_or(""), parts.get(3).copied().unwrap_or("0"), parts.get(4).copied().unwrap_or("0"))
    } else {
        let mut parts = text.rsplitn(3, '\u{a7}');
        let max = parts.next().unwrap_or("0");
        let online = parts.next().unwrap_or("0");
        (parts.next().unwrap_or(""), "", online, max)
    };
    ServerStatus {
        online: true,
        motd: strip_codes(motd).trim().to_string(),
        players_online: online.trim().parse().unwrap_or(0),
        players_max: max.trim().parse().unwrap_or(0),
        version: strip_codes(version),
        latency,
        ..Default::default()
    }
}

fn connect_error(e: &std::io::Error) -> String {
    let text = e.to_string();
    let lower = text.to_lowercase();
    let reason = if lower.contains("no such host") || lower.contains("11001") || lower.contains("failed to lookup") || lower.contains("not known") {
        "address not found"
    } else if lower.contains("refused") || lower.contains("10061") {
        "server is offline or the port is wrong"
    } else if lower.contains("timed out") || lower.contains("10060") {
        "timed out"
    } else {
        text.as_str()
    };
    format!("Could not connect: {reason}")
}

async fn query(host: &str, port: u16) -> Result<(serde_json::Value, u32)> {
    let started = Instant::now();
    let mut stream = tokio::net::TcpStream::connect((host, port)).await.map_err(|e| Error::msg(connect_error(&e)))?;
    let latency = started.elapsed().as_millis() as u32;
    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0);
    write_varint(&mut handshake, -1);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1);
    let mut packet = Vec::new();
    write_varint(&mut packet, handshake.len() as i32);
    packet.extend_from_slice(&handshake);
    packet.extend_from_slice(&[1, 0]);
    stream.write_all(&packet).await?;
    let _length = read_varint(&mut stream).await?;
    if read_varint(&mut stream).await? != 0 {
        return Err(Error::msg("Unexpected server response"));
    }
    let size = read_varint(&mut stream).await?;
    if !(0..=1_048_576).contains(&size) {
        return Err(Error::msg("Server response too large"));
    }
    let mut body = vec![0u8; size as usize];
    stream.read_exact(&mut body).await?;
    Ok((serde_json::from_slice(&body)?, latency))
}

pub async fn ping(address: &str) -> ServerStatus {
    let (host, port) = resolve(address).await;
    let shown = if port == 25565 { host.clone() } else { format!("{host}:{port}") };
    let mut status = match tokio::time::timeout(Duration::from_secs(6), query(&host, port)).await {
        Ok(Ok((json, latency))) => ServerStatus {
            online: true,
            motd: strip_codes(&flatten_text(&json["description"])).trim().to_string(),
            players_online: json["players"]["online"].as_u64().unwrap_or(0) as u32,
            players_max: json["players"]["max"].as_u64().unwrap_or(0) as u32,
            version: strip_codes(json["version"]["name"].as_str().unwrap_or("")),
            favicon: json["favicon"].as_str().filter(|f| f.starts_with("data:image/png;base64,")).unwrap_or("").to_string(),
            latency,
            ..Default::default()
        },
        Ok(Err(e)) if !e.0.starts_with("Could not connect") => match tokio::time::timeout(Duration::from_secs(4), query_legacy(&host, port)).await {
            Ok(Ok(status)) => status,
            _ => ServerStatus { error: e.0, ..Default::default() },
        },
        Ok(Err(e)) => ServerStatus { error: e.0, ..Default::default() },
        Err(_) => ServerStatus { error: "Timed out".into(), ..Default::default() },
    };
    status.address = shown;
    status
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct HostedServer {
    pub id: String,
    pub name: String,
    pub minecraft: String,
    pub memory: u32,
    pub port: u16,
    pub motd: String,
    pub created: u64,
    #[serde(skip_deserializing)]
    pub state: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewHostedServer {
    pub name: String,
    pub minecraft: String,
    pub memory: u32,
    pub port: u16,
    #[serde(default)]
    pub motd: String,
    pub accept_eula: bool,
}

struct Running {
    state: String,
    input: Option<mpsc::UnboundedSender<String>>,
    pid: Option<u32>,
}

static RUNNING: LazyLock<Mutex<HashMap<String, Running>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static LOGS: LazyLock<Mutex<HashMap<String, VecDeque<String>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn hosted_root() -> PathBuf {
    storage::root().join("hosted-servers")
}

fn hosted_dir(id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(Error::msg("Invalid server id"));
    }
    Ok(hosted_root().join(id))
}

fn state_of(id: &str) -> String {
    RUNNING.lock().ok().and_then(|r| r.get(id).map(|s| s.state.clone())).unwrap_or_else(|| "stopped".into())
}

fn load_hosted(id: &str) -> Result<HostedServer> {
    let mut server: HostedServer = storage::read_json(&hosted_dir(id)?.join(CONFIG_FILE))?.ok_or_else(|| Error::msg("Server not found"))?;
    server.state = state_of(&server.id);
    Ok(server)
}

pub fn hosted_list() -> Vec<HostedServer> {
    let mut out: Vec<HostedServer> = std::fs::read_dir(hosted_root())
        .map(|entries| entries.flatten().filter_map(|e| load_hosted(&e.file_name().to_string_lossy()).ok()).collect())
        .unwrap_or_default();
    out.sort_by_key(|s| s.created);
    out
}

fn properties(server: &HostedServer) -> String {
    let motd: String = server.motd.chars().filter(|c| !c.is_control()).take(59).collect();
    format!("server-port={}\nmotd={}\nonline-mode=true\nenable-status=true\nmax-players=10\nview-distance=10\nsimulation-distance=8\nspawn-protection=0\n", server.port, if motd.is_empty() { "A Zip server".into() } else { motd })
}

pub fn hosted_create(request: NewHostedServer) -> Result<HostedServer> {
    if !request.accept_eula {
        return Err(Error::msg("You must accept the Minecraft EULA to host a server"));
    }
    let name = crate::instances::validate_name(&request.name)?;
    if request.minecraft.trim().is_empty() {
        return Err(Error::msg("Choose a Minecraft version"));
    }
    if request.port < 1024 {
        return Err(Error::msg("Use a port between 1024 and 65535"));
    }
    if hosted_list().iter().any(|s| s.port == request.port) {
        return Err(Error::msg(format!("Another server already uses port {}", request.port)));
    }
    let server = HostedServer {
        id: format!("srv-{}", now_millis()),
        name,
        minecraft: request.minecraft.trim().to_string(),
        memory: request.memory.clamp(1024, 32768),
        port: request.port,
        motd: request.motd.trim().to_string(),
        created: crate::instances::now(),
        state: "stopped".into(),
    };
    let dir = hosted_dir(&server.id)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("eula.txt"), "eula=true\n")?;
    std::fs::write(dir.join("server.properties"), properties(&server))?;
    storage::write_json(&dir.join(CONFIG_FILE), &server)?;
    Ok(server)
}

pub fn hosted_delete(id: &str) -> Result<()> {
    if state_of(id) != "stopped" {
        return Err(Error::msg("Stop the server before deleting it"));
    }
    let dir = hosted_dir(id)?;
    if dir.join(CONFIG_FILE).is_file() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn hosted_dir_path(id: &str) -> Result<PathBuf> {
    let dir = hosted_dir(id)?;
    if !dir.join(CONFIG_FILE).is_file() {
        return Err(Error::msg("Server not found"));
    }
    Ok(dir)
}

pub fn logs(id: &str) -> Vec<String> {
    LOGS.lock().ok().and_then(|l| l.get(id).map(|q| q.iter().cloned().collect())).unwrap_or_default()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StateEvent {
    id: String,
    state: String,
    message: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogEvent {
    id: String,
    lines: Vec<String>,
}

fn set_state(app: &AppHandle, id: &str, state: &str, message: Option<String>) {
    if let Ok(mut running) = RUNNING.lock() {
        if state == "stopped" {
            running.remove(id);
        } else if let Some(entry) = running.get_mut(id) {
            entry.state = state.into();
        }
    }
    let _ = app.emit("server-state", StateEvent { id: id.into(), state: state.into(), message });
}

fn push_logs(app: &AppHandle, id: &str, lines: Vec<String>) {
    if lines.is_empty() {
        return;
    }
    if let Ok(mut logs) = LOGS.lock() {
        let queue = logs.entry(id.to_string()).or_default();
        queue.extend(lines.iter().cloned());
        while queue.len() > LOG_LIMIT {
            queue.pop_front();
        }
    }
    let _ = app.emit("server-log", LogEvent { id: id.into(), lines });
}

pub async fn start(app: AppHandle, id: String) -> Result<()> {
    let server = load_hosted(&id)?;
    {
        let mut running = RUNNING.lock().map_err(|_| Error::msg("Server registry busy"))?;
        if running.contains_key(&id) {
            return Err(Error::msg("This server is already running"));
        }
        running.insert(id.clone(), Running { state: "preparing".into(), input: None, pid: None });
    }
    set_state(&app, &id, "preparing", None);
    match spawn(&app, &server).await {
        Ok(()) => Ok(()),
        Err(e) => {
            set_state(&app, &id, "stopped", Some(e.0.clone()));
            Err(e)
        }
    }
}

async fn spawn(app: &AppHandle, server: &HostedServer) -> Result<()> {
    let dir = hosted_dir(&server.id)?;
    let reporter = Reporter::new(app, format!("server:{}", server.id));
    let java_path = {
        let _lock = crate::game::INSTALL_LOCK.lock().await;
        reporter.phase(&format!("Downloading Minecraft {} server", server.minecraft));
        install::ensure_vanilla_json(&server.minecraft).await?;
        let meta = version::load_local(&storage::versions_dir(), &server.minecraft)?;
        let jar = meta.downloads.as_ref().and_then(|d| d.get("server")).ok_or_else(|| Error::msg(format!("Mojang publishes no server for Minecraft {}", server.minecraft)))?;
        net::download_all(vec![DownloadItem::new(jar.url.clone(), dir.join("server.jar"), jar.sha1.clone(), jar.size)], reporter.progress(), false).await?;
        reporter.phase("Preparing Java");
        java::select(meta.java_version.as_ref(), None, reporter.progress()).await?
    };
    reporter.phase("");
    drop(reporter);
    let _ = std::fs::write(dir.join("eula.txt"), "eula=true\n");
    let props = dir.join("server.properties");
    if let Ok(text) = std::fs::read_to_string(&props) {
        let updated: Vec<String> = text.lines().map(|l| if l.starts_with("server-port=") { format!("server-port={}", server.port) } else { l.to_string() }).collect();
        let _ = std::fs::write(&props, updated.join("\n") + "\n");
    } else {
        std::fs::write(&props, properties(server))?;
    }
    let mut command = tokio::process::Command::new(&java_path);
    command
        .args([format!("-Xms{}M", (server.memory / 2).max(512)), format!("-Xmx{}M", server.memory), "-jar".into(), "server.jar".into(), "nogui".into()])
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::platform::hide_console(&mut command);
    let mut child = command.spawn().map_err(|e| Error::msg(format!("Could not start the server: {e}")))?;
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<String>();
    if let Some(mut stdin) = child.stdin.take() {
        tauri::async_runtime::spawn(async move {
            while let Some(line) = input_rx.recv().await {
                if stdin.write_all(format!("{line}\n").as_bytes()).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });
    }
    if let Ok(mut running) = RUNNING.lock() {
        if let Some(entry) = running.get_mut(&server.id) {
            entry.input = Some(input_tx);
            entry.pid = child.id();
        }
    }
    if let Ok(mut logs) = LOGS.lock() {
        logs.insert(server.id.clone(), VecDeque::new());
    }
    set_state(app, &server.id, "starting", None);
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>)].into_iter().flatten() {
        let tx = line_tx.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    drop(line_tx);
    let app = app.clone();
    let id = server.id.clone();
    tauri::async_runtime::spawn(async move {
        let mut batch = Vec::new();
        let mut ticker = tokio::time::interval(Duration::from_millis(250));
        let mut ready = false;
        loop {
            tokio::select! {
                line = line_rx.recv() => match line {
                    Some(line) => {
                        if !ready && line.contains("Done (") && line.contains("For help") {
                            ready = true;
                            set_state(&app, &id, "running", None);
                        }
                        batch.push(line);
                    }
                    None => break,
                },
                _ = ticker.tick() => push_logs(&app, &id, std::mem::take(&mut batch)),
            }
        }
        push_logs(&app, &id, batch);
        let status = child.wait().await.ok().and_then(|s| s.code());
        let message = status.filter(|c| *c != 0).map(|c| format!("The server stopped with exit code {c}"));
        set_state(&app, &id, "stopped", message);
    });
    Ok(())
}

pub fn command(id: &str, line: &str) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    let running = RUNNING.lock().map_err(|_| Error::msg("Server registry busy"))?;
    let input = running.get(id).and_then(|r| r.input.clone()).ok_or_else(|| Error::msg("This server is not running"))?;
    input.send(line.trim_start_matches('/').to_string()).map_err(|_| Error::msg("The server console is closed"))
}

pub fn stop(app: &AppHandle, id: &str) -> Result<()> {
    command(id, "stop")?;
    set_state(app, id, "stopping", None);
    let pid = RUNNING.lock().ok().and_then(|r| r.get(id).and_then(|e| e.pid));
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(45)).await;
        let still = RUNNING.lock().map(|r| r.get(&id).and_then(|e| e.pid) == pid && pid.is_some()).unwrap_or(false);
        if still {
            if let Some(pid) = pid {
                force_kill(pid);
            }
        }
    });
    Ok(())
}

fn force_kill(pid: u32) {
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/T", "/F"]);
        c
    } else {
        let mut c = std::process::Command::new("kill");
        c.args(["-9", &pid.to_string()]);
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let _ = command.status();
}

pub fn running_count() -> usize {
    RUNNING.lock().map(|r| r.len()).unwrap_or(0)
}

pub fn shutdown() {
    let targets: Vec<(Option<mpsc::UnboundedSender<String>>, Option<u32>)> = RUNNING.lock().map(|r| r.values().map(|e| (e.input.clone(), e.pid)).collect()).unwrap_or_default();
    if targets.is_empty() {
        return;
    }
    for (input, _) in &targets {
        if let Some(input) = input {
            let _ = input.send("stop".into());
        }
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while running_count() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    for (_, pid) in targets {
        if let Some(pid) = pid {
            if RUNNING.lock().map(|r| r.values().any(|e| e.pid == Some(pid))).unwrap_or(false) {
                force_kill(pid);
            }
        }
    }
}

pub fn lan_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 300);
        assert_eq!(buf, vec![0xAC, 0x02]);
        buf.clear();
        write_varint(&mut buf, -1);
        assert_eq!(buf, vec![0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[tokio::test]
    async fn reads_varint() {
        let data: &[u8] = &[0xAC, 0x02];
        let mut reader = data;
        assert_eq!(read_varint(&mut reader).await.unwrap(), 300);
    }

    #[test]
    fn addresses() {
        assert_eq!(parse_address("play.example.com"), ("play.example.com".into(), None));
        assert_eq!(parse_address("1.2.3.4:25570"), ("1.2.3.4".into(), Some(25570)));
        assert_eq!(parse_address("[::1]:25566"), ("::1".into(), Some(25566)));
        assert!(valid_address("mc.host-1.net:25565"));
        assert!(!valid_address("bad host"));
    }

    #[test]
    fn legacy_status() {
        let s = parse_legacy("\u{a7}1\0127\01.6.4\0A \u{a7}aMinecraft Server\05\020", 9);
        assert_eq!((s.motd.as_str(), s.version.as_str(), s.players_online, s.players_max), ("A Minecraft Server", "1.6.4", 5, 20));
        let s = parse_legacy("Old server\u{a7}3\u{a7}10", 1);
        assert_eq!((s.motd.as_str(), s.players_online, s.players_max), ("Old server", 3, 10));
        assert_eq!(parse_address("mc.example.com"), ("mc.example.com".into(), None));
        assert_eq!(parse_address("mc.example.com:25570"), ("mc.example.com".into(), Some(25570)));
    }

    #[test]
    fn servers_dat_upsert() {
        let dir = tempfile::tempdir().unwrap();
        edit_dat(dir.path(), |list| upsert(list, None, "One", "one.example.com")).unwrap();
        edit_dat(dir.path(), |list| upsert(list, None, "Two", "two.example.com")).unwrap();
        edit_dat(dir.path(), |list| upsert(list, Some("one.example.com"), "Uno", "uno.example.com")).unwrap();
        edit_dat(dir.path(), |list| upsert(list, None, "Two", "two.example.com")).unwrap();
        let root = nbt::read(&std::fs::read(dir.path().join("servers.dat")).unwrap()).unwrap();
        let Some(Tag::List(_, list)) = root.get("servers") else { panic!() };
        let ips: Vec<&str> = list.iter().filter_map(entry_ip).collect();
        assert_eq!(ips, vec!["uno.example.com", "two.example.com"]);
    }

    #[test]
    fn motd_text() {
        let v = serde_json::json!({"text": "§aHello ", "extra": [{"text": "World"}, "!"]});
        assert_eq!(strip_codes(&flatten_text(&v)), "Hello World!");
    }
}
