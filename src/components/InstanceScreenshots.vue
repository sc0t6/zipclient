<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import Modal from "./Modal.vue";
import { api, errorText, type Instance, type Screenshot } from "../api";
import { useNotify } from "../stores/notify";

const props = defineProps<{ instance: Instance }>();
const notify = useNotify();
const shots = ref<Screenshot[]>([]);
const loaded = ref(false);
const open = ref(-1);
const deleting = ref<Screenshot | null>(null);

const current = computed(() => shots.value[open.value] ?? null);
const src = (s: Screenshot) => `${convertFileSrc(s.path)}?v=${s.modified}`;

async function load() {
  try {
    shots.value = await api.screenshots(props.instance.id);
  } catch (e) {
    notify.error("Could not read screenshots", errorText(e));
  }
  loaded.value = true;
}

function step(d: number) {
  if (open.value < 0 || !shots.value.length) return;
  open.value = (open.value + d + shots.value.length) % shots.value.length;
}

function onKey(e: KeyboardEvent) {
  if (open.value < 0 || deleting.value) return;
  if (e.key === "Escape") open.value = -1;
  else if (e.key === "ArrowRight") step(1);
  else if (e.key === "ArrowLeft") step(-1);
  else if (e.key === "Delete" && current.value) deleting.value = current.value;
}

async function remove() {
  const s = deleting.value;
  deleting.value = null;
  if (!s) return;
  try {
    await api.deleteScreenshot(props.instance.id, s.name);
    const i = shots.value.indexOf(s);
    shots.value = shots.value.filter((x) => x !== s);
    if (open.value >= 0) open.value = shots.value.length ? Math.min(i, shots.value.length - 1) : -1;
  } catch (e) {
    notify.error("Could not delete screenshot", errorText(e));
  }
}

async function copy(s: Screenshot) {
  try {
    const blob = await (await fetch(src(s))).blob();
    await navigator.clipboard.write([new ClipboardItem({ [blob.type || "image/png"]: blob })]);
    notify.success("Screenshot copied", s.name);
  } catch (e) {
    notify.error("Could not copy screenshot", errorText(e));
  }
}

function day(t: number) {
  return new Date(t * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

const groups = computed(() => {
  const map = new Map<string, { shot: Screenshot; index: number }[]>();
  shots.value.forEach((shot, index) => {
    const key = day(shot.modified);
    map.set(key, [...(map.get(key) ?? []), { shot, index }]);
  });
  return [...map.entries()];
});

onMounted(() => {
  load();
  window.addEventListener("keydown", onKey);
});
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="pane">
    <div class="toolbar">
      <span class="muted grow">{{ shots.length }} screenshot{{ shots.length === 1 ? "" : "s" }} · press F2 in game to take one</span>
      <button class="btn" @click="load">Refresh</button>
      <button class="btn" @click="api.openInstanceFolder(instance.id, 'screenshots')">Open folder</button>
    </div>
    <div v-if="shots.length" class="scroll grid-wrap">
      <section v-for="[label, items] in groups" :key="label">
        <h4>{{ label }}</h4>
        <div class="grid">
          <button v-for="{ shot, index } in items" :key="shot.name" class="thumb" :title="shot.name" @click="open = index">
            <img :src="src(shot)" loading="lazy" decoding="async" alt="" />
            <span class="time">{{ new Date(shot.modified * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) }}</span>
          </button>
        </div>
      </section>
    </div>
    <div v-else-if="loaded" class="empty">No screenshots yet. Press F2 in game and they show up here.</div>

    <Teleport to="body">
      <div v-if="current" class="lightbox" @mousedown.self="open = -1">
        <img :src="src(current)" alt="" @click="step(1)" />
        <button class="nav prev" title="Previous" @click="step(-1)">‹</button>
        <button class="nav next" title="Next" @click="step(1)">›</button>
        <div class="bar">
          <span class="ellipsis grow">{{ current.name }} · {{ open + 1 }} / {{ shots.length }} · {{ (current.size / 1048576).toFixed(1) }} MB</span>
          <button class="btn" @click="copy(current)">Copy</button>
          <button class="btn" @click="api.openInstanceFolder(instance.id, 'screenshots')">Show in folder</button>
          <button class="btn danger" @click="deleting = current">Delete</button>
          <button class="btn" @click="open = -1">Close</button>
        </div>
      </div>
      <Modal v-if="deleting" title="Delete screenshot?" :width="420" @close="deleting = null">
        <div class="confirm">
          <p><b>{{ deleting.name }}</b> will be deleted from the screenshots folder. This cannot be undone.</p>
          <div class="row end">
            <button class="btn" @click="deleting = null">Cancel</button>
            <button class="btn danger" @click="remove">Delete</button>
          </div>
        </div>
      </Modal>
    </Teleport>
  </div>
</template>

<style scoped>
.pane {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: calc(12px * var(--text-scale, 1));
}

.grid-wrap {
  flex: 1;
  min-height: 0;
}

h4 {
  font-size: calc(11px * var(--text-scale, 1));
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-faint);
  margin: 8px 0 6px;
  font-weight: 600;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 8px;
}

.thumb {
  position: relative;
  aspect-ratio: 16 / 9;
  border-radius: var(--r-row);
  overflow: hidden;
  background: var(--panel-3);
  border: 1px solid var(--border-soft);
  transition: border-color var(--t-fast), transform var(--t-fast) var(--ease);
}

.thumb:hover {
  border-color: var(--accent-border);
  transform: translateY(-2px);
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.time {
  position: absolute;
  right: 6px;
  bottom: 6px;
  font-size: calc(10px * var(--text-scale, 1));
  padding: 2px 6px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.6);
  color: #fff;
}

.lightbox {
  position: fixed;
  inset: 53px 0 0;
  z-index: 99;
  background: rgba(0, 0, 0, 0.88);
  display: grid;
  place-items: center;
  padding: 40px 70px 80px;
}

.lightbox img {
  max-width: 100%;
  max-height: calc(100vh - 190px);
  border-radius: var(--r-row);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
  cursor: pointer;
}

.nav {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 44px;
  height: 64px;
  font-size: 34px;
  color: #fff;
  border-radius: var(--r-row);
  background: rgba(255, 255, 255, 0.08);
}

.nav:hover {
  background: rgba(255, 255, 255, 0.18);
}

.prev {
  left: 14px;
}

.next {
  right: 14px;
}

.bar {
  position: absolute;
  left: 50%;
  bottom: 18px;
  transform: translateX(-50%);
  width: min(900px, calc(100vw - 40px));
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border-radius: 999px;
  background: var(--panel);
  border: 1px solid var(--border-soft);
  font-size: calc(12px * var(--text-scale, 1));
}

.confirm {
  padding: 0 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.empty {
  flex: 1;
  display: grid;
  place-items: center;
  color: var(--text-dim);
  font-size: calc(12px * var(--text-scale, 1));
}
</style>
