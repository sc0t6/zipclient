<script setup lang="ts">
import { computed, onActivated, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import Dropdown from "./Dropdown.vue";
import ModItem from "./ModItem.vue";
import KebabMenu, { type MenuItem } from "./KebabMenu.vue";
import Modal from "./Modal.vue";
import Toggle from "./Toggle.vue";
import VirtualList from "./VirtualList.vue";
import { api, errorText, formatBytes, formatCount, type ContentKind, type ContentVersion, type Instance, type InstalledFile, type SearchHit, type UpdateInfo } from "../api";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";

const props = defineProps<{ instance: Instance; kind: ContentKind }>();
const app = useApp();
const notify = useNotify();

const labels: Record<string, { one: string; many: string; folder: string }> = {
  mod: { one: "mod", many: "Mods", folder: "mods" },
  resourcepack: { one: "resource pack", many: "Resource packs", folder: "resourcepacks" },
  shader: { one: "shader", many: "Shaders", folder: "shaderpacks" },
};
const label = computed(() => labels[props.kind]);

const mode = ref<"installed" | "browse">("installed");
const view = ref<"list" | "grid">(storedView());
let autoChecked = false;
const files = ref<InstalledFile[]>([]);
const loading = ref(false);
const filter = ref("");
const updates = ref<Record<string, UpdateInfo>>({});
const checking = ref(false);
const pendingRemove = ref("");
const busy = reactive<Record<string, boolean>>({});

const query = ref("");
const category = ref("");
const sort = ref("relevance");
const horizonMods = props.instance.loader === "horizon" && props.kind === "mod";
const source = ref(horizonMods ? "horizon" : "modrinth");
const hits = ref<SearchHit[]>([]);
const total = ref(0);
const searching = ref(false);
const categories = ref<{ id: string; name: string }[]>([]);
const chosenVersion = reactive<Record<string, ContentVersion>>({});
const versionDialog = ref<SearchHit | null>(null);
const versionList = ref<ContentVersion[]>([]);
const versionLoading = ref(false);
let searchTimer = 0;
let searchToken = 0;

const installedBy = computed(() => new Map(files.value.filter((f) => f.projectId).map((f) => [f.projectId, f])));
const folder = computed(() => (horizonMods ? "horizon-mods" : label.value.folder));

function storedView(): "list" | "grid" {
  try {
    return localStorage.getItem("zip.contentView") === "grid" ? "grid" : "list";
  } catch {
    return "list";
  }
}

watch(view, (v) => {
  try {
    localStorage.setItem("zip.contentView", v);
  } catch {}
});
const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return q ? files.value.filter((f) => f.title.toLowerCase().includes(q) || f.fileName.toLowerCase().includes(q)) : files.value;
});
const updateCount = computed(() => Object.keys(updates.value).length);
const vanillaMods = computed(() => props.kind === "mod" && props.instance.loader === "vanilla");
const sourceOptions = computed(() => horizonMods ? [{ value: "horizon", label: "Horizon mods" }] : [
  { value: "modrinth", label: "Modrinth" },
  { value: "curseforge", label: "CurseForge", disabled: !app.info?.curseforgeConfigured, hint: app.info?.curseforgeConfigured ? "" : "API key needed" },
]);
const sortOptions = [
  { value: "relevance", label: "Relevance" },
  { value: "downloads", label: "Downloads" },
  { value: "updated", label: "Updated" },
  { value: "newest", label: "Newest" },
];
const categoryOptions = computed(() => [{ value: "", label: "All categories" }, ...categories.value.map((c) => ({ value: c.id, label: c.name }))]);

async function load(identify: boolean) {
  loading.value = !files.value.length;
  try {
    files.value = await api.listContent(props.instance.id, props.kind, identify);
  } catch (e) {
    notify.error(`Could not read ${label.value.many.toLowerCase()}`, errorText(e));
  } finally {
    loading.value = false;
  }
}

async function refresh() {
  await load(false);
  await load(true);
  if (!autoChecked && files.value.some((f) => f.source === "modrinth" || f.source === "horizon")) {
    autoChecked = true;
    checkUpdates(true);
  }
}

async function toggle(file: InstalledFile, enabled: boolean) {
  try {
    const newPath = await api.toggleContent(props.instance.id, file.path, enabled);
    file.path = newPath;
    file.enabled = enabled;
    if (updates.value[file.path.replace(/\.disabled$/, "")]) updates.value = { ...updates.value };
  } catch (e) {
    notify.error("Could not change state", errorText(e));
  }
}

async function remove(file: InstalledFile) {
  if (pendingRemove.value !== file.path) {
    pendingRemove.value = file.path;
    window.setTimeout(() => pendingRemove.value === file.path && (pendingRemove.value = ""), 3000);
    return;
  }
  pendingRemove.value = "";
  try {
    await api.removeContent(props.instance.id, file.path);
    files.value = files.value.filter((f) => f.path !== file.path);
  } catch (e) {
    notify.error("Could not remove file", errorText(e));
  }
}

async function addFiles() {
  const paths = await open({
    multiple: true,
    filters: [{ name: label.value.many, extensions: horizonMods ? ["hmod"] : props.kind === "mod" ? ["jar", "zip"] : ["zip"] }],
  });
  if (!paths) return;
  try {
    const list = Array.isArray(paths) ? paths : [paths];
    const added = await api.importContent(props.instance.id, props.kind, list);
    notify.success(`Added ${added.length} ${added.length === 1 ? label.value.one : label.value.many.toLowerCase()}`);
    refresh();
  } catch (e) {
    notify.error("Import failed", errorText(e));
  }
}

async function checkUpdates(silent = false) {
  checking.value = true;
  try {
    const list = await api.contentUpdates(props.instance.id, props.kind);
    updates.value = Object.fromEntries(list.map((u) => [u.path.replace(/\.disabled$/, ""), u]));
    if (!list.length && !silent) notify.info("Everything is up to date");
  } catch (e) {
    if (!silent) notify.error("Update check failed", errorText(e));
  } finally {
    checking.value = false;
  }
}

function updateFor(file: InstalledFile) {
  return updates.value[file.path.replace(/\.disabled$/, "")];
}

async function applyUpdate(file: InstalledFile) {
  const info = updateFor(file);
  if (!info) return;
  busy[file.path] = true;
  try {
    await api.updateContent(props.instance.id, file.path, info.versionId);
    const next = { ...updates.value };
    delete next[file.path.replace(/\.disabled$/, "")];
    updates.value = next;
    notify.success("Updated", `${file.title} → ${info.latest}`);
    await load(false);
  } catch (e) {
    notify.error("Update failed", errorText(e));
  } finally {
    busy[file.path] = false;
  }
}

async function updateAll() {
  for (const file of files.value.filter((f) => updateFor(f))) await applyUpdate(file);
}

async function search(reset = true) {
  const token = ++searchToken;
  searching.value = true;
  try {
    const result = await api.search({
      source: source.value,
      kind: props.kind,
      query: query.value,
      minecraft: props.instance.minecraft,
      loader: props.instance.loader,
      category: category.value,
      sort: sort.value,
      offset: reset ? 0 : hits.value.length,
      limit: 20,
    });
    if (token !== searchToken) return;
    hits.value = reset ? result.hits : [...hits.value, ...result.hits.filter((h) => !hits.value.some((x) => x.id === h.id))];
    total.value = result.total;
  } catch (e) {
    if (token === searchToken) notify.error("Search failed", errorText(e));
  } finally {
    if (token === searchToken) searching.value = false;
  }
}

function more() {
  if (!searching.value && hits.value.length < total.value) search(false);
}

async function loadCategories() {
  try {
    categories.value = await api.categories(source.value, props.kind);
  } catch {
    categories.value = [];
  }
}

async function install(hit: SearchHit) {
  busy[hit.id] = true;
  try {
    const result = await api.installContent(props.instance.id, hit.source, hit.id, chosenVersion[hit.id]?.id ?? null, props.kind);
    const extra = result.installed.length > 1 ? ` (+${result.installed.length - 1} dependencies)` : "";
    delete chosenVersion[hit.id];
    notify.success(`${label.value.one[0].toUpperCase()}${label.value.one.slice(1)} installed`, `${hit.title}${extra}`);
    if (result.skipped.length) notify.warn("Some files were skipped", result.skipped.join("\n"));
    await load(false);
  } catch (e) {
    notify.error(`Could not install ${hit.title}`, errorText(e));
  } finally {
    busy[hit.id] = false;
  }
}

async function chooseVersion(hit: SearchHit) {
  versionDialog.value = hit;
  versionList.value = [];
  versionLoading.value = true;
  try {
    versionList.value = await api.contentVersions(hit.source, hit.id, props.kind, props.instance.minecraft, props.instance.loader);
  } catch (e) {
    notify.error("Could not load versions", errorText(e));
  } finally {
    versionLoading.value = false;
  }
}

function pickVersion(v: ContentVersion) {
  if (versionDialog.value) chosenVersion[versionDialog.value.id] = v;
  versionDialog.value = null;
}

function copy(text: string) {
  navigator.clipboard.writeText(text).catch(() => {});
}

function isInstalled(hit: SearchHit) {
  const file = installedBy.value.get(hit.id);
  if (!file) return false;
  const chosen = chosenVersion[hit.id];
  return !chosen || chosen.id === file.versionId;
}

function versionLabel(hit: SearchHit) {
  const chosen = chosenVersion[hit.id];
  if (chosen) return `v${chosen.versionNumber}`;
  const file = installedBy.value.get(hit.id);
  return file ? `✓ ${file.version || "Ver"}` : "+ Ver";
}

function fileUrl(file: InstalledFile) {
  if (!file.projectId) return "";
  if (file.source === "modrinth") return `https://modrinth.com/project/${file.projectId}`;
  if (file.source === "curseforge") return `https://www.curseforge.com/projects/${file.projectId}`;
  if (file.source === "horizon") return `https://github.com/horizon-rust/horizon-mods/blob/main/mods/${file.projectId}.toml`;
  return "";
}

function fileMenu(file: InstalledFile): MenuItem[] {
  const url = fileUrl(file);
  const update = updateFor(file);
  return [
    ...(url ? [{ label: "Open page", run: () => openUrl(url) }] : []),
    ...(update ? [{ label: `Update to ${update.latest}`, run: () => applyUpdate(file) }] : []),
    { label: file.enabled ? "Disable" : "Enable", run: () => toggle(file, !file.enabled) },
    { label: "Open folder", run: () => api.openInstanceFolder(props.instance.id, folder.value) },
    { label: "Copy file name", run: () => copy(file.fileName) },
  ];
}

function hitMenu(hit: SearchHit): MenuItem[] {
  return [
    { label: "Open page", run: () => openUrl(projectUrl(hit)) },
    { label: "Copy link", run: () => copy(projectUrl(hit)) },
    ...(installedBy.value.has(hit.id) ? [{ label: "Open folder", run: () => api.openInstanceFolder(props.instance.id, folder.value) }] : []),
  ];
}

function projectUrl(hit: SearchHit) {
  if (hit.source === "horizon") return `https://github.com/horizon-rust/horizon-mods/blob/main/mods/${hit.id}.toml`;
  if (hit.source === "curseforge") return `https://www.curseforge.com/minecraft/${props.kind === "mod" ? "mc-mods" : props.kind === "shader" ? "shaders" : "texture-packs"}/${hit.slug}`;
  return `https://modrinth.com/${props.kind}/${hit.slug}`;
}

watch([query, category, sort, source], () => {
  window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => search(true), 300);
});

watch(source, loadCategories);

watch(mode, (m) => {
  if (m === "browse" && !hits.value.length) {
    search(true);
    loadCategories();
  }
});

let focusTimer = 0;

function onFocus() {
  if (mode.value !== "installed" || document.hidden) return;
  window.clearTimeout(focusTimer);
  focusTimer = window.setTimeout(() => load(true), 250);
}

onMounted(() => {
  refresh();
  window.addEventListener("focus", onFocus);
  document.addEventListener("visibilitychange", onFocus);
});

onActivated(onFocus);

onUnmounted(() => {
  window.clearTimeout(focusTimer);
  window.removeEventListener("focus", onFocus);
  document.removeEventListener("visibilitychange", onFocus);
});
</script>

<template>
  <div class="content">
    <template v-if="mode === 'installed'">
      <div class="toolbar">
        <input v-model="filter" class="field pill grow" :placeholder="`Filter ${label.many.toLowerCase()}..`" />
        <div class="seg" role="group" aria-label="View">
          <button :class="{ on: view === 'list' }" title="List view" @click="view = 'list'">
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4h12M2 8h12M2 12h12" /></svg>
          </button>
          <button :class="{ on: view === 'grid' }" title="Grid view" @click="view = 'grid'">
            <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2" y="2" width="5" height="5" rx="1" /><rect x="9" y="2" width="5" height="5" rx="1" /><rect x="2" y="9" width="5" height="5" rx="1" /><rect x="9" y="9" width="5" height="5" rx="1" /></svg>
          </button>
        </div>
        <button class="btn primary" :disabled="vanillaMods" @click="mode = 'browse'">+ Browse</button>
        <button class="btn" :disabled="vanillaMods" @click="addFiles">Add file</button>
        <button class="btn" :disabled="checking || !files.length" @click="checkUpdates()">
          <span v-if="checking" class="spinner" />
          Check updates
        </button>
        <button class="btn ghost" title="Open folder" @click="api.openInstanceFolder(instance.id, folder)">Folder</button>
      </div>
      <div v-if="vanillaMods" class="banner">Vanilla instances cannot load mods. Choose Fabric, Forge, NeoForge or Quilt in the instance settings.</div>
      <div v-if="updateCount" class="banner update">
        {{ updateCount }} update{{ updateCount === 1 ? "" : "s" }} available
        <button class="btn small primary" @click="updateAll">Update all</button>
      </div>
      <VirtualList v-if="shown.length && view === 'list'" class="list" :items="shown" :item-height="52" :gap="5" :key-of="(f) => f.path">
        <template #default="{ item }">
          <div class="file" :class="{ disabled: !item.enabled }">
            <div class="file-icon">
              <img v-if="item.iconUrl" :src="item.iconUrl" loading="lazy" alt="" />
              <span v-else>{{ item.title.charAt(0).toUpperCase() }}</span>
            </div>
            <div class="grow">
              <div class="file-title ellipsis">{{ item.title }}</div>
              <div class="file-sub ellipsis">
                <template v-if="item.version">{{ item.version }} · </template>{{ item.fileName }} · {{ formatBytes(item.size) }}
              </div>
            </div>
            <span class="src" :class="item.source">{{ item.source }}</span>
            <button v-if="updateFor(item)" class="btn small primary" :disabled="busy[item.path]" @click="applyUpdate(item)">
              <span v-if="busy[item.path]" class="spinner" />
              Update to {{ updateFor(item)!.latest }}
            </button>
            <Toggle :model-value="item.enabled" :label="`Enable ${item.title}`" @update:model-value="toggle(item, $event)" />
            <KebabMenu :items="fileMenu(item)" :size="28" />
            <button class="rm" :class="{ confirm: pendingRemove === item.path }" :title="pendingRemove === item.path ? 'Click again to remove' : 'Remove'" @click="remove(item)">
              {{ pendingRemove === item.path ? "Remove?" : "✕" }}
            </button>
          </div>
        </template>
      </VirtualList>
      <div v-else-if="shown.length" class="grid scroll">
        <div v-for="item in shown" :key="item.path" class="card" :class="{ disabled: !item.enabled, upd: updateFor(item) }">
          <div class="card-top">
            <div class="file-icon big">
              <img v-if="item.iconUrl" :src="item.iconUrl" loading="lazy" alt="" />
              <span v-else>{{ item.title.charAt(0).toUpperCase() }}</span>
            </div>
            <div class="card-text">
              <div class="file-title ellipsis" :title="item.title">{{ item.title }}</div>
              <div class="file-sub ellipsis" :title="item.fileName">{{ item.version || item.fileName }}</div>
            </div>
            <KebabMenu :items="fileMenu(item)" :size="28" />
          </div>
          <div class="card-bottom">
            <span class="src" :class="item.source">{{ item.source }}</span>
            <span class="grow" />
            <button v-if="updateFor(item)" class="btn small primary" :disabled="busy[item.path]" :title="`Update to ${updateFor(item)!.latest}`" @click="applyUpdate(item)">
              <span v-if="busy[item.path]" class="spinner" />
              Update
            </button>
            <Toggle :model-value="item.enabled" :label="`Enable ${item.title}`" @update:model-value="toggle(item, $event)" />
            <button class="rm" :class="{ confirm: pendingRemove === item.path }" :title="pendingRemove === item.path ? 'Click again to remove' : 'Remove'" @click="remove(item)">
              {{ pendingRemove === item.path ? "Remove?" : "✕" }}
            </button>
          </div>
        </div>
      </div>
      <div v-else-if="loading" class="empty"><span class="spinner" /></div>
      <div v-else class="empty">No {{ label.many.toLowerCase() }} installed{{ filter ? " matching that filter" : "" }}.</div>
    </template>

    <template v-else>
      <div class="toolbar">
        <button class="btn" @click="mode = 'installed'; refresh()">‹ Installed</button>
        <input v-model="query" class="field pill grow" :placeholder="`Search ${label.many.toLowerCase()}..`" />
        <Dropdown v-model="category" :options="categoryOptions" class="w140" searchable />
        <Dropdown v-model="sort" :options="sortOptions" class="w110" />
        <Dropdown v-model="source" :options="sourceOptions" class="w120" />
      </div>
      <div class="filters">Showing {{ instance.loader === "vanilla" || kind !== "mod" ? "" : instance.loader + " · " }}{{ instance.minecraft }} compatible results · {{ formatCount(total) }} found</div>
      <VirtualList v-if="hits.length" class="list" :items="hits" :item-height="84" :gap="6" :key-of="(h) => h.id" @end="more">
        <template #default="{ item }">
          <ModItem
            :icon="item.iconUrl"
            :title="item.title"
            :subtitle="item.author ? `by ${item.author}` : ''"
            :description="item.note || item.description"
            :chips="[
              ...(item.downloads ? [`↓ ${formatCount(item.downloads)}`] : []),
              ...(chosenVersion[item.id] ? [`Ver ${chosenVersion[item.id].versionNumber}`] : item.source === 'horizon' && item.latestVersion ? [`Ver ${item.latestVersion}`] : []),
              ...item.categories.slice(0, 3),
            ]"
            :disabled="!item.installable"
            :install-label="!item.installable ? 'Native' : installedBy.has(item.id) ? 'Switch' : '+ Install'"
            :busy="busy[item.id]"
            :installed="isInstalled(item)"
            :version-label="versionLabel(item)"
            :version-done="!chosenVersion[item.id] && installedBy.has(item.id)"
            :menu="hitMenu(item)"
            @install="install(item)"
            @version="chooseVersion(item)"
          />
        </template>
        <template #footer>
          <div v-if="searching" class="empty"><span class="spinner" /></div>
        </template>
      </VirtualList>
      <div v-else-if="searching" class="empty"><span class="spinner" /></div>
      <div v-else class="empty">Nothing found.</div>
    </template>

    <Modal v-if="versionDialog" :title="versionDialog.title" :width="520" @close="versionDialog = null">
      <div class="versions scroll">
        <button v-for="v in versionList" :key="v.id" class="version" :class="{ on: chosenVersion[versionDialog.id]?.id === v.id }" @click="pickVersion(v)">
          <div class="grow">
            <div class="ellipsis"><b>{{ v.versionNumber }}</b> <span class="muted">{{ v.name !== v.versionNumber ? v.name : "" }}</span></div>
            <div class="vsub ellipsis">{{ v.loaders.join(", ") }} · {{ v.gameVersions.slice(0, 6).join(", ") }}{{ v.gameVersions.length > 6 ? "…" : "" }}</div>
          </div>
          <span v-if="installedBy.get(versionDialog.id)?.versionId === v.id" class="tag ok">Installed</span>
          <span v-else-if="chosenVersion[versionDialog.id]?.id === v.id" class="tag">Selected</span>
          <span v-if="v.channel !== 'release'" class="src">{{ v.channel }}</span>
          <span class="vdate">{{ v.date.slice(0, 10) }}</span>
        </button>
        <div v-if="versionLoading" class="empty"><span class="spinner" /></div>
        <div v-else-if="!versionList.length" class="empty">No compatible versions for {{ instance.loader }} {{ instance.minecraft }}.</div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>
.content {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 10px 14px 12px;
  gap: 8px;
}

.toolbar {
  display: flex;
  gap: 6px;
  align-items: center;
}

.w140 {
  width: 140px;
}

.w110 {
  width: 110px;
}

.w120 {
  width: 120px;
}

.filters {
  font-size: 11px;
  color: var(--text-faint);
  font-style: italic;
}

.banner {
  font-size: 11px;
  padding: 7px 10px;
  border-radius: var(--r-small);
  background: var(--panel-2);
  color: var(--text-dim);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.banner.update {
  background: var(--accent-bg);
}

.list {
  flex: 1;
  min-height: 0;
}

.file {
  height: 52px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 10px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  transition: opacity var(--t-med), background var(--t-fast);
}

.file:hover {
  background: var(--chip-hover);
}

.file.disabled {
  opacity: 0.55;
}

.file-icon {
  width: 34px;
  height: 34px;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: var(--field);
  display: grid;
  place-items: center;
  overflow: hidden;
  flex: none;
  color: var(--text-faint);
  font-weight: 600;
}

.file-icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.file-title {
  font-size: 13px;
  font-weight: 600;
}

.file-sub {
  font-size: 11px;
  color: var(--text-dim);
  font-style: italic;
}

.src {
  font-size: 9px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 2px 6px;
  border-radius: 10px;
  background: var(--panel-3);
  color: var(--text-faint);
  flex: none;
}

.src.modrinth {
  color: #5fd38d;
}

.src.curseforge {
  color: #f0884d;
}

.rm {
  min-width: 24px;
  height: 24px;
  border-radius: var(--r-small);
  color: var(--text-faint);
  font-size: 11px;
  padding: 0 4px;
  transition: background var(--t-fast), color var(--t-fast);
  flex: none;
}

.rm:hover {
  background: var(--danger-bg);
  color: var(--danger-text);
}

.rm.confirm {
  background: var(--danger-bg);
  color: var(--danger-text);
}

.versions {
  padding: 0 16px 16px;
  max-height: 60vh;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.version {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  text-align: left;
  border-radius: var(--r-row);
  background: var(--panel);
  font-size: 12px;
  transition: background var(--t-fast);
}

.version:hover {
  background: var(--chip-hover);
}

.version.on {
  background: var(--accent-bg-hover);
}

.vsub {
  font-size: 10px;
  color: var(--text-dim);
  margin-top: 2px;
}

.tag {
  font-size: 10px;
  padding: 2px 7px;
  border-radius: 10px;
  background: var(--panel-3);
  color: var(--text-dim);
  flex: none;
}

.tag.ok {
  color: var(--accent);
  background: var(--accent-bg);
}

.seg {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  flex: none;
}

.seg button {
  width: 30px;
  height: 26px;
  display: grid;
  place-items: center;
  border-radius: var(--r-small);
  color: var(--text-faint);
  transition: background var(--t-fast), color var(--t-fast);
}

.seg button:hover {
  color: var(--text);
}

.seg button.on {
  background: var(--chip-active);
  color: var(--text);
}

.seg svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
}

.grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
  grid-auto-rows: max-content;
  gap: 8px;
  align-content: start;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px;
  border-radius: var(--r-panel);
  background: var(--panel-2);
  border: 1px solid transparent;
  content-visibility: auto;
  contain-intrinsic-size: auto 100px;
  transition: opacity var(--t-med), background var(--t-fast), border-color var(--t-fast);
}

.card:hover {
  background: var(--chip-hover);
  border-color: var(--border-soft);
}

.card.disabled {
  opacity: 0.55;
}

.card.upd {
  border-color: var(--accent-border);
}

.card-top,
.card-bottom {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.card-text {
  flex: 1;
  min-width: 0;
}

.file-icon.big {
  width: 42px;
  height: 42px;
  border-radius: 8px;
}

.vdate {
  font-size: 10px;
  color: var(--text-faint);
  flex: none;
}
</style>
