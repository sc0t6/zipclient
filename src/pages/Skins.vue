<script setup lang="ts">
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, shallowRef, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { IdleAnimation, SkinViewer } from "skinview3d";
import Toggle from "../components/Toggle.vue";
import { api, errorText, type SkinEntry } from "../api";
import { bodyFront, looksSlim, texture } from "../skin";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";

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
let idleTimer = 0;
let resizeObserver: ResizeObserver | null = null;

const account = computed(() => app.activeAccount);
const cards = computed<Card[]>(() => [
  ...(original.value ? [original.value] : []),
  ...library.value.map((s) => ({ ...s, thumb: thumbs.value[s.id + s.variant] ?? "", original: false })),
]);
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
    original.value = { id: "original", name: "Original", variant, dataUrl, thumb: await bodyFront(dataUrl, variant === "slim"), original: true };
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
  v.zoom = 0.68;
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
  <div class="skins">
    <section class="preview">
      <div ref="stage" class="stage" @pointerdown="wake" @pointermove="(e) => e.buttons && wake()" @wheel.passive="wake">
        <canvas ref="canvas" />
      </div>
      <div class="player-name ellipsis">{{ account?.name ?? "No account" }}</div>
    </section>
    <section class="side">
      <div class="top">
        <div class="using ellipsis">You are currently using: {{ account?.name ?? "nobody" }}</div>
        <div class="controls">
          <label><Toggle v-model="layers" label="Outer layer" /> Layers</label>
          <label><Toggle v-model="animate" label="Animate" /> Animate</label>
          <label><Toggle v-model="autoRotate" label="Auto rotate" /> Rotate</label>
        </div>
      </div>
      <div class="cards scroll">
        <div v-for="card in cards" :key="card.id" class="card" :class="{ previewing: preview?.id === card.id }" @click="previewId = card.id">
          <div class="thumb">
            <img v-if="card.thumb" :src="card.thumb" class="pixel" alt="" />
          </div>
          <div class="card-foot">
            <div class="card-name ellipsis">{{ card.name }}</div>
            <div v-if="selectedId === card.id" class="card-state">(selected)</div>
            <button v-else class="card-state wear" :disabled="!!wearing" @click.stop="wear(card)">
              <span v-if="wearing === card.id" class="spinner" />
              <template v-else>+ Wear</template>
            </button>
          </div>
          <div v-if="!card.original" class="card-tools" @click.stop>
            <button :title="`Switch to ${card.variant === 'slim' ? 'classic' : 'slim'} arms`" @click="switchVariant(card)">{{ card.variant === "slim" ? "Slim" : "Classic" }}</button>
            <button class="x" title="Remove from library" @click="removeSkin(card)">✕</button>
          </div>
          <div v-else class="card-tools"><span class="tag">{{ card.variant === "slim" ? "Slim" : "Classic" }}</span></div>
        </div>
        <button class="add" title="Import PNG skin" @click="importSkin">+</button>
      </div>
      <p class="hint">
        Drag to rotate, scroll to zoom.
        <template v-if="account?.kind === 'microsoft'">Wearing a skin uploads it to your Minecraft profile.</template>
        <template v-else-if="account">Offline profiles keep skins local to Zip.</template>
      </p>
    </section>
  </div>
</template>

<style scoped>
.skins {
  height: 100%;
  display: flex;
  gap: 6px;
  padding: 12px 12px 14px 12px;
}

.preview {
  width: 198px;
  flex: none;
  display: flex;
  flex-direction: column;
  background: var(--panel);
  border: 1px solid var(--panel-border);
  border-radius: 13px;
  overflow: hidden;
  box-shadow: inset 0 1px 0 var(--hairline), var(--shadow-sm);
}

.stage {
  flex: 1;
  min-height: 0;
  cursor: grab;
  position: relative;
}

.stage:active {
  cursor: grabbing;
}

.stage canvas {
  position: absolute;
  inset: 0;
}

.player-name {
  height: 50px;
  flex: none;
  display: grid;
  place-items: center;
  background: var(--panel-3);
  font-size: 20px;
  font-style: italic;
  margin: 0 0 0 2px;
  border-radius: 0 0 12px 12px;
  padding: 0 8px;
}

.side {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 1px 0 0 4px;
}

.top {
  display: flex;
  align-items: center;
  gap: 12px;
  justify-content: space-between;
}

.using {
  font-size: 16px;
  font-style: italic;
}

.controls {
  display: flex;
  gap: 12px;
  flex: none;
}

.controls label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--text-dim);
}

.cards {
  margin-top: 14px;
  display: flex;
  flex-wrap: wrap;
  gap: 11px;
  align-content: flex-start;
  flex: 1;
  min-height: 0;
  padding: 2px;
}

.card {
  width: 120px;
  height: 185px;
  border-radius: var(--r-row);
  background: var(--panel);
  border: 1px solid var(--border-soft);
  display: flex;
  flex-direction: column;
  position: relative;
  cursor: pointer;
  box-shadow: inset 0 1px 0 var(--hairline);
  transition: transform var(--t-fast) var(--ease), box-shadow var(--t-fast), border-color var(--t-fast);
}

.card:hover {
  transform: translateY(-2px);
  border-color: var(--border-strong);
  box-shadow: var(--shadow-pop), inset 0 1px 0 var(--hairline);
}

.card.previewing {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-border), 0 4px 16px var(--accent-glow);
}

.thumb {
  flex: 1;
  display: grid;
  place-items: center;
  padding-top: 14px;
}

.thumb img {
  height: 112px;
  width: 56px;
}

.card-foot {
  height: 50px;
  background: var(--panel-3);
  border-radius: 0 0 12px 12px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
}

.card-name {
  font-size: 16px;
  font-style: italic;
  max-width: 110px;
  padding: 0 3px;
}

.card-state {
  font-size: 12px;
  font-style: italic;
  color: var(--text-faint);
}

.card-state.wear {
  font-size: 16px;
  transition: color var(--t-fast);
  display: grid;
  place-items: center;
  min-height: 19px;
}

.card-state.wear:hover {
  color: var(--accent);
}

.card-tools {
  position: absolute;
  top: 6px;
  left: 6px;
  right: 6px;
  display: flex;
  justify-content: space-between;
  opacity: 0;
  transition: opacity var(--t-fast);
}

.card:hover .card-tools {
  opacity: 1;
}

.card-tools button,
.tag {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: var(--r-small);
  background: var(--panel-3);
  color: var(--text-dim);
}

.card-tools button:hover {
  color: var(--text);
}

.card-tools .x:hover {
  background: var(--danger-bg);
  color: var(--danger-text);
}

.add {
  width: 47px;
  height: 185px;
  border-radius: var(--r-small);
  background: var(--accent-bg);
  border: 1px solid var(--accent-border);
  color: var(--accent);
  font-size: 36px;
  transition: background var(--t-fast), transform var(--t-fast) var(--ease), box-shadow var(--t-fast);
}

.add:hover {
  background: var(--accent-bg-hover);
  box-shadow: 0 0 16px var(--accent-glow);
}

.add:active {
  transform: scale(0.97);
}

.hint {
  font-size: 11px;
  color: var(--text-faint);
  margin: 8px 0 0;
}
</style>
