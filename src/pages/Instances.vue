<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import Dropdown from "../components/Dropdown.vue";
import InstanceIcon from "../components/InstanceIcon.vue";
import IconPicker from "../components/IconPicker.vue";
import Modal from "../components/Modal.vue";
import { api, errorText, formatPlaytime, loaderNames, type Instance } from "../api";
import { importFile, importing } from "../importer";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";
import { useSettings } from "../stores/settings";

defineOptions({ name: "InstancesPage" });

const settings = useSettings();
const layout = computed(() => settings.ui.instances);

const InstanceSettings = defineAsyncComponent(() => import("../components/InstanceSettings.vue"));
const ContentManager = defineAsyncComponent(() => import("../components/ContentManager.vue"));
const LogView = defineAsyncComponent(() => import("../components/LogView.vue"));

const app = useApp();
const notify = useNotify();

const search = ref("");
const sort = ref("recent");
const grouped = ref(true);
const collapsed = ref<Record<string, boolean>>({});
const menuOpen = ref(false);
const importMenu = ref(false);
const deleteOpen = ref(false);
const iconOpen = ref(false);
const renaming = ref(false);
const renameValue = ref("");
const working = ref("");

const sortOptions = [
  { value: "recent", label: "Recently played" },
  { value: "name", label: "Name" },
  { value: "playtime", label: "Playtime" },
  { value: "version", label: "Minecraft version" },
  { value: "created", label: "Newest" },
];

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  const list = app.instances.filter((i) => !q || i.name.toLowerCase().includes(q) || i.minecraft.includes(q) || i.loader.includes(q) || i.group.toLowerCase().includes(q));
  const sorters: Record<string, (a: Instance, b: Instance) => number> = {
    recent: (a, b) => b.last_played - a.last_played || a.name.localeCompare(b.name),
    name: (a, b) => a.name.localeCompare(b.name),
    playtime: (a, b) => b.playtime - a.playtime,
    version: (a, b) => b.minecraft.localeCompare(a.minecraft, undefined, { numeric: true }),
    created: (a, b) => b.created - a.created,
  };
  return [...list].sort(sorters[sort.value]);
});

const groups = computed(() => {
  if (!grouped.value) return [{ name: "", items: filtered.value }];
  const map = new Map<string, Instance[]>();
  for (const i of filtered.value) {
    const key = i.group || "Ungrouped";
    map.set(key, [...(map.get(key) ?? []), i]);
  }
  return [...map.entries()].sort((a, b) => (a[0] === "Ungrouped" ? 1 : b[0] === "Ungrouped" ? -1 : a[0].localeCompare(b[0]))).map(([name, items]) => ({ name, items }));
});

const selected = computed(() => app.instances.find((i) => i.id === app.selectedInstance) ?? null);
const game = computed(() => (selected.value ? app.games[selected.value.id] : undefined));
const progress = computed(() => (selected.value ? app.progress[selected.value.id] : undefined));
const tabs = computed(() => [
  { id: "overview", label: "Overview" },
  { id: "mods", label: "Mods" },
  { id: "resourcepacks", label: "Resource packs" },
  ...(selected.value?.loader === "horizon" ? [] : [{ id: "shaders", label: "Shaders" }]),
  { id: "settings", label: "Settings" },
  { id: "logs", label: "Logs" },
]);

const playLabel = computed(() => {
  const s = game.value?.state;
  if (s === "running") return "Stop";
  if (s === "launching") return "Launching";
  if (s === "downloading" || s === "preparing") return progress.value?.totalFiles ? `${Math.round((progress.value.fraction ?? 0) * 100)}%` : "Preparing";
  return "Play";
});

watch(
  () => app.instances,
  (list) => {
    if (!list.find((i) => i.id === app.selectedInstance)) app.selectedInstance = list[0]?.id ?? "";
  },
  { immediate: true },
);

function select(id: string) {
  app.selectedInstance = id;
  renaming.value = false;
}

function play() {
  const i = selected.value;
  if (!i) return;
  const s = game.value?.state;
  if (s === "running") app.kill(i.id);
  else if (!s) {
    if (!app.accounts.length) {
      notify.info("Add an account first", "Sign in with Microsoft or add an offline profile.");
      app.loginOpen = true;
      return;
    }
    app.launch(i.id);
  }
}

async function run(label: string, task: () => Promise<void>) {
  working.value = label;
  menuOpen.value = false;
  try {
    await task();
  } catch (e) {
    notify.error(`${label} failed`, errorText(e));
  } finally {
    working.value = "";
  }
}

function startRename() {
  if (!selected.value) return;
  renameValue.value = selected.value.name;
  renaming.value = true;
  menuOpen.value = false;
}

async function commitRename() {
  const i = selected.value;
  if (!i || !renaming.value) return;
  renaming.value = false;
  if (renameValue.value.trim() === i.name) return;
  await run("Rename", async () => app.replaceInstance(await api.updateInstance(i.id, { name: renameValue.value })));
}

const duplicate = () =>
  run("Duplicate", async () => {
    const copy = await api.duplicateInstance(selected.value!.id);
    app.replaceInstance(copy);
    app.selectedInstance = copy.id;
    notify.success("Instance duplicated", copy.name);
  });

const exportInstance = () =>
  run("Export", async () => {
    const i = selected.value!;
    const path = await save({ defaultPath: `${i.name}.zip`, filters: [{ name: "Zip instance", extensions: ["zip"] }] });
    if (!path) return;
    await api.exportInstance(i.id, path);
    notify.success("Instance exported", path);
  });

const repair = () =>
  run("Repair", async () => {
    const i = selected.value!;
    app.replaceInstance(await api.installInstance(i.id, true));
    notify.success("Repair complete", `${i.name} files were verified and restored.`);
  });

const install = () =>
  run("Install", async () => {
    const i = selected.value!;
    app.replaceInstance(await api.installInstance(i.id, false));
    notify.success("Minecraft installation completed", `${i.name} is ready to play.`);
  });

const changeIcon = () =>
  run("Icon", async () => {
    const path = await open({ multiple: false, filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg"] }] });
    if (typeof path !== "string") return;
    app.replaceInstance(await api.setInstanceIcon(selected.value!.id, path));
  });

const clearIcon = () => run("Icon", async () => app.replaceInstance(await api.setInstanceIcon(selected.value!.id, null)));

const pickIcon = (value: string) =>
  run("Icon", async () => app.replaceInstance(value ? await api.updateInstance(selected.value!.id, { icon: value }) : await api.setInstanceIcon(selected.value!.id, null)));

function uploadIcon() {
  iconOpen.value = false;
  changeIcon();
}

async function confirmDelete() {
  const i = selected.value;
  deleteOpen.value = false;
  if (!i) return;
  await run("Delete", async () => {
    await api.deleteInstance(i.id);
    await app.refreshInstances();
    notify.info("Instance deleted", i.name);
  });
}

function lastPlayed(i: Instance) {
  return i.last_played ? new Date(i.last_played * 1000).toLocaleString() : "Never";
}
</script>

<template>
  <div class="instances" :class="[`side-${layout.side}`, `width-${layout.width}`]">
    <aside class="side panel">
      <input v-model="search" class="field pill" placeholder="Search instances.." />
      <div class="row tools">
        <Dropdown v-model="sort" :options="sortOptions" class="grow" />
        <button class="chip-btn group-btn" :class="{ on: grouped }" title="Group by folder" @click="grouped = !grouped">
          <svg viewBox="0 0 16 16"><path d="M2 4h5l1 1.5h6V13H2z" fill="none" stroke="currentColor" stroke-width="1.3" /></svg>
        </button>
      </div>
      <div class="list scroll">
        <template v-for="g in groups" :key="g.name">
          <button v-if="g.name" class="group-head" @click="collapsed[g.name] = !collapsed[g.name]">
            <svg :class="{ closed: collapsed[g.name] }" viewBox="0 0 10 6"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" /></svg>
            <span class="ellipsis">{{ g.name }}</span>
            <span class="count">{{ g.items.length }}</span>
          </button>
          <template v-if="!collapsed[g.name]">
            <button v-for="i in g.items" :key="i.id" class="item" :class="{ on: i.id === app.selectedInstance }" @click="select(i.id)">
              <InstanceIcon :instance="i" />
              <div class="grow">
                <div class="item-name ellipsis">{{ i.name }}</div>
                <div class="item-sub ellipsis">{{ i.loader === "vanilla" ? "Vanilla" : loaderNames[i.loader] }} {{ i.minecraft }}</div>
              </div>
              <span v-if="app.games[i.id]" class="live" :class="app.games[i.id].state" />
            </button>
          </template>
        </template>
        <div v-if="!app.instances.length" class="empty">No instances yet.</div>
        <div v-else-if="!filtered.length" class="empty">No matches.</div>
      </div>
      <div class="side-actions">
        <button class="btn primary grow" @click="app.createOpen = true">+ New instance</button>
        <div class="import-wrap">
          <button class="btn" :disabled="importing" @click="importMenu = !importMenu">
            <span v-if="importing" class="spinner" />
            Import
          </button>
          <Transition name="pop">
            <div v-if="importMenu" class="pop-menu" @mouseleave="importMenu = false">
              <button @click="importMenu = false; importFile('curseforge')">CurseForge .zip</button>
              <button @click="importMenu = false; importFile('prism')">Prism Launcher .zip</button>
              <button @click="importMenu = false; importFile('modrinth')">Modrinth .mrpack</button>
              <button @click="importMenu = false; importFile('any')">Zip export .zip</button>
            </div>
          </Transition>
        </div>
      </div>
    </aside>

    <section v-if="selected" class="detail panel">
      <header class="head">
        <button class="icon-btn" title="Change icon" @click="iconOpen = true" @contextmenu.prevent="clearIcon">
          <InstanceIcon :instance="selected" :size="52" />
        </button>
        <div class="grow">
          <input v-if="renaming" v-model="renameValue" class="field rename" maxlength="64" autofocus @keydown.enter="commitRename" @keydown.esc="renaming = false" @blur="commitRename" />
          <h2 v-else class="ellipsis" title="Double-click to rename" @dblclick="startRename">{{ selected.name }}</h2>
          <div class="meta">
            {{ selected.loader === "vanilla" ? "Vanilla" : `${loaderNames[selected.loader]} ${selected.loader_version}` }} · Minecraft {{ selected.minecraft }}
            <template v-if="selected.modpack"> · {{ selected.modpack.name }} {{ selected.modpack.version }}</template>
          </div>
        </div>
        <button class="btn play" :class="{ danger: game?.state === 'running', primary: game?.state !== 'running' }" :disabled="!!game && game.state !== 'running'" @click="play">
          <span v-if="game && game.state !== 'running'" class="spinner" />
          {{ playLabel }}
        </button>
        <button class="chip-btn big" title="Open folder" @click="api.openInstanceFolder(selected.id)">
          <svg viewBox="0 0 16 16"><path d="M2 4h5l1 1.5h6V13H2z" fill="none" stroke="currentColor" stroke-width="1.3" /></svg>
        </button>
        <div class="menu-wrap">
          <button class="chip-btn big" title="More" @click="menuOpen = !menuOpen">
            <svg viewBox="0 0 16 16"><circle cx="8" cy="3.5" r="1.4" /><circle cx="8" cy="8" r="1.4" /><circle cx="8" cy="12.5" r="1.4" /></svg>
          </button>
          <Transition name="pop">
            <div v-if="menuOpen" class="pop-menu right" @mouseleave="menuOpen = false">
              <button @click="startRename">Rename</button>
              <button @click="duplicate">Duplicate</button>
              <button @click="exportInstance">Export</button>
              <button :disabled="!!game" @click="install">Download files now</button>
              <button :disabled="!!game" @click="repair">Repair installation</button>
              <button @click="menuOpen = false; iconOpen = true">Change icon</button>
              <button v-if="selected.icon" @click="clearIcon">Reset icon</button>
              <button class="danger" :disabled="!!game" @click="menuOpen = false; deleteOpen = true">Delete</button>
            </div>
          </Transition>
        </div>
      </header>
      <div v-if="working || (game && progress)" class="working">
        <span class="spinner" />
        <span class="ellipsis grow">{{ progress?.phase || `${working}..` }}</span>
        <span v-if="progress?.totalFiles">{{ progress.doneFiles }}/{{ progress.totalFiles }}</span>
        <div class="bar"><span :style="{ transform: `scaleX(${progress?.fraction ?? 0})` }" /></div>
      </div>
      <nav class="tabs">
        <button v-for="t in tabs" :key="t.id" class="tab" :class="{ on: app.instanceTab === t.id }" @click="app.instanceTab = t.id">{{ t.label }}</button>
      </nav>
      <div class="tab-body">
        <div v-if="app.instanceTab === 'overview'" class="overview scroll">
          <div class="stats">
            <div class="stat"><span class="label">Playtime</span><strong>{{ formatPlaytime(selected.playtime) }}</strong></div>
            <div class="stat"><span class="label">Launches</span><strong>{{ selected.launches }}</strong></div>
            <div class="stat"><span class="label">Last played</span><strong class="small">{{ lastPlayed(selected) }}</strong></div>
            <div class="stat"><span class="label">Created</span><strong class="small">{{ new Date(selected.created * 1000).toLocaleDateString() }}</strong></div>
          </div>
          <h3>Launch history</h3>
          <div class="history">
            <div v-for="h in selected.history" :key="h.started" class="hist">
              <span>{{ new Date(h.started * 1000).toLocaleString() }}</span>
              <span class="muted">{{ formatPlaytime(h.duration) }}</span>
              <span :class="h.exit_code === 0 ? 'ok' : h.exit_code === null ? 'muted' : 'bad'">{{ h.exit_code === 0 ? "Exited normally" : h.exit_code === null ? "Stopped from Zip" : `Exit code ${h.exit_code}` }}</span>
            </div>
            <div v-if="!selected.history.length" class="empty">Not played yet.</div>
          </div>
        </div>
        <ContentManager
          v-else-if="['mods', 'resourcepacks', 'shaders'].includes(app.instanceTab)"
          :key="selected.id + app.instanceTab"
          :instance="selected"
          :kind="app.instanceTab === 'mods' ? 'mod' : app.instanceTab === 'shaders' ? 'shader' : 'resourcepack'"
        />
        <InstanceSettings v-else-if="app.instanceTab === 'settings'" :key="selected.id" :instance="selected" />
        <LogView v-else-if="app.instanceTab === 'logs'" :key="selected.id" :instance="selected" />
      </div>
    </section>
    <section v-else class="detail panel blank">
      <div class="blank-inner">
        <h2>No instance selected</h2>
        <p>Create a new instance or import a modpack to get started.</p>
        <div class="row">
          <button class="btn primary" @click="app.createOpen = true">+ New instance</button>
          <button class="btn" @click="importFile('any')">Import</button>
        </div>
      </div>
    </section>

    <Modal v-if="iconOpen && selected" title="Change icon" :width="440" @close="iconOpen = false">
      <div class="confirm-body">
        <IconPicker :model-value="selected.icon.startsWith('preset:') ? selected.icon : selected.icon ? 'custom' : ''" :letter="selected.name" @update:model-value="pickIcon" />
        <div class="row end">
          <button class="btn" @click="uploadIcon">Upload image…</button>
          <button class="btn primary" @click="iconOpen = false">Done</button>
        </div>
      </div>
    </Modal>
    <Modal v-if="deleteOpen && selected" title="Delete instance?" :width="420" @close="deleteOpen = false">
      <div class="confirm-body">
        <p>
          <b>{{ selected.name }}</b> and everything in its folder (worlds, screenshots, configs, mods) will be permanently deleted. This cannot be undone.
        </p>
        <div class="row end">
          <button class="btn" @click="deleteOpen = false">Cancel</button>
          <button class="btn danger" @click="confirmDelete">Delete permanently</button>
        </div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>
.instances {
  --list: 236px;
  height: 100%;
  display: grid;
  grid-template-columns: var(--list) 1fr;
  gap: 11px;
  padding: 12px 11px 12px 12px;
}

.instances.width-narrow {
  --list: 196px;
}

.instances.width-wide {
  --list: 300px;
}

.instances.side-right {
  grid-template-columns: 1fr var(--list);
}

.instances.side-right > .side {
  order: 2;
}

.side {
  border-radius: 13px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}

.tools {
  gap: 6px;
}

.group-btn {
  width: 30px;
  height: 30px;
  color: var(--text-dim);
}

.group-btn.on {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
  color: var(--accent);
}

.group-btn svg,
.chip-btn svg {
  width: 15px;
  height: 15px;
  fill: currentColor;
}

.group-btn svg path,
.chip-btn svg path {
  fill: none;
}

.list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0 -4px;
  padding: 0 4px;
}

.group-head {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-dim);
  padding: 6px 4px 2px;
  font-style: italic;
}

.group-head svg {
  width: 8px;
  height: 5px;
  transition: transform var(--t-med) var(--ease);
}

.group-head svg.closed {
  transform: rotate(-90deg);
}

.count {
  margin-left: auto;
  color: var(--text-faint);
}

.item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 7px;
  border-radius: var(--r-row);
  text-align: left;
  background: var(--panel-2);
  border: 1px solid transparent;
  position: relative;
  transition: background var(--t-fast), border-color var(--t-fast), transform var(--t-fast) var(--ease);
  flex: none;
}

.item:hover {
  background: var(--chip-hover);
  transform: translateX(1px);
}

.item.on {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
  box-shadow: inset 2px 0 0 var(--accent);
}

.item.on .item-name {
  color: var(--accent);
}

.item-name {
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
}

.item-sub {
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-dim);
  font-style: italic;
}

.live {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--warn);
  box-shadow: 0 0 7px color-mix(in srgb, var(--warn) 60%, transparent);
  flex: none;
}

.live.running {
  background: var(--running);
  box-shadow: 0 0 7px var(--accent-glow);
}

.side-actions {
  display: flex;
  gap: 6px;
}

.import-wrap,
.menu-wrap {
  position: relative;
}

.pop-menu {
  position: absolute;
  bottom: 36px;
  right: 0;
  min-width: 180px;
  background: var(--panel);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  padding: 4px;
  z-index: 20;
  box-shadow: var(--shadow-pop), inset 0 1px 0 var(--hairline);
}

.pop-menu.right {
  top: 34px;
  bottom: auto;
}

.pop-menu button {
  width: 100%;
  text-align: left;
  padding: 7px 9px;
  border-radius: var(--r-small);
  font-size: calc(12px * var(--text-scale, 1));
}

.pop-menu button:hover:not(:disabled) {
  background: var(--chip-hover);
}

.pop-menu button:disabled {
  opacity: 0.4;
}

.pop-menu .danger {
  color: var(--danger-text);
}

.detail {
  border-radius: 13px;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px 8px;
}

.icon-btn {
  border-radius: 8px;
  transition: transform var(--t-fast) var(--ease-spring), box-shadow var(--t-fast);
}

.icon-btn:hover {
  transform: scale(1.05);
  box-shadow: 0 0 0 2px var(--accent-border), 0 0 14px var(--accent-glow);
}

h2 {
  margin: 0;
  font-size: calc(20px * var(--text-scale, 1));
  font-weight: 600;
}

.rename {
  height: 30px;
  font-size: calc(16px * var(--text-scale, 1));
}

.meta {
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  font-style: italic;
  margin-top: 2px;
}

.play {
  height: 36px;
  min-width: 96px;
  font-size: calc(15px * var(--text-scale, 1));
  font-weight: 700;
  letter-spacing: 0.01em;
  border-radius: var(--r-row);
}

.play.primary {
  box-shadow: 0 2px 12px var(--accent-glow), inset 0 1px 0 rgba(255, 255, 255, 0.25);
}

.chip-btn.big {
  width: 34px;
  height: 34px;
  background: var(--panel-2);
}

.working {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 14px 6px;
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-dim);
  position: relative;
  padding-bottom: 5px;
}

.bar {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 2px;
  background: var(--panel-3);
  border-radius: 2px;
  overflow: hidden;
}

.bar span {
  display: block;
  height: 100%;
  background: var(--accent-grad);
  box-shadow: 0 0 8px var(--accent-glow);
  transform-origin: left;
  transition: transform var(--t-slow) var(--ease);
}

.tabs {
  display: flex;
  gap: 2px;
  padding: 0 12px;
  border-bottom: 1px solid var(--border-soft);
  flex: none;
}

.tab {
  padding: 8px 10px;
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  position: relative;
  transition: color var(--t-fast);
}

.tab::after {
  content: "";
  position: absolute;
  left: 8px;
  right: 8px;
  bottom: -1px;
  height: 2px;
  border-radius: 2px;
  background: var(--accent);
  transform: scaleX(0);
  transition: transform var(--t-med) var(--ease);
}

.tab:hover {
  color: var(--text);
}

.tab.on {
  color: var(--text);
  font-weight: 600;
}

.tab.on::after {
  transform: scaleX(1);
  box-shadow: 0 0 8px var(--accent-glow);
}

.tab-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.overview {
  padding: 14px;
}

.stats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}

.stat {
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  box-shadow: inset 0 1px 0 var(--hairline);
  transition: border-color var(--t-fast), transform var(--t-fast) var(--ease);
}

.stat:hover {
  border-color: var(--accent-border);
  transform: translateY(-1px);
}

.stat .label {
  font-size: calc(10px * var(--text-scale, 1));
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-faint);
}

.stat strong {
  font-size: calc(18px * var(--text-scale, 1));
}

.stat strong.small {
  font-size: calc(12px * var(--text-scale, 1));
}

h3 {
  font-size: calc(13px * var(--text-scale, 1));
  font-weight: 600;
  margin: 16px 0 8px;
}

.history {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.hist {
  display: grid;
  grid-template-columns: 1fr 80px 140px;
  gap: 8px;
  font-size: calc(12px * var(--text-scale, 1));
  padding: 6px 10px;
  border-radius: var(--r-small);
  background: var(--panel-2);
  border: 1px solid transparent;
  transition: border-color var(--t-fast), background var(--t-fast);
}

.hist:hover {
  background: var(--chip-hover);
  border-color: var(--border-soft);
}

.ok {
  color: var(--running);
}

.bad {
  color: var(--danger);
}

.blank {
  display: grid;
  place-items: center;
}

.blank-inner {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
}

.blank-inner p {
  color: var(--text-dim);
  margin: 0 0 8px;
}

.confirm-body {
  padding: 0 20px 18px;
}

.confirm-body .picker {
  margin: 4px 0 18px;
}

.confirm-body p {
  color: var(--text-dim);
  line-height: 1.5;
  margin: 0 0 14px;
}

.end {
  justify-content: flex-end;
}
</style>
