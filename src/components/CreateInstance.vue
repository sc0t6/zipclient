<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { openUrl } from "@tauri-apps/plugin-opener";
import Modal from "./Modal.vue";
import Toggle from "./Toggle.vue";
import VersionPicker from "./VersionPicker.vue";
import ModItem from "./ModItem.vue";
import IconPicker from "./IconPicker.vue";
import { randomPreset } from "../presets";
import { api, errorText, formatCount, loaderNames, type ImportResult, type Loader, type OptimizePreset, type SearchHit } from "../api";
import { importFile, importing, importPath, installModpack, type ImportKind } from "../importer";
import { icons } from "../icons";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";

const emit = defineEmits<{ close: [] }>();
const app = useApp();
const notify = useNotify();

const tab = ref<"new" | "modpack" | "import">("new");
const name = ref("");
const icon = ref(randomPreset());
const nameTouched = ref(false);
const minecraft = ref("");
const loader = ref<Loader>("vanilla");
const loaderVersion = ref("");
const group = ref("");
const creating = ref(false);
const optimized = ref(true);
const memoryWarning = ref(true);
const fastLaunch = ref(false);
const preset = ref<OptimizePreset | null>(null);
let presetTimer = 0;

function loadPreset() {
  window.clearTimeout(presetTimer);
  preset.value = null;
  if (!optimized.value || !minecraft.value || loader.value === "horizon") return;
  const key = `${minecraft.value}|${loader.value}`;
  presetTimer = window.setTimeout(async () => {
    const result = await api.optimizePreset(minecraft.value, loader.value).catch(() => null);
    if (key === `${minecraft.value}|${loader.value}`) preset.value = result;
  }, 250);
}

watch([minecraft, loader, optimized], loadPreset);

const gb = (mb: number) => `${(mb / 1024).toFixed(mb % 1024 ? 1 : 0)} GB`;

const query = ref("");
const packs = ref<SearchHit[]>([]);
const packsLoading = ref(false);
const installingPack = ref("");
let searchTimer = 0;

const defaultName = computed(() => (minecraft.value ? `${loader.value === "vanilla" ? "Minecraft" : loaderNames[loader.value]} ${minecraft.value}` : ""));
const canCreate = computed(() => !!minecraft.value && (loader.value === "vanilla" || !!loaderVersion.value) && !creating.value);
const importProgress = computed(() => app.progress["import"]);

watch(defaultName, (n) => {
  if (!nameTouched.value) name.value = n;
});

watch(
  () => app.versions,
  (v) => {
    if (v && !minecraft.value) minecraft.value = v.latestRelease;
  },
  { immediate: true },
);

async function create() {
  if (!canCreate.value) return;
  creating.value = true;
  try {
    const instance = await api.createInstance({
      name: name.value.trim() || defaultName.value,
      minecraft: minecraft.value,
      loader: loader.value,
      loaderVersion: loaderVersion.value,
      group: group.value,
    });
    const horizon = loader.value === "horizon";
    const tuned = !horizon && optimized.value ? preset.value ?? (await api.optimizePreset(minecraft.value, loader.value).catch(() => null)) : null;
    const final = await api.updateInstance(instance.id, {
      icon: icon.value,
      ...(horizon ? {} : { memoryWarning: memoryWarning.value, fastLaunch: fastLaunch.value }),
      ...(tuned ? { memoryMin: tuned.memoryMin, memoryMax: tuned.memoryMax, jvmArgs: tuned.jvmArgs } : {}),
    });
    app.replaceInstance(final);
    app.quickInstance = instance.id;
    notify.success("Instance created", `${instance.name} will download when you press Play.`);
    emit("close");
    app.openInstance(instance.id);
  } catch (e) {
    notify.error("Could not create instance", errorText(e));
  } finally {
    creating.value = false;
  }
}

async function searchPacks() {
  packsLoading.value = true;
  try {
    const result = await api.search({ source: "modrinth", kind: "modpack", query: query.value, minecraft: "", loader: "", category: "", sort: "relevance", offset: 0, limit: 20 });
    packs.value = result.hits;
  } catch (e) {
    notify.error("Modpack search failed", errorText(e));
  } finally {
    packsLoading.value = false;
  }
}

function packMenu(hit: SearchHit) {
  const url = `https://modrinth.com/modpack/${hit.slug}`;
  return [
    { label: "Open page", run: () => openUrl(url) },
    { label: "Copy link", run: () => navigator.clipboard.writeText(url).catch(() => {}) },
  ];
}

async function installPack(hit: SearchHit) {
  installingPack.value = hit.id;
  const result = await installModpack(hit.id);
  installingPack.value = "";
  if (result) {
    emit("close");
    app.openInstance(result.instance.id);
  }
}

const sources: { kind: ImportKind; name: string; ext: string }[] = [
  { kind: "curseforge", name: "CurseForge", ext: ".zip modpack" },
  { kind: "prism", name: "Prism Launcher", ext: ".zip instance" },
  { kind: "modrinth", name: "Modrinth", ext: ".mrpack modpack" },
];
const dragging = ref(false);
const importPercent = computed(() => Math.round((importProgress.value?.fraction ?? 0) * 100));

async function finishImport(pending: Promise<ImportResult | null>) {
  const result = await pending;
  if (result) {
    emit("close");
    app.openInstance(result.instance.id);
  }
}

const doImport = (kind: ImportKind) => finishImport(importFile(kind));

const stopDrop = getCurrentWebview().onDragDropEvent(({ payload }) => {
  if (tab.value !== "import" || importing.value || payload.type === "leave") {
    dragging.value = false;
    return;
  }
  if (payload.type !== "drop") {
    dragging.value = true;
    return;
  }
  dragging.value = false;
  const path = payload.paths.find((p) => /\.(zip|mrpack)$/i.test(p));
  if (path) finishImport(importPath(path));
  else notify.error("Unsupported file", "Drop a .zip or .mrpack file.");
});

onUnmounted(() => stopDrop.then((stop) => stop()));

watch(query, () => {
  window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(searchPacks, 350);
});

watch(tab, (t) => {
  if (t === "modpack" && !packs.value.length) searchPacks();
});

onMounted(() => {
  if (!app.versions) app.loadVersions();
  loadPreset();
});
</script>

<template>
  <Modal title="New instance" :width="620" :dismissable="!importing" @close="!importing && emit('close')">
    <div class="tabs">
      <button v-for="t in [['new', 'Create'], ['modpack', 'Modpacks'], ['import', 'Import']]" :key="t[0]" class="tab" :class="{ on: tab === t[0] }" :disabled="importing" @click="tab = t[0] as typeof tab">
        {{ t[1] }}
      </button>
    </div>
    <div v-if="tab === 'new'" class="body scroll">
      <label class="label">Name</label>
      <input v-model="name" class="field" maxlength="64" placeholder="Instance name" @input="nameTouched = true" @keydown.enter="create" />
      <label class="label">Icon</label>
      <IconPicker v-model="icon" :letter="name || defaultName" />
      <VersionPicker v-model:minecraft="minecraft" v-model:loader="loader" v-model:loader-version="loaderVersion" />
      <div v-if="loader !== 'horizon'" class="options">
        <div class="opt">
          <div class="grow">
            <b>Optimized Mode</b>
            <small v-if="!optimized">Use the launcher defaults for memory and JVM arguments.</small>
            <small v-else-if="preset">{{ gb(preset.memoryMax) }} of {{ gb(preset.systemMb) }} RAM · G1 garbage collector tuned for Java {{ preset.javaMajor }} and {{ loaderNames[loader] }}</small>
            <small v-else>Checking this Minecraft version, loader and your system..</small>
          </div>
          <Toggle v-model="optimized" label="Optimized Mode" />
        </div>
        <div class="opt">
          <div class="grow">
            <b>Low memory warning</b>
            <small>Notify me when Minecraft is running out of memory.</small>
          </div>
          <Toggle v-model="memoryWarning" label="Low memory warning" />
        </div>
        <div class="opt">
          <div class="grow">
            <b>Improve loading speeds</b>
            <small>Skips file checks and launches straight away. First launch still checks once, then checks turn off.</small>
          </div>
          <Toggle v-model="fastLaunch" label="Improve loading speeds" />
        </div>
      </div>
      <label class="label">Group (optional)</label>
      <input v-model="group" class="field" maxlength="40" placeholder="e.g. Survival" />
      <div class="footer">
        <button class="btn" @click="emit('close')">Cancel</button>
        <button class="btn primary" :disabled="!canCreate" @click="create">
          <span v-if="creating" class="spinner" />
          Create instance
        </button>
      </div>
    </div>
    <div v-else-if="tab === 'modpack'" class="body">
      <input v-model="query" class="field pill" placeholder="Search Modrinth modpacks.." />
      <div class="packs scroll">
        <ModItem
          v-for="hit in packs"
          :key="hit.id"
          :icon="hit.iconUrl"
          :title="hit.title"
          :subtitle="`by ${hit.author} · ${formatCount(hit.downloads)} downloads`"
          :description="hit.description"
          :busy="installingPack === hit.id"
          :disabled="importing && installingPack !== hit.id"
          install-label="+ Install"
          :menu="packMenu(hit)"
          @install="installPack(hit)"
        />
        <div v-if="packsLoading" class="empty"><span class="spinner" /></div>
        <div v-else-if="!packs.length" class="empty">No modpacks found.</div>
      </div>
      <div v-if="importing && importProgress" class="progress-line">
        <span class="ellipsis">{{ importProgress.phase }}</span>
        <span>{{ Math.round(importProgress.fraction * 100) }}%</span>
      </div>
    </div>
    <div v-else class="body">
      <button class="drop" :class="{ over: dragging, busy: importing }" :disabled="importing" @click="doImport('any')">
        <template v-if="importing">
          <span class="spinner big" />
          <b class="ellipsis">{{ importProgress?.phase ?? "Importing.." }}</b>
          <div class="bar"><div :style="{ width: `${importPercent}%` }" /></div>
          <small>{{ importPercent }}%</small>
        </template>
        <template v-else>
          <span class="badge"><img :src="icons.upload" class="icon" alt="" /></span>
          <b>{{ dragging ? "Release to import" : "Drop a modpack here" }}</b>
          <small>or click to browse · .zip and .mrpack</small>
        </template>
      </button>
      <label class="label">Or pick a format</label>
      <div class="import-grid">
        <button v-for="s in sources" :key="s.kind" class="import-card" :disabled="importing" @click="doImport(s.kind)">
          <strong>{{ s.name }}</strong>
          <span>{{ s.ext }}</span>
        </button>
      </div>
      <p class="muted-text">Files are validated and extracted safely into a new instance.</p>
    </div>
  </Modal>
</template>

<style scoped>
.tabs {
  display: flex;
  gap: 4px;
  margin: 0 20px 4px;
  padding: 3px;
  background: var(--panel);
  border-radius: var(--r-row);
}

.tab {
  flex: 1;
  height: 28px;
  border-radius: var(--r-chip);
  font-size: 12px;
  color: var(--text-dim);
  transition: background var(--t-med), color var(--t-fast);
}

.tab.on {
  background: var(--chip-active);
  color: var(--text);
  font-weight: 600;
}

.body {
  padding: 8px 20px 18px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 0;
  max-height: 70vh;
}

.options {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
}

.opt {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 9px 12px;
  border-radius: var(--r-row);
  background: var(--panel);
}

.opt b {
  font-size: 12px;
  display: block;
}

.opt small {
  font-size: 11px;
  color: var(--text-faint);
}

.body > * {
  flex-shrink: 0;
}

.footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 14px;
}

.packs {
  display: flex;
  flex-direction: column;
  gap: 6px;
  height: 340px;
  margin-top: 6px;
}

.drop {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  min-height: 196px;
  margin-top: 4px;
  padding: 20px;
  background: var(--panel);
  border: 1.5px dashed var(--border);
  border-radius: var(--r-panel);
  transition: background var(--t-fast), border-color var(--t-fast), transform var(--t-med) var(--ease);
}

.drop:hover:not(:disabled),
.drop.over {
  background: var(--accent-bg);
  border-color: var(--accent-border);
}

.drop.over {
  background: var(--accent-bg-hover);
  border-color: var(--accent);
  transform: scale(1.01);
}

.drop.busy {
  cursor: progress;
  border-style: solid;
  border-color: var(--border-soft);
}

.drop .badge {
  width: 58px;
  height: 58px;
  margin-bottom: 8px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: var(--chip-active);
  transition: background var(--t-fast), transform var(--t-med) var(--ease-spring);
}

.drop:hover:not(:disabled) .badge,
.drop.over .badge {
  background: color-mix(in srgb, var(--accent) 22%, var(--panel));
  transform: translateY(-3px);
}

.drop .badge .icon {
  width: 26px;
  height: 26px;
}

.drop b {
  font-size: 14px;
  max-width: 100%;
}

.drop small {
  font-size: 11px;
  color: var(--text-dim);
}

.spinner.big {
  width: 30px;
  height: 30px;
  border-width: 3px;
  margin-bottom: 10px;
}

.bar {
  width: 60%;
  height: 4px;
  margin-top: 8px;
  border-radius: 2px;
  overflow: hidden;
  background: var(--chip-active);
}

.bar div {
  height: 100%;
  background: var(--accent);
  transition: width var(--t-med);
}

.import-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.import-card {
  background: var(--panel);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  align-items: center;
  transition: background var(--t-fast), transform var(--t-fast), border-color var(--t-fast);
}

.import-card:hover:not(:disabled) {
  background: var(--accent-bg-hover);
  border-color: var(--border);
}

.import-card:active:not(:disabled) {
  transform: scale(0.98);
}

.import-card:disabled {
  opacity: 0.5;
}

.import-card span {
  font-size: 11px;
  color: var(--text-dim);
}

.muted-text {
  color: var(--text-dim);
  font-size: 12px;
  margin: 4px 0;
}

.progress-line {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--text-dim);
  margin-top: 6px;
}
</style>
