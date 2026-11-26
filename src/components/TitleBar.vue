<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { icons } from "../icons";
import { navTabs as tabs } from "../nav";
import { useApp } from "../stores/app";
import { useSettings } from "../stores/settings";

const app = useApp();
const settings = useSettings();
const win = getCurrentWindow();
const maximized = ref(false);
let unlisten: (() => void) | null = null;

const edges = ["n", "s", "e", "w", "ne", "nw", "se", "sw"] as const;
const directions = {
  n: "North",
  s: "South",
  e: "East",
  w: "West",
  ne: "NorthEast",
  nw: "NorthWest",
  se: "SouthEast",
  sw: "SouthWest",
} as const;

const side = computed(() => settings.s?.appearance.nav === "side" || settings.s?.appearance.nav === "icons");

const status = computed(() => {
  const running = app.running;
  const busy = app.busy;
  const nameOf = (id: string) => app.instances.find((i) => i.id === id)?.name ?? "Minecraft";
  if (running.length === 1 && busy.length === 0) return { tone: "on", text: `Running ${nameOf(running[0].instanceId)}` };
  if (running.length > 1) return { tone: "on", text: `${running.length} instances running` };
  if (busy.length > 0) {
    const state = busy[0].state;
    const label = state === "downloading" ? "Downloading" : state === "launching" ? "Launching" : "Preparing";
    return { tone: "busy", text: `${label} ${nameOf(busy[0].instanceId)}` };
  }
  return { tone: "off", text: "Not running" };
});

async function syncMaximized() {
  maximized.value = await win.isMaximized();
}

function close() {
  if (settings.s?.general.confirm_exit) app.exitOpen = true;
  else settings.flush().finally(() => win.destroy());
}

onMounted(async () => {
  syncMaximized();
  unlisten = await win.onResized(() => syncMaximized());
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <template v-if="app.info?.platform === 'linux' && !maximized">
    <div v-for="edge in edges" :key="edge" class="resize" :class="edge" @mousedown.left.prevent="win.startResizeDragging(directions[edge])" />
  </template>
  <header class="titlebar" data-tauri-drag-region>
    <nav class="pill">
      <img :src="icons.logo" class="logo icon" alt="Zip" />
      <button
        v-for="tab in side ? [] : tabs"
        :key="tab.id"
        class="tab"
        :class="{ active: app.page === tab.id }"
        :title="tab.label"
        @click="app.go(tab.id)"
      >
        <img :src="tab.icon" class="icon" alt="" />
        <span class="tab-label">{{ tab.label }}</span>
      </button>
    </nav>
    <div class="status" :class="status.tone" data-tauri-drag-region>
      <span class="dot" />
      <span class="ellipsis">{{ status.text }}</span>
    </div>
    <div class="spacer" data-tauri-drag-region />
    <div class="pill controls">
      <button class="chip-btn" :title="maximized ? 'Restore' : 'Maximize'" @click="win.toggleMaximize()">
        <img :src="icons.expand" class="icon" alt="" />
      </button>
      <button class="chip-btn" title="Minimize" @click="win.minimize()">
        <img :src="icons.minimize" class="icon small" alt="" />
      </button>
      <button class="chip-btn close" title="Close" @click="close">
        <img :src="icons.close" class="icon" alt="" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.resize {
  position: fixed;
  z-index: 1000;
}

.resize.n,
.resize.s {
  left: 8px;
  right: 8px;
  height: 5px;
  cursor: ns-resize;
}

.resize.n {
  top: 0;
}

.resize.s {
  bottom: 0;
}

.resize.e,
.resize.w {
  top: 8px;
  bottom: 8px;
  width: 5px;
  cursor: ew-resize;
}

.resize.e {
  right: 0;
}

.resize.w {
  left: 0;
}

.resize.ne,
.resize.nw,
.resize.se,
.resize.sw {
  width: 10px;
  height: 10px;
}

.resize.ne {
  top: 0;
  right: 0;
  cursor: nesw-resize;
}

.resize.sw {
  bottom: 0;
  left: 0;
  cursor: nesw-resize;
}

.resize.nw {
  top: 0;
  left: 0;
  cursor: nwse-resize;
}

.resize.se {
  bottom: 0;
  right: 0;
  cursor: nwse-resize;
}

.titlebar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px;
  height: 53px;
  flex: none;
}

.pill {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 39px;
  padding: 0 6px 0 8px;
  background: var(--panel);
  border: 1px solid var(--panel-border);
  border-radius: 14px;
  box-shadow: inset 0 1px 0 var(--hairline), var(--shadow);
}

.logo {
  width: 24px;
  height: 24px;
  margin-right: 4px;
  filter: drop-shadow(0 1px 3px rgba(0, 0, 0, 0.4));
}

.tab {
  height: 27px;
  min-width: 27px;
  display: flex;
  align-items: center;
  padding: 0 6px;
  border-radius: var(--r-chip);
  background: var(--chip);
  border: 1px solid transparent;
  color: var(--text-dim);
  transition: background var(--t-fast) var(--ease), transform var(--t-fast) var(--ease), color var(--t-fast) var(--ease), border-color var(--t-fast) var(--ease), box-shadow var(--t-fast) var(--ease);
}

.tab .icon {
  width: 15px;
  height: 15px;
  transition: filter var(--t-fast);
}

.tab:hover {
  background: var(--chip-hover);
  color: var(--text);
}

.tab:active {
  transform: scale(0.94);
}

.tab.active {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
  color: var(--accent);
  box-shadow: 0 0 12px color-mix(in srgb, var(--accent) 18%, transparent), inset 0 1px 0 var(--hairline);
}

.tab.active .icon {
  filter: var(--icon-filter) drop-shadow(0 0 4px var(--accent-glow));
}

.tab-label {
  max-width: 0;
  overflow: hidden;
  white-space: nowrap;
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
  opacity: 0;
  transition: max-width var(--t-slow) var(--ease), opacity var(--t-med) var(--ease), margin var(--t-slow) var(--ease);
}

.tab.active .tab-label {
  max-width: 80px;
  opacity: 1;
  margin: 0 4px 0 7px;
}

.status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  min-width: 0;
  max-width: 40%;
  margin-left: 2px;
}

.dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--danger);
  box-shadow: 0 0 8px var(--danger-glow);
  flex: none;
  transition: background var(--t-med) var(--ease), box-shadow var(--t-med) var(--ease);
}

.status.on {
  color: var(--text);
}

.status.on .dot {
  background: var(--running);
  box-shadow: 0 0 8px color-mix(in srgb, var(--running) 55%, transparent);
}

.status.busy .dot {
  background: var(--warn);
  box-shadow: 0 0 8px color-mix(in srgb, var(--warn) 55%, transparent);
  animation: pulse 1.2s ease-in-out infinite;
}

@keyframes pulse {
  50% {
    opacity: 0.4;
  }
}

.spacer {
  flex: 1;
  height: 100%;
}

.controls {
  padding: 0 6px;
}

.controls .icon {
  width: 14px;
  height: 14px;
}

.controls .icon.small {
  width: 13px;
  height: 13px;
}

.chip-btn.close:hover {
  background: var(--danger-bg-hover);
  border-color: color-mix(in srgb, var(--danger) 40%, transparent);
  box-shadow: 0 0 10px var(--danger-glow);
}
</style>
