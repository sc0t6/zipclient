import { invoke } from "@tauri-apps/api/core";

export type Loader = "vanilla" | "fabric" | "forge" | "neoforge" | "quilt" | "horizon";
export type ContentKind = "mod" | "resourcepack" | "shader" | "modpack";

export interface Settings {
  schema: number;
  setup: { completed: boolean; step: number; legal: string };
  general: {
    language: string;
    startup_page: string;
    after_launch: string;
    check_updates: boolean;
    update_endpoint: string;
    skipped_update: string;
    notifications: boolean;
    confirm_exit: boolean;
    instances_dir: string;
  };
  appearance: {
    theme: string;
    accent: string;
    scale: number;
    animations: string;
    background: string;
    background_dim: number;
    follow_system_motion: boolean;
    nav: "top" | "side" | "icons";
    ui: Record<string, unknown>;
  };
  minecraft: {
    java_mode: string;
    java_path: string;
    memory_min: number;
    memory_max: number;
    jvm_args: string;
    width: number;
    height: number;
    game_dir: string;
    default_account: string;
  };
  downloads: { concurrency: number; retries: number; timeout_secs: number; cache_limit_mb: number };
  advanced: { debug_logging: boolean; last_page: string };
}

export interface AppInfo {
  version: string;
  platform: string;
  portable: boolean;
  dataDir: string;
  msConfigured: boolean;
  curseforgeConfigured: boolean;
}

export interface PlaySession {
  started: number;
  duration: number;
  exit_code: number | null;
}

export interface LaunchLog {
  found: boolean;
  path: string;
  lines: string[];
  summary: string;
}

export interface Screenshot {
  name: string;
  path: string;
  size: number;
  modified: number;
}

export interface World {
  folder: string;
  name: string;
  lastPlayed: number;
  mode: string;
  version: string;
  hardcore: boolean;
  icon: string | null;
  datapacks: number;
}

export interface Datapack {
  name: string;
  size: number;
  folder: boolean;
}

export interface Cape {
  id: string;
  alias: string;
  url: string;
  active: boolean;
}

export interface Instance {
  id: string;
  name: string;
  icon: string;
  group: string;
  minecraft: string;
  loader: Loader;
  loader_version: string;
  version_id: string;
  java_path: string;
  memory_min: number;
  memory_max: number;
  width: number;
  height: number;
  jvm_args: string;
  game_dir: string;
  account_id: string;
  zip_client: boolean;
  memory_warning: boolean;
  fast_launch: boolean;
  checked: boolean;
  created: number;
  last_played: number;
  playtime: number;
  launches: number;
  history: PlaySession[];
  modpack: { source: string; name: string; version: string; project_id: string } | null;
}

export interface InstancePatch {
  name?: string;
  group?: string;
  minecraft?: string;
  loader?: Loader;
  loaderVersion?: string;
  javaPath?: string;
  memoryMin?: number;
  memoryMax?: number;
  width?: number;
  height?: number;
  jvmArgs?: string;
  gameDir?: string;
  accountId?: string;
  zipClient?: boolean;
  memoryWarning?: boolean;
  fastLaunch?: boolean;
  icon?: string;
}

export interface ManifestVersion {
  id: string;
  type: string;
  url: string;
  releaseTime: string;
}

export interface VersionList {
  latestRelease: string;
  latestSnapshot: string;
  versions: ManifestVersion[];
}

export interface LoaderVersion {
  version: string;
  minecraft: string;
  stable: boolean;
  recommended: boolean;
}

export interface JavaInstall {
  path: string;
  version: string;
  major: number;
  vendor: string;
  source: string;
}

export interface Account {
  id: string;
  name: string;
  kind: "microsoft" | "offline";
  skinUrl: string | null;
  skinVariant: string | null;
  active: boolean;
  needsRelogin: boolean;
}

export interface DeviceCodeInfo {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
}

export interface GameState {
  instanceId: string;
  state: string;
  message: string | null;
  exitCode: number | null;
  crashReport: string | null;
  logFile: string | null;
}

export interface ProgressEvent {
  key: string;
  phase: string;
  fraction: number;
  doneFiles: number;
  totalFiles: number;
  doneBytes: number;
  totalBytes: number;
}

export interface SearchHit {
  source: string;
  id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  iconUrl: string;
  downloads: number;
  categories: string[];
  latestVersion: string;
  installable: boolean;
  note: string;
}

export interface ContentVersion {
  id: string;
  name: string;
  versionNumber: string;
  gameVersions: string[];
  loaders: string[];
  date: string;
  fileName: string;
  url: string;
  sha1: string;
  size: number;
  channel: string;
}

export interface ProjectDetails {
  title: string;
  summary: string;
  body: string;
  html: boolean;
  iconUrl: string;
  downloads: number;
  categories: string[];
  gallery: { url: string; title: string }[];
  updated: string;
}

export interface InstalledFile {
  path: string;
  fileName: string;
  kind: string;
  enabled: boolean;
  size: number;
  title: string;
  version: string;
  projectId: string;
  versionId: string;
  source: string;
  iconUrl: string;
}

export interface UpdateInfo {
  path: string;
  projectId: string;
  current: string;
  latest: string;
  versionId: string;
}

export interface SkinEntry {
  id: string;
  name: string;
  variant: string;
  dataUrl: string;
}

export interface NewsItem {
  title: string;
  text: string;
  date: string;
  url: string;
  image: string;
}

export interface OptimizePreset {
  memoryMin: number;
  memoryMax: number;
  jvmArgs: string;
  javaMajor: number;
  systemMb: number;
}

export interface UpdateStatus {
  configured: boolean;
  available: boolean;
  current: string;
  version: string | null;
  notes: string | null;
  portable: boolean;
  skipped: boolean;
  error: string | null;
}

export interface ZipClientStatus {
  installed: boolean;
  available: boolean;
  version: string | null;
  label: string;
}

export interface ImportResult {
  instance: Instance;
  warnings: string[];
}

export interface SavedServer {
  id: string;
  name: string;
  address: string;
  instanceId: string;
  added: number;
}

export interface ServerStatus {
  online: boolean;
  motd: string;
  playersOnline: number;
  playersMax: number;
  version: string;
  favicon: string;
  latency: number;
  address: string;
  error: string;
}

export interface HostedServer {
  id: string;
  name: string;
  minecraft: string;
  memory: number;
  port: number;
  motd: string;
  created: number;
  state: "stopped" | "preparing" | "starting" | "running" | "stopping";
}

export interface SystemInfo {
  launcherVersion: string;
  os: string;
  arch: string;
  dataDir: string;
  gameDir: string;
  instancesDir: string;
  portable: boolean;
  cacheBytes: number;
  java: JavaInstall[];
  instanceCount: number;
  msAuthConfigured: boolean;
  curseforgeConfigured: boolean;
  updateSourceConfigured: boolean;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  exit: () => invoke<void>("app_exit"),
  settings: () => invoke<Settings>("settings_get"),
  saveSettings: (settings: Settings) => invoke<Settings>("settings_save", { settings }),
  exportSettings: (path: string) => invoke<void>("settings_export", { path }),
  importSettings: (path: string) => invoke<Settings>("settings_import", { path }),
  resetSettings: () => invoke<Settings>("settings_reset"),
  versions: (force = false) => invoke<VersionList>("minecraft_versions", { force }),
  loaderVersions: (loader: Loader, minecraft: string) => invoke<LoaderVersion[]>("loader_versions", { loader, minecraft }),
  loaderSupported: (loader: Loader) => invoke<string[]>("loader_supported", { loader }),
  javaDetect: () => invoke<JavaInstall[]>("java_detect"),
  javaCheck: (path: string) => invoke<JavaInstall>("java_check", { path }),
  javaInstallRuntime: (component: string) => invoke<string>("java_install_runtime", { component }),
  instances: () => invoke<Instance[]>("instances_list"),
  instance: (id: string) => invoke<Instance>("instance_get", { id }),
  createInstance: (request: { name: string; minecraft: string; loader: Loader; loaderVersion: string; group: string }) =>
    invoke<Instance>("instance_create", { request }),
  updateInstance: (id: string, patch: InstancePatch) => invoke<Instance>("instance_update", { id, patch }),
  duplicateInstance: (id: string) => invoke<Instance>("instance_duplicate", { id }),
  deleteInstance: (id: string) => invoke<void>("instance_delete", { id }),
  setInstanceIcon: (id: string, path: string | null) => invoke<Instance>("instance_set_icon", { id, path }),
  instanceIcon: (id: string) => invoke<string | null>("instance_icon", { id }),
  exportInstance: (id: string, path: string) => invoke<void>("instance_export", { id, path }),
  importInstance: (path: string) => invoke<ImportResult>("instance_import", { path }),
  installModpack: (projectId: string, versionId: string | null) => invoke<ImportResult>("modpack_install", { projectId, versionId }),
  openInstanceFolder: (id: string, sub?: string) => invoke<void>("instance_open_folder", { id, sub: sub ?? null }),
  openFolder: (kind: "data" | "logs" | "instances" | "game" | "zipclient") => invoke<void>("open_folder", { kind }),
  openFile: (path: string) => invoke<void>("open_file", { path }),
  installInstance: (id: string, verify: boolean) => invoke<Instance>("instance_install", { id, verify }),
  launch: (id: string, server: string | null = null) => invoke<void>("game_launch", { id, server }),
  kill: (id: string) => invoke<void>("game_kill", { id }),
  gameStates: () => invoke<GameState[]>("game_states"),
  gameLogs: (id: string) => invoke<string[]>("game_logs", { id }),
  accounts: () => invoke<Account[]>("accounts_list"),
  addOffline: (name: string) => invoke<Account>("account_add_offline", { name }),
  beginMicrosoft: () => invoke<DeviceCodeInfo>("account_begin_microsoft"),
  cancelMicrosoft: () => invoke<void>("account_cancel_microsoft"),
  setActiveAccount: (id: string) => invoke<void>("account_set_active", { id }),
  removeAccount: (id: string) => invoke<void>("account_remove", { id }),
  refreshAccount: (id: string) => invoke<Account[]>("account_refresh", { id }),
  search: (query: {
    source: string;
    kind: ContentKind;
    query: string;
    minecraft: string;
    loader: string;
    category: string;
    sort: string;
    offset: number;
    limit: number;
  }) => invoke<{ hits: SearchHit[]; total: number }>("content_search", { query }),
  categories: (source: string, kind: ContentKind) => invoke<{ id: string; name: string }[]>("content_categories", { source, kind }),
  contentVersions: (source: string, projectId: string, kind: ContentKind, minecraft: string, loader: string) =>
    invoke<ContentVersion[]>("content_versions", { source, projectId, kind, minecraft, loader }),
  contentProject: (source: string, projectId: string) => invoke<ProjectDetails>("content_project", { source, projectId }),
  uiBackground: () => invoke<string | null>("ui_background_get"),
  setUiBackground: (path: string) => invoke<string>("ui_background_set", { path }),
  clearUiBackground: () => invoke<void>("ui_background_clear"),
  installContent: (instanceId: string, source: string, projectId: string, versionId: string | null, kind: ContentKind) =>
    invoke<{ installed: string[]; skipped: string[] }>("content_install", { instanceId, source, projectId, versionId, kind }),
  listContent: (instanceId: string, kind: ContentKind, identify: boolean) => invoke<InstalledFile[]>("content_list", { instanceId, kind, identify }),
  toggleContent: (instanceId: string, path: string, enabled: boolean) => invoke<string>("content_toggle", { instanceId, path, enabled }),
  removeContent: (instanceId: string, path: string) => invoke<void>("content_remove", { instanceId, path }),
  importContent: (instanceId: string, kind: ContentKind, paths: string[]) => invoke<string[]>("content_import", { instanceId, kind, paths }),
  contentUpdates: (instanceId: string, kind: ContentKind) => invoke<UpdateInfo[]>("content_updates", { instanceId, kind }),
  updateContent: (instanceId: string, path: string, versionId: string) => invoke<string[]>("content_update", { instanceId, path, versionId }),
  skins: () => invoke<{ skins: SkinEntry[]; selected: Record<string, string> }>("skins_library"),
  importSkin: (path: string, name: string, variant: string) => invoke<SkinEntry>("skin_import", { path, name, variant }),
  removeSkin: (id: string) => invoke<void>("skin_remove", { id }),
  setSkinVariant: (id: string, variant: string) => invoke<void>("skin_set_variant", { id, variant }),
  selectLocalSkin: (accountId: string, skinId: string | null) => invoke<void>("skin_select_local", { accountId, skinId }),
  skinTexture: (url: string) => invoke<string>("skin_texture", { url }),
  defaultSkin: (variant: string) => invoke<string>("skin_default", { variant }),
  uploadSkin: (accountId: string, skinId: string) => invoke<Account[]>("skin_upload", { accountId, skinId }),
  launchLog: (id: string, started: number, crashed: boolean) => invoke<LaunchLog>("launch_log", { id, started, crashed }),
  screenshots: (id: string) => invoke<Screenshot[]>("screenshots_list", { id }),
  deleteScreenshot: (id: string, name: string) => invoke<void>("screenshot_delete", { id, name }),
  worlds: (id: string) => invoke<World[]>("worlds_list", { id }),
  datapacks: (id: string, world: string) => invoke<Datapack[]>("datapacks_list", { id, world }),
  importDatapacks: (id: string, world: string, paths: string[]) => invoke<number>("datapack_import", { id, world, paths }),
  removeDatapack: (id: string, world: string, name: string) => invoke<void>("datapack_remove", { id, world, name }),
  capes: (accountId: string) => invoke<Cape[]>("account_capes", { accountId }),
  setCape: (accountId: string, capeId: string | null) => invoke<Account[]>("account_set_cape", { accountId, capeId }),
  news: (source: "zip" | "minecraft") => invoke<NewsItem[]>("news_list", { source }),
  optimizePreset: (minecraft: string, loader: Loader) => invoke<OptimizePreset>("optimize_preset", { minecraft, loader }),
  checkUpdate: () => invoke<UpdateStatus>("update_check"),
  installUpdate: () => invoke<void>("update_install"),
  zipClientStatus: (minecraft: string, loader: string) => invoke<ZipClientStatus>("zipclient_status", { minecraft, loader }),
  cacheInfo: () => invoke<number>("cache_info"),
  clearCache: () => invoke<void>("cache_clear"),
  diagnostics: () => invoke<SystemInfo>("diagnostics_info"),
  logError: (message: string) => invoke<void>("log_frontend_error", { message }),
  savedServers: () => invoke<SavedServer[]>("servers_saved"),
  saveServer: (server: Partial<SavedServer> & { name: string; address: string }) =>
    invoke<SavedServer>("server_save", { server: { id: "", instanceId: "", added: 0, ...server } }),
  removeServer: (id: string) => invoke<void>("server_remove", { id }),
  pingServer: (address: string) => invoke<ServerStatus>("server_ping", { address }),
  hostedServers: () => invoke<HostedServer[]>("hosted_list"),
  createHosted: (request: { name: string; minecraft: string; memory: number; port: number; motd: string; acceptEula: boolean }) =>
    invoke<HostedServer>("hosted_create", { request }),
  deleteHosted: (id: string) => invoke<void>("hosted_delete", { id }),
  startHosted: (id: string) => invoke<void>("hosted_start", { id }),
  stopHosted: (id: string) => invoke<void>("hosted_stop", { id }),
  hostedCommand: (id: string, line: string) => invoke<void>("hosted_command", { id, line }),
  hostedLogs: (id: string) => invoke<string[]>("hosted_logs", { id }),
  openHostedFolder: (id: string) => invoke<void>("hosted_open_folder", { id }),
  lanAddress: () => invoke<string | null>("lan_address"),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

export function formatPlaytime(seconds: number): string {
  if (!seconds) return "0m";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h === 0) return `${Math.max(m, 1)}m`;
  return `${h}h ${m}m`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function rangeFill(value: number, min: number, max: number): { "--fill": string } {
  const span = max - min;
  const pct = span > 0 ? Math.min(100, Math.max(0, ((value - min) / span) * 100)) : 0;
  return { "--fill": `${pct}%` };
}

export const loaderNames: Record<Loader, string> = {
  vanilla: "Vanilla",
  fabric: "Fabric",
  forge: "Forge",
  neoforge: "NeoForge",
  quilt: "Quilt",
  horizon: "Horizon",
};
