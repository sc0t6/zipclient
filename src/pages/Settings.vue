<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import Dropdown from "../components/Dropdown.vue";
import Modal from "../components/Modal.vue";
import Toggle from "../components/Toggle.vue";
import UiEditor from "../components/UiEditor.vue";
import { api, errorText, formatBytes, rangeFill, type JavaInstall, type SystemInfo } from "../api";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";
import { useSettings } from "../stores/settings";
import { themes } from "../themes";
import { languages } from "../i18n";

defineOptions({ name: "SettingsPage" });

const app = useApp();
const notify = useNotify();
const store = useSettings();
const s = computed(() => store.s!);

const sections = ["General", "Appearance", "Minecraft", "Downloads", "Advanced", "About"] as const;
const section = ref<(typeof sections)[number]>("General");
const java = ref<JavaInstall[]>([]);
const javaLoading = ref(false);
const cacheBytes = ref(0);
const diagnostics = ref<SystemInfo | null>(null);
const resetOpen = ref(false);
const editorOpen = ref(false);
const installingUpdate = ref(false);
const runtimeBusy = ref("");
const accents = ["#7fcf8b", "#6fa8ff", "#c08cff", "#ff8fb1", "#ffb86b", "#ffe066", "#5fd3c6", "#e0e0e0"];

const accountOptions = computed(() => app.accounts.map((a) => ({ value: a.id, label: a.name, hint: a.kind === "microsoft" ? "Microsoft" : "offline" })));
const activeAccount = computed({
  get: () => app.activeAccount?.id ?? "",
  set: async (id: string) => {
    await api.setActiveAccount(id);
    await app.refreshAccounts();
    s.value.minecraft.default_account = id;
  },
});
const runtimes = [
  { id: "jre-legacy", label: "Java 8 (Minecraft ≤ 1.16)" },
  { id: "java-runtime-gamma", label: "Java 17 (1.17 – 1.20.4)" },
  { id: "java-runtime-delta", label: "Java 21 (1.20.5 – 1.21.x)" },
  { id: "java-runtime-epsilon", label: "Java 25 (26.x and newer)" },
];

async function loadJava() {
  javaLoading.value = true;
  java.value = await api.javaDetect().catch(() => []);
  javaLoading.value = false;
}

async function pickFolder(target: "instances" | "game") {
  const path = await open({ directory: true, multiple: false });
  if (typeof path !== "string") return;
  if (target === "instances") s.value.general.instances_dir = path;
  else s.value.minecraft.game_dir = path;
  await store.flush();
  await app.refreshInstances();
  notify.info("Folder changed", "Existing files were not moved. Copy them yourself if needed.");
}

async function pickJava() {
  const path = await open({ multiple: false, filters: [{ name: "Java", extensions: ["exe"] }] });
  if (typeof path !== "string") return;
  try {
    const info = await api.javaCheck(path);
    s.value.minecraft.java_path = path;
    s.value.minecraft.java_mode = "custom";
    notify.success("Java selected", `Java ${info.major} · ${info.vendor}`);
  } catch (e) {
    notify.error("Not a Java installation", errorText(e));
  }
}

async function installRuntime(id: string) {
  runtimeBusy.value = id;
  try {
    await api.javaInstallRuntime(id);
    notify.success("Java runtime ready", id);
    await loadJava();
  } catch (e) {
    notify.error("Runtime download failed", errorText(e));
  } finally {
    runtimeBusy.value = "";
  }
}

async function clearCache() {
  try {
    await api.clearCache();
    cacheBytes.value = await api.cacheInfo();
    notify.success("Cache cleared");
  } catch (e) {
    notify.error("Could not clear cache", errorText(e));
  }
}

async function exportSettings() {
  const path = await save({ defaultPath: "zip-settings.json", filters: [{ name: "JSON", extensions: ["json"] }] });
  if (!path) return;
  try {
    await store.flush();
    await api.exportSettings(path);
    notify.success("Settings exported", path);
  } catch (e) {
    notify.error("Export failed", errorText(e));
  }
}

async function importSettings() {
  const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
  if (typeof path !== "string") return;
  try {
    store.replace(await api.importSettings(path));
    notify.success("Settings imported");
  } catch (e) {
    notify.error("Import failed", errorText(e));
  }
}

async function resetSettings() {
  resetOpen.value = false;
  try {
    store.replace(await api.resetSettings());
    notify.success("Settings reset to defaults");
  } catch (e) {
    notify.error("Reset failed", errorText(e));
  }
}

async function loadDiagnostics() {
  diagnostics.value = await api.diagnostics().catch(() => null);
}

function copyDiagnostics() {
  if (!diagnostics.value) return;
  navigator.clipboard.writeText(JSON.stringify(diagnostics.value, null, 2)).then(() => notify.success("Diagnostics copied"));
}

async function installUpdate() {
  installingUpdate.value = true;
  try {
    await api.installUpdate();
  } catch (e) {
    notify.error("Update failed", errorText(e));
  } finally {
    installingUpdate.value = false;
  }
}

function skipUpdate() {
  if (app.update?.version) {
    s.value.general.skipped_update = app.update.version;
    app.update = { ...app.update, skipped: true };
  }
}

function rerunSetup() {
  s.value.setup.completed = false;
  s.value.setup.step = 1;
}

watch(section, (next) => {
  if (next === "Minecraft" && !java.value.length) loadJava();
  if (next === "Downloads") api.cacheInfo().then((b) => (cacheBytes.value = b));
  if (next === "Advanced" || next === "About") loadDiagnostics();
});

onMounted(() => {
  if (section.value === "Minecraft") loadJava();
});
</script>

<template>
  <div class="settings">
    <nav class="cats panel">
      <button v-for="name in sections" :key="name" class="cat" :class="{ on: section === name }" @click="section = name">{{ name }}</button>
      <div class="ver">Zip {{ app.info?.version }}{{ app.info?.portable ? " · portable" : "" }}</div>
    </nav>
    <Transition name="page" mode="out-in">
      <div :key="section" class="body panel scroll">
        <template v-if="section === 'General'">
          <h2>General</h2>
          <div class="opt">
            <div><b>Language</b><small>Applies to the launcher interface.</small></div>
            <Dropdown v-model="s.general.language" class="w180" :options="languages" />
          </div>
          <div class="opt">
            <div><b>Startup page</b><small>Which page opens when Zip starts.</small></div>
            <Dropdown v-model="s.general.startup_page" class="w180" :options="[{ value: 'home', label: 'Home' }, { value: 'last', label: 'Last opened page' }]" />
          </div>
          <div class="opt">
            <div><b>When Minecraft starts</b><small>What Zip does once the game window is up.</small></div>
            <Dropdown
              v-model="s.general.after_launch"
              class="w180"
              :options="[
                { value: 'keep', label: 'Keep Zip open' },
                { value: 'minimize', label: 'Minimize Zip' },
                { value: 'close', label: 'Hide until game closes' },
              ]"
            />
          </div>
          <div class="opt">
            <div><b>Notifications</b><small>Show pop-up notifications. Errors are always shown.</small></div>
            <Toggle v-model="s.general.notifications" label="Notifications" />
          </div>
          <div class="opt">
            <div><b>Confirm before closing</b><small>Ask before closing the launcher.</small></div>
            <Toggle v-model="s.general.confirm_exit" label="Confirm before closing" />
          </div>
          <div class="opt col">
            <div><b>Instances folder</b><small>Where instances are stored. Existing instances are not moved automatically.</small></div>
            <div class="row">
              <input class="field grow" :value="s.general.instances_dir || 'Default (inside Zip data folder)'" readonly />
              <button class="btn" @click="pickFolder('instances')">Browse</button>
              <button v-if="s.general.instances_dir" class="btn ghost" @click="s.general.instances_dir = ''; store.flush().then(app.refreshInstances)">Reset</button>
            </div>
          </div>
          <h3>Updates</h3>
          <div class="opt">
            <div><b>Check for updates on startup</b><small>Only signed updates from the configured source are accepted.</small></div>
            <Toggle v-model="s.general.check_updates" label="Check for updates" />
          </div>
          <div class="opt col">
            <div><b>Zip {{ app.info?.version }}</b><small>Updates come from the official Zip releases on GitHub. Only packages signed with Zip's release key are installed.</small></div>
            <div class="row">
              <button class="btn" :disabled="app.updateChecking" @click="app.checkUpdate(true)">
                <span v-if="app.updateChecking" class="spinner" />
                Check for updates
              </button>
            </div>
            <div v-if="app.update?.available" class="update-box">
              <div>
                <b>Zip {{ app.update.version }} is available</b>
                <small v-if="app.update.notes" class="notes">{{ app.update.notes }}</small>
              </div>
              <div class="row">
                <button class="btn ghost" :disabled="app.update.skipped" @click="skipUpdate">{{ app.update.skipped ? "Skipped" : "Skip" }}</button>
                <button class="btn primary" :disabled="installingUpdate || app.update.portable" :title="app.update.portable ? 'Portable builds update by downloading the new portable exe' : ''" @click="installUpdate">
                  <span v-if="installingUpdate" class="spinner" />
                  Install and restart
                </button>
              </div>
            </div>
            <small v-else-if="app.update?.error" class="warn-text">{{ app.update.error }}</small>
          </div>
        </template>

        <template v-else-if="section === 'Appearance'">
          <h2>Appearance</h2>
          <div class="opt edit-ui">
            <div><b>Edit UI</b><small>Change the font, text size, background, corners and the layout of each page.</small></div>
            <button class="btn primary" @click="editorOpen = true">Edit UI</button>
          </div>
          <div class="opt col">
            <div><b>Theme</b></div>
            <div class="themes">
              <button v-for="t in themes" :key="t.id" class="theme" :class="{ on: s.appearance.theme === t.id }" @click="s.appearance.theme = t.id">
                <span class="preview" :style="{ background: t.bg }"><span :style="{ background: t.panel }" /><i :style="{ background: s.appearance.accent }" /></span>
                <span>{{ t.label }}</span>
              </button>
            </div>
          </div>
          <div class="opt">
            <div><b>Accent color</b><small>Used for primary buttons, toggles and highlights.</small></div>
            <div class="swatches">
              <button v-for="c in accents" :key="c" class="swatch" :class="{ on: s.appearance.accent === c }" :style="{ background: c }" :title="c" @click="s.appearance.accent = c" />
              <label class="swatch custom" title="Custom color"><input v-model="s.appearance.accent" type="color" /></label>
            </div>
          </div>
          <div class="opt">
            <div><b>Navigation</b><small>Show the main tabs across the top or down the left side.</small></div>
            <div class="seg">
              <button :class="{ on: s.appearance.nav === 'top' }" @click="s.appearance.nav = 'top'">Top bar</button>
              <button :class="{ on: s.appearance.nav === 'side' }" @click="s.appearance.nav = 'side'">Sidebar</button>
              <button :class="{ on: s.appearance.nav === 'icons' }" @click="s.appearance.nav = 'icons'">Sidebar (No Name)</button>
            </div>
          </div>
          <div class="opt">
            <div><b>UI scale</b><small>{{ Math.round(s.appearance.scale * 100) }}%</small></div>
            <input v-model.number="s.appearance.scale" class="range" type="range" min="0.75" max="1.5" step="0.05" :style="rangeFill(s.appearance.scale, 0.75, 1.5)" />
          </div>
          <div class="opt">
            <div><b>Animations</b><small>Reduce or disable interface motion.</small></div>
            <div class="seg">
              <button v-for="m in ['full', 'reduced', 'off']" :key="m" :class="{ on: s.appearance.animations === m }" @click="s.appearance.animations = m">{{ m[0].toUpperCase() + m.slice(1) }}</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Respect system reduced motion</b><small>Turn animations off when your system asks for reduced motion.</small></div>
            <Toggle v-model="s.appearance.follow_system_motion" label="Follow system motion" />
          </div>
          <div class="opt">
            <div><b>Home background</b></div>
            <div class="seg">
              <button :class="{ on: s.appearance.background === 'image' }" @click="s.appearance.background = 'image'">Image</button>
              <button :class="{ on: s.appearance.background === 'none' }" @click="s.appearance.background = 'none'">None</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Background dimming</b><small>{{ Math.round(s.appearance.background_dim * 100) }}%</small></div>
            <input v-model.number="s.appearance.background_dim" class="range" type="range" min="0" max="0.95" step="0.05" :disabled="s.appearance.background === 'none'" :style="rangeFill(s.appearance.background_dim, 0, 0.95)" />
          </div>
        </template>

        <template v-else-if="section === 'Minecraft'">
          <h2>Minecraft</h2>
          <div class="opt col">
            <div><b>Java</b><small>Managed uses the runtime Mojang specifies per version (with a verified Eclipse Temurin fallback).</small></div>
            <div class="seg wide">
              <button :class="{ on: s.minecraft.java_mode === 'managed' }" @click="s.minecraft.java_mode = 'managed'">Managed</button>
              <button :class="{ on: s.minecraft.java_mode === 'system' }" @click="s.minecraft.java_mode = 'system'">Prefer installed Java</button>
              <button :class="{ on: s.minecraft.java_mode === 'custom' }" @click="pickJava">Custom…</button>
            </div>
            <small v-if="s.minecraft.java_mode === 'custom'" class="path">{{ s.minecraft.java_path }}</small>
          </div>
          <div class="opt col">
            <div class="row between"><b>Detected Java installations</b><button class="btn small" :disabled="javaLoading" @click="loadJava">Refresh</button></div>
            <div class="java-list">
              <div v-for="j in java" :key="j.path" class="java">
                <b>Java {{ j.major }}</b>
                <span class="ellipsis grow" :title="j.path">{{ j.vendor }} · {{ j.version }}</span>
                <span class="tag">{{ j.source }}</span>
              </div>
              <div v-if="javaLoading" class="empty"><span class="spinner" /></div>
              <div v-else-if="!java.length" class="empty">No Java found. Managed runtimes download automatically when needed.</div>
            </div>
            <div class="runtimes">
              <button v-for="r in runtimes" :key="r.id" class="btn small" :disabled="!!runtimeBusy" @click="installRuntime(r.id)">
                <span v-if="runtimeBusy === r.id" class="spinner" />
                {{ r.label }}
              </button>
            </div>
          </div>
          <div class="opt">
            <div><b>Maximum memory</b><small>{{ s.minecraft.memory_max }} MB</small></div>
            <input v-model.number="s.minecraft.memory_max" class="range" type="range" min="1024" max="32768" step="256" :style="rangeFill(s.minecraft.memory_max, 1024, 32768)" />
          </div>
          <div class="opt">
            <div><b>Minimum memory</b><small>{{ s.minecraft.memory_min }} MB</small></div>
            <input v-model.number="s.minecraft.memory_min" class="range" type="range" min="256" :max="s.minecraft.memory_max" step="256" :style="rangeFill(s.minecraft.memory_min, 256, s.minecraft.memory_max)" />
          </div>
          <div class="opt col">
            <div><b>Default JVM arguments</b></div>
            <input v-model.lazy="s.minecraft.jvm_args" class="field mono" placeholder="-XX:+UseG1GC" spellcheck="false" />
          </div>
          <div class="opt">
            <div><b>Default resolution</b><small>0 lets Minecraft decide.</small></div>
            <div class="row">
              <input v-model.number="s.minecraft.width" class="field w80" type="number" min="0" />
              <span class="muted">×</span>
              <input v-model.number="s.minecraft.height" class="field w80" type="number" min="0" />
            </div>
          </div>
          <div class="opt col">
            <div><b>Game files folder</b><small>Shared libraries, assets, versions and runtimes.</small></div>
            <div class="row">
              <input class="field grow" :value="s.minecraft.game_dir || 'Default (inside Zip data folder)'" readonly />
              <button class="btn" @click="pickFolder('game')">Browse</button>
              <button v-if="s.minecraft.game_dir" class="btn ghost" @click="s.minecraft.game_dir = ''">Reset</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Default account</b></div>
            <Dropdown v-model="activeAccount" class="w180" :options="accountOptions" placeholder="No accounts" :disabled="!accountOptions.length" />
          </div>
        </template>

        <template v-else-if="section === 'Downloads'">
          <h2>Downloads</h2>
          <div class="opt">
            <div><b>Concurrent downloads</b><small>{{ s.downloads.concurrency }}</small></div>
            <input v-model.number="s.downloads.concurrency" class="range" type="range" min="1" max="32" :style="rangeFill(s.downloads.concurrency, 1, 32)" />
          </div>
          <div class="opt">
            <div><b>Retries per file</b><small>{{ s.downloads.retries }}</small></div>
            <input v-model.number="s.downloads.retries" class="range" type="range" min="0" max="10" :style="rangeFill(s.downloads.retries, 0, 10)" />
          </div>
          <div class="opt">
            <div><b>Network timeout</b><small>{{ s.downloads.timeout_secs }} seconds</small></div>
            <input v-model.number="s.downloads.timeout_secs" class="range" type="range" min="5" max="120" step="5" :style="rangeFill(s.downloads.timeout_secs, 5, 120)" />
          </div>
          <div class="opt">
            <div><b>Cache limit</b><small>{{ s.downloads.cache_limit_mb }} MB · metadata, icons and skins. Game files are never pruned.</small></div>
            <input v-model.number="s.downloads.cache_limit_mb" class="range" type="range" min="20" max="2048" step="20" :style="rangeFill(s.downloads.cache_limit_mb, 20, 2048)" />
          </div>
          <div class="opt">
            <div><b>Cache usage</b><small>{{ formatBytes(cacheBytes) }}</small></div>
            <button class="btn" @click="clearCache">Clear cache</button>
          </div>
        </template>

        <template v-else-if="section === 'Advanced'">
          <h2>Advanced</h2>
          <div class="opt">
            <div><b>Debug logging</b><small>Write extra detail to the launcher log.</small></div>
            <Toggle v-model="s.advanced.debug_logging" label="Debug logging" />
          </div>
          <div class="opt">
            <div><b>Folders</b></div>
            <div class="row">
              <button class="btn" @click="api.openFolder('data')">Data</button>
              <button class="btn" @click="api.openFolder('logs')">Logs</button>
              <button class="btn" @click="api.openFolder('game')">Game files</button>
              <button class="btn" @click="api.openFolder('zipclient')">Zip Client</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Settings file</b></div>
            <div class="row">
              <button class="btn" @click="exportSettings">Export</button>
              <button class="btn" @click="importSettings">Import</button>
              <button class="btn danger" @click="resetOpen = true">Reset</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Troubleshooting</b><small>Run the first-run setup again or copy diagnostics for a bug report.</small></div>
            <div class="row">
              <button class="btn" @click="rerunSetup">Run setup</button>
              <button class="btn" :disabled="!diagnostics" @click="copyDiagnostics">Copy diagnostics</button>
            </div>
          </div>
          <pre v-if="diagnostics" class="diag selectable">{{ JSON.stringify({ ...diagnostics, java: diagnostics.java.map((j) => `${j.major} ${j.vendor} (${j.source})`) }, null, 2) }}</pre>
        </template>

        <template v-else>
          <h2>About Zip</h2>
          <p class="about">Zip is a lightweight Minecraft launcher for Vanilla, Fabric, Forge, NeoForge and Quilt.</p>
          <div class="opt"><b>Version</b><span>{{ app.info?.version }}</span></div>
          <div class="opt"><b>Mode</b><span>{{ app.info?.portable ? "Portable" : "Installed" }}</span></div>
          <div class="opt col"><b>Data folder</b><span class="path selectable">{{ app.info?.dataDir }}</span></div>
          <div class="opt"><b>Microsoft sign-in</b><span>{{ app.info?.msConfigured ? "Configured" : "Not configured" }}</span></div>
          <div class="opt"><b>CurseForge</b><span>{{ app.info?.curseforgeConfigured ? "Configured" : "Not configured" }}</span></div>
          <div class="opt"><b>Zip Client</b><span>Coming soon</span></div>
          <div class="opt col"><b>Privacy</b><span class="path">Zip collects no telemetry, analytics or usage data. It only contacts the services needed for what you do: Mojang and Microsoft for the game and sign-in, Modrinth and CurseForge for content, Adoptium for Java, GitHub for Horizon and updates.</span></div>
          <div class="opt col">
            <b>Legal</b>
            <div class="row">
              <button class="btn" @click="app.legalDoc = 'terms'">Terms of Use</button>
              <button class="btn" @click="app.legalDoc = 'privacy'">Privacy Policy</button>
              <button class="btn" @click="app.legalDoc = 'disclaimer'">Legal Disclaimer</button>
            </div>
          </div>
          <p class="legal">Produced by Krizuvo. NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.</p>
        </template>
      </div>
    </Transition>

    <UiEditor v-if="editorOpen" @close="editorOpen = false" />
    <Modal v-if="resetOpen" title="Reset settings?" :width="400" @close="resetOpen = false">
      <div class="confirm">
        <p>All launcher settings return to their defaults. Accounts and instances are kept.</p>
        <div class="row end">
          <button class="btn" @click="resetOpen = false">Cancel</button>
          <button class="btn danger" @click="resetSettings">Reset settings</button>
        </div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>
.settings {
  height: 100%;
  display: grid;
  grid-template-columns: 170px 1fr;
  gap: 11px;
  padding: 12px 11px 12px 12px;
}

.cats {
  border-radius: 13px;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.cat {
  text-align: left;
  padding: 8px 10px;
  border-radius: var(--r-row);
  font-size: calc(13px * var(--text-scale, 1));
  color: var(--text-dim);
  position: relative;
  border: 1px solid transparent;
  transition: background var(--t-med) var(--ease), color var(--t-fast), border-color var(--t-fast);
}

.cat:hover {
  background: var(--chip-hover);
  color: var(--text);
}

.cat.on {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
  color: var(--accent);
  font-weight: 600;
  box-shadow: inset 2px 0 0 var(--accent);
}

.ver {
  margin-top: auto;
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-faint);
  padding: 4px 8px;
}

.body {
  border-radius: 13px;
  padding: 14px 18px 24px;
  min-height: 0;
}

h2 {
  font-size: calc(20px * var(--text-scale, 1));
  font-weight: 600;
  margin: 0 0 10px;
}

h3 {
  font-size: calc(13px * var(--text-scale, 1));
  font-weight: 600;
  margin: 18px 0 6px;
  color: var(--text-dim);
}

.opt {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 12px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  box-shadow: inset 0 1px 0 var(--hairline);
  margin-bottom: 6px;
  transition: border-color var(--t-fast);
}

.opt:hover {
  border-color: var(--border);
}

.opt.edit-ui {
  border-color: var(--accent-border);
  background: var(--accent-bg);
}

.opt.col {
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}

.opt b {
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
  display: block;
}

.opt small {
  display: block;
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
  margin-top: 2px;
}

.between {
  justify-content: space-between;
}

.w180 {
  width: 180px;
  flex: none;
}

.w80 {
  width: 80px;
}

.range {
  width: 200px;
  flex: none;
}

.range:disabled {
  opacity: 0.4;
}

.seg {
  display: flex;
  gap: 3px;
  padding: 3px;
  background: var(--panel-3);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
}

.seg.wide button {
  flex: 1;
}

.seg button {
  padding: 5px 12px;
  border-radius: var(--r-chip);
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  transition: background var(--t-med), color var(--t-fast);
}

.seg button:hover {
  color: var(--text);
}

.seg button.on {
  background: var(--accent-bg-hover);
  color: var(--accent);
  font-weight: 600;
  box-shadow: inset 0 0 0 1px var(--accent-border);
}

.themes {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
  gap: 8px;
}

.theme {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px;
  border-radius: var(--r-row);
  border: 1px solid var(--border-soft);
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-dim);
  position: relative;
  transition: border-color var(--t-fast), transform var(--t-fast) var(--ease), color var(--t-fast), box-shadow var(--t-fast);
}

.theme:hover {
  transform: translateY(-2px);
  color: var(--text);
  border-color: var(--border-strong);
}

.theme.on {
  border-color: var(--accent);
  color: var(--text);
  font-weight: 600;
  box-shadow: 0 0 0 1px var(--accent-border), 0 4px 16px var(--accent-glow);
}

.theme.on::after {
  content: "";
  position: absolute;
  top: 10px;
  right: 10px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 8px var(--accent-glow);
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Cpath d='M4 8.5l2.5 2.5L12 5.5' fill='none' stroke='%230d1f12' stroke-width='2.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
  background-size: 11px;
  background-position: center;
  background-repeat: no-repeat;
}

.preview {
  height: 44px;
  border-radius: 6px;
  position: relative;
  display: block;
  border: 1px solid rgba(128, 128, 128, 0.25);
}

.preview span {
  position: absolute;
  left: 6px;
  right: 18px;
  top: 6px;
  bottom: 6px;
  border-radius: 4px;
}

.preview i {
  position: absolute;
  right: 5px;
  bottom: 6px;
  width: 9px;
  height: 9px;
  border-radius: 50%;
}

.swatches {
  display: flex;
  gap: 6px;
  align-items: center;
}

.swatch {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 2px solid transparent;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
  transition: transform var(--t-fast) var(--ease-spring), border-color var(--t-fast), box-shadow var(--t-fast);
}

.swatch:hover {
  transform: scale(1.15);
}

.swatch.on {
  border-color: var(--text);
  box-shadow: 0 0 0 2px var(--panel), 0 0 10px var(--accent-glow);
}

.swatch.custom {
  background: conic-gradient(#ff6b6b, #ffd93d, #6bff95, #6bc5ff, #c56bff, #ff6b6b);
  position: relative;
  overflow: hidden;
  cursor: pointer;
}

.swatch.custom input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.mono {
  font-family: ui-monospace, Consolas, monospace;
  font-size: calc(11px * var(--text-scale, 1));
}

.path {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-dim);
  word-break: break-all;
}

.java-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.java {
  display: flex;
  gap: 10px;
  align-items: center;
  font-size: calc(12px * var(--text-scale, 1));
  padding: 6px 8px;
  border-radius: var(--r-small);
  background: var(--panel);
  border: 1px solid var(--border-soft);
}

.tag {
  font-size: calc(9px * var(--text-scale, 1));
  text-transform: uppercase;
  padding: 2px 6px;
  border-radius: 10px;
  background: var(--panel-3);
  color: var(--text-faint);
}

.runtimes {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.update-box {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border-radius: var(--r-row);
  background: var(--accent-bg);
  border: 1px solid var(--accent-border);
  box-shadow: inset 0 1px 0 var(--hairline);
}

.notes {
  white-space: pre-wrap;
  max-height: 80px;
  overflow: auto;
}

.warn-text {
  color: var(--warn) !important;
}

.diag {
  background: var(--panel-3);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  padding: 10px;
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-dim);
  overflow: auto;
  max-height: 220px;
}

.about {
  color: var(--text-dim);
  margin: 0 0 12px;
}

.legal {
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-faint);
  margin-top: 16px;
}

.confirm {
  padding: 0 20px 18px;
}

.confirm p {
  color: var(--text-dim);
  margin: 0 0 14px;
}

.end {
  justify-content: flex-end;
}
</style>
