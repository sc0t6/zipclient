<script setup lang="ts">
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, shallowRef, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { IdleAnimation, SkinViewer } from "skinview3d";
import { api, errorText, type SkinEntry } from "../api";
import { bodyFront, looksSlim, texture } from "../skin";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";
import { useSettings } from "../stores/settings";

defineOptions({ name: "SkinsPage" });

interface Card {
  id: string;
  name: string;
  variant: string;
  dataUrl: string;
  thumb: string;
  original: boolean;
}

const app = useApp();
const notify = useNotify();
const settings = useSettings();
const layout = computed(() => settings.ui.skins);

const canvas = ref<HTMLCanvasElement | null>(null);
const stage = ref<HTMLElement | null>(null);
const viewer = shallowRef<SkinViewer | null>(null);
const library = ref<SkinEntry[]>([]);
const selectedMap = ref<Record<string, string>>({});
const original = ref<Card | null>(null);
const thumbs = ref<Record<string, string>>({});
const previewId = ref("original");
const layers = ref(true);
const animate = ref(false);
const autoRotate = ref(false);
const wearing = ref("");
const filter = ref("");
let idleTimer = 0;
let resizeObserver: ResizeObserver | null = null;

const account = computed(() => app.activeAccount);
const cards = computed<Card[]>(() => [
  ...(original.value ? [original.value] : []),
  ...library.value.map((s) => ({ ...s, thumb: thumbs.value[s.id + s.variant] ?? "", original: false })),
]);
const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return q ? cards.value.filter((c) => c.name.toLowerCase().includes(q)) : cards.value;
});
const preview = computed(() => cards.value.find((c) => c.id === previewId.value) ?? cards.value[0] ?? null);
const selectedId = computed(() => {
  const a = account.value;
  if (!a) return "";
  const local = selectedMap.value[a.id];
  if (local && library.value.some((s) => s.id === local)) return local;
  return "original";
});

async function loadOriginal() {
  const a = account.value;
  try {
    const dataUrl = await texture(a?.skinUrl, a?.skinVariant);
    const variant = a?.skinVariant === "slim" ? "slim" : "classic";
    original.value = { id: "original", name: "Profile skin", variant, dataUrl, thumb: await bodyFront(dataUrl, variant === "slim"), original: true };
  } catch {
    original.value = null;
  }
}

async function loadLibrary() {
  const lib = await api.skins();
  library.value = lib.skins;
  selectedMap.value = lib.selected;
  for (const s of lib.skins) {
    const key = s.id + s.variant;
    if (!thumbs.value[key]) bodyFront(s.dataUrl, s.variant === "slim").then((t) => (thumbs.value = { ...thumbs.value, [key]: t }));
  }
}

function wake() {
  const v = viewer.value;
  if (!v) return;
  v.renderPaused = false;
  window.clearTimeout(idleTimer);
  if (!animate.value && !autoRotate.value) idleTimer = window.setTimeout(() => viewer.value && (viewer.value.renderPaused = true), 2500);
}

function applyPreview() {
  const v = viewer.value;
  const p = preview.value;
  if (!v || !p) return;
  v.loadSkin(p.dataUrl, { model: p.variant === "slim" ? "slim" : "default" }).then(() => {
    v.playerObject.skin.setOuterLayerVisible(layers.value);
    wake();
  });
}

function createViewer() {
  if (!canvas.value || viewer.value) return;
  const rect = stage.value!.getBoundingClientRect();
  const v = new SkinViewer({ canvas: canvas.value, width: Math.max(rect.width, 50), height: Math.max(rect.height, 50), pixelRatio: Math.min(window.devicePixelRatio, 2) });
  v.fov = 50;
  v.zoom = 0.6;
  v.controls.enableRotate = true;
  v.controls.enableZoom = true;
  v.controls.enablePan = false;
  v.playerObject.rotation.y = 0.45;
  viewer.value = v;
  resizeObserver = new ResizeObserver(() => {
    const r = stage.value?.getBoundingClientRect();
    if (r && viewer.value) {
      viewer.value.setSize(r.width, r.height);
      wake();
    }
  });
  resizeObserver.observe(stage.value!);
  applyPreview();
}

async function importSkin() {
  const path = await open({ multiple: false, filters: [{ name: "Minecraft skin", extensions: ["png"] }] });
  if (typeof path !== "string") return;
  try {
    let entry = await api.importSkin(path, "", "classic");
    if (await looksSlim(entry.dataUrl)) {
      await api.setSkinVariant(entry.id, "slim");
      entry = { ...entry, variant: "slim" };
    }
    await loadLibrary();
    previewId.value = entry.id;
    notify.success("Skin imported", entry.name);
  } catch (e) {
    notify.error("Could not import skin", errorText(e));
  }
}

async function wear(card: Card) {
  const a = account.value;
  if (!a) {
    notify.info("Add an account first");
    app.loginOpen = true;
    return;
  }
  wearing.value = card.id;
  try {
    if (card.original) {
      await api.selectLocalSkin(a.id, null);
    } else if (a.kind === "microsoft") {
      app.accounts = await api.uploadSkin(a.id, card.id);
      await loadOriginal();
      notify.success("Skin changed", "Minecraft confirmed the new skin on your profile.");
    } else {
      await api.selectLocalSkin(a.id, card.id);
      notify.info("Skin selected", "Offline profiles show this skin only inside Zip.");
    }
    await loadLibrary();
    await app.refreshLocalSkins();
  } catch (e) {
    notify.error("Skin change failed", errorText(e));
  } finally {
    wearing.value = "";
  }
}

async function removeSkin(card: Card) {
  try {
    await api.removeSkin(card.id);
    if (previewId.value === card.id) previewId.value = "original";
    await loadLibrary();
    await app.refreshLocalSkins();
  } catch (e) {
    notify.error("Could not remove skin", errorText(e));
  }
}

async function switchVariant(card: Card) {
  const next = card.variant === "slim" ? "classic" : "slim";
  await api.setSkinVariant(card.id, next);
  await loadLibrary();
  if (previewId.value === card.id) applyPreview();
}

watch(preview, applyPreview);
watch(layers, (on) => {
  viewer.value?.playerObject.skin.setOuterLayerVisible(on);
  wake();
});
watch(animate, (on) => {
  if (viewer.value) viewer.value.animation = on ? new IdleAnimation() : null;
  wake();
});
watch(autoRotate, (on) => {
  if (viewer.value) viewer.value.autoRotate = on;
  wake();
});
watch(account, () => {
  loadOriginal();
  previewId.value = "original";
});

onMounted(async () => {
  await Promise.all([loadOriginal(), loadLibrary()]);
  createViewer();
});

onActivated(() => {
  if (viewer.value) wake();
});

onDeactivated(() => {
  if (viewer.value) viewer.value.renderPaused = true;
});

onUnmounted(() => {
  window.clearTimeout(idleTimer);
  resizeObserver?.disconnect();
  viewer.value?.dispose();
});
</script>

<template>
  <div class="skins" :class="[`preview-${layout.preview}`, `size-${layout.size}`]">
    <section class="showcase panel">
      <div class="show-head">
        <div class="who ellipsis">{{ account?.name ?? "No account" }}</div>
        <div class="tools">
          <button class="tool" :class="{ on: layers }" title="Outer layer" @click="layers = !layers">
            <svg viewBox="0 0 16 16"><path d="M8 2 14 5 8 8 2 5zM2 8l6 3 6-3M2 11l6 3 6-3" /></svg>
          </button>
          <button class="tool" :class="{ on: animate }" title="Animate" @click="animate = !animate">
            <svg viewBox="0 0 16 16"><path d="M5 3v10l8-5z" /></svg>
          </button>
          <button class="tool" :class="{ on: autoRotate }" title="Auto rotate" @click="autoRotate = !autoRotate">
            <svg viewBox="0 0 16 16"><path d="M13 8a5 5 0 1 1-1.5-3.5M13 2v3h-3" /></svg>
          </button>
        </div>
      </div>
      <div ref="stage" class="stage" @pointerdown="wake" @pointermove="(e) => e.buttons && wake()" @wheel.passive="wake">
        <div class="floor" />
        <canvas ref="canvas" />
      </div>
      <div v-if="preview" class="show-info">
        <div class="show-name ellipsis">{{ preview.name }}</div>
        <div class="show-meta">
          <span class="pill">{{ preview.variant === "slim" ? "Slim arms" : "Classic arms" }}</span>
          <span v-if="selectedId === preview.id" class="pill accent">Wearing</span>
        </div>
        <div class="show-actions">
          <button class="btn primary grow" :disabled="!!wearing || selectedId === preview.id" @click="wear(preview)">
            <span v-if="wearing === preview.id" class="spinner" />
            {{ selectedId === preview.id ? "✓ Wearing" : "Wear this skin" }}
          </button>
          <button v-if="!preview.original" class="btn" :title="`Switch to ${preview.variant === 'slim' ? 'classic' : 'slim'} arms`" @click="switchVariant(preview)">Arms</button>
        </div>
      </div>
    </section>

    <section class="library">
      <header class="lib-head">
        <div class="lib-title">
          <h2>Skins</h2>
          <small>{{ cards.length }} in your library</small>
        </div>
        <input v-model="filter" class="field pill search" placeholder="Search skins.." />
        <button class="btn primary" @click="importSkin">+ Import skin</button>
      </header>
      <div class="grid scroll">
        <div
          v-for="card in shown"
          :key="card.id"
          class="card"
          :class="{ previewing: preview?.id === card.id, worn: selectedId === card.id }"
          @click="previewId = card.id"
        >
          <div class="thumb">
            <img v-if="card.thumb" :src="card.thumb" class="pixel" alt="" />
            <span v-else class="spinner" />
          </div>
          <div class="card-info">
            <div class="card-name ellipsis" :title="card.name">{{ card.name }}</div>
            <div class="card-meta">
              <span>{{ card.variant === "slim" ? "Slim" : "Classic" }}</span>
              <span v-if="selectedId === card.id" class="worn-tag">Wearing</span>
            </div>
          </div>
          <div class="card-actions" @click.stop>
            <button v-if="selectedId !== card.id" class="mini wear" :disabled="!!wearing" @click="wear(card)">
              <span v-if="wearing === card.id" class="spinner" />
              <template v-else>Wear</template>
            </button>
            <span class="grow" />
            <button v-if="!card.original" class="mini" :title="`Switch to ${card.variant === 'slim' ? 'classic' : 'slim'} arms`" @click="switchVariant(card)">Arms</button>
            <button v-if="!card.original" class="mini danger" title="Remove from library" @click="removeSkin(card)">✕</button>
          </div>
        </div>
        <button v-if="!filter" class="card add" @click="importSkin">
          <span class="plus">+</span>
          <span class="add-label">Import a skin</span>
          <small>PNG file</small>
        </button>
        <div v-if="filter && !shown.length" class="empty">No skins match that search.</div>
      </div>
      <p class="hint">
        Drag the model to rotate, scroll to zoom.
        <template v-if="account?.kind === 'microsoft'">Wearing a skin uploads it to your Minecraft profile.</template>
        <template v-else-if="account">Offline profiles keep skins inside Zip.</template>
      </p>
    </section>
  </div>
</template>

<style scoped>
.skins {
  --card: 128px;
  height: 100%;
  display: grid;
  grid-template-columns: 300px 1fr;
  gap: 12px;
  padding: 12px;
}

.skins.preview-right {
  grid-template-columns: 1fr 300px;
}

.skins.preview-right .showcase {
  order: 2;
}

.skins.size-small {
  --card: 104px;
}

.skins.size-large {
  --card: 160px;
}

.showcase {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  border-radius: 14px;
}

.show-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 12px 12px 0 16px;
}

.who {
  font-size: calc(13px * var(--text-scale, 1));
  color: var(--text-dim);
}

.tools {
  display: flex;
  gap: 4px;
  flex: none;
}

.tool {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: var(--r-chip);
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  color: var(--text-faint);
  transition: color var(--t-fast), background var(--t-fast), border-color var(--t-fast);
}

.tool:hover {
  color: var(--text);
}

.tool.on {
  color: var(--accent);
  background: var(--accent-bg);
  border-color: var(--accent-border);
}

.tool svg {
  width: 14px;
  height: 14px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.stage {
  flex: 1;
  min-height: 0;
  position: relative;
  cursor: grab;
  background: radial-gradient(60% 50% at 50% 42%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 70%);
}

.stage:active {
  cursor: grabbing;
}

.floor {
  position: absolute;
  left: 50%;
  bottom: 9%;
  width: 46%;
  height: 7%;
  translate: -50% 0;
  border-radius: 50%;
  background: radial-gradient(closest-side, rgba(0, 0, 0, 0.45), transparent);
  pointer-events: none;
}

.stage canvas {
  position: absolute;
  inset: 0;
}

.show-info {
  padding: 12px 14px 14px;
  border-top: 1px solid var(--border-soft);
  background: var(--panel-2);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.show-name {
  font-size: calc(17px * var(--text-scale, 1));
  font-weight: 650;
}

.show-meta {
  display: flex;
  gap: 6px;
}

.pill {
  font-size: calc(10px * var(--text-scale, 1));
  padding: 2px 8px;
  border-radius: 10px;
  background: var(--panel-3);
  color: var(--text-dim);
}

.pill.accent {
  background: var(--accent-bg-hover);
  color: var(--accent);
}

.show-actions {
  display: flex;
  gap: 6px;
}

.library {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.lib-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 2px 2px 12px;
}

.lib-title {
  flex: 1;
  min-width: 0;
}

.lib-title h2 {
  margin: 0;
  font-size: calc(20px * var(--text-scale, 1));
  font-weight: 650;
}

.lib-title small {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
}

.search {
  width: 190px;
  flex: none;
}

.grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card), 1fr));
  grid-auto-rows: max-content;
  gap: 10px;
  padding: 3px 3px 6px;
  align-content: start;
}

.card {
  position: relative;
  display: flex;
  flex-direction: column;
  border-radius: var(--r-panel);
  background: var(--panel);
  border: 1px solid var(--panel-border);
  box-shadow: inset 0 1px 0 var(--hairline), var(--shadow-sm);
  cursor: pointer;
  overflow: hidden;
  transition: transform var(--t-med) var(--ease), filter var(--t-med) var(--ease), border-color var(--t-fast), box-shadow var(--t-fast);
}

.card:hover {
  transform: translateY(-2px);
  filter: brightness(1.1);
  border-color: var(--border-strong);
}

.card.previewing {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-border), 0 4px 16px var(--accent-glow);
}

.thumb {
  aspect-ratio: 4 / 5;
  display: grid;
  place-items: center;
  background:
    radial-gradient(70% 55% at 50% 40%, color-mix(in srgb, var(--accent) 8%, transparent), transparent 70%),
    var(--panel-3);
}

.thumb img {
  height: 78%;
  aspect-ratio: 1 / 2;
}

.card-info {
  padding: 8px 10px 9px;
}

.card-name {
  font-size: calc(13px * var(--text-scale, 1));
  font-weight: 600;
}

.card-meta {
  display: flex;
  gap: 6px;
  margin-top: 2px;
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-faint);
}

.worn-tag {
  color: var(--accent);
  font-weight: 600;
}

.card-actions {
  position: absolute;
  top: 6px;
  left: 6px;
  right: 6px;
  display: flex;
  gap: 4px;
  opacity: 0;
  translate: 0 -3px;
  transition: opacity var(--t-fast), translate var(--t-fast) var(--ease);
}

.card:hover .card-actions {
  opacity: 1;
  translate: 0 0;
}

.mini {
  height: 22px;
  min-width: 22px;
  padding: 0 7px;
  display: grid;
  place-items: center;
  border-radius: var(--r-small);
  font-size: calc(10px * var(--text-scale, 1));
  background: color-mix(in srgb, var(--panel) 85%, transparent);
  border: 1px solid var(--border-soft);
  color: var(--text-dim);
  backdrop-filter: blur(4px);
  transition: color var(--t-fast), background var(--t-fast);
}

.mini:hover {
  color: var(--text);
}

.mini.wear {
  color: var(--on-accent);
  background: var(--accent);
  border-color: transparent;
  font-weight: 600;
}

.mini.danger:hover {
  background: var(--danger-bg);
  color: var(--danger-text);
}

.card.add {
  align-items: center;
  justify-content: center;
  gap: 4px;
  min-height: calc(var(--card) * 1.25 + 46px);
  border: 1.5px dashed var(--accent-border);
  background: var(--accent-bg);
  box-shadow: none;
  color: var(--accent);
}

.card.add:hover {
  border-color: var(--accent);
}

.plus {
  font-size: calc(30px * var(--text-scale, 1));
  line-height: 1;
}

.add-label {
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
}

.card.add small {
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-faint);
}

.hint {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
  margin: 8px 2px 0;
}
</style>
