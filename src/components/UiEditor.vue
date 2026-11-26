<script setup lang="ts">
import { computed, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Modal from "./Modal.vue";
import Toggle from "./Toggle.vue";
import { api, errorText, rangeFill } from "../api";
import { defaultUi, fonts, type UiLayout } from "../ui";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";
import { useSettings } from "../stores/settings";

const emit = defineEmits<{ close: [] }>();
const store = useSettings();
const app = useApp();
const notify = useNotify();
const ui = computed(() => store.ui);
const sections = ["Text", "Background", "Shape", "Home", "Instances", "Mods", "Skins", "Sources"] as const;
const section = ref<(typeof sections)[number]>("Text");
const colors = ["#1f3a2b", "#1b2a4a", "#3a1f2b", "#2e2147", "#123a40", "#3d3420", "#0d0d0d", "#e9e4da"];
const curseforgeReady = computed(() => !!app.info?.curseforgeConfigured);
const sourceCount = computed(() => Number(ui.value.sources.modrinth) + Number(ui.value.sources.curseforge && curseforgeReady.value));

function set(change: (d: UiLayout) => void) {
  store.editUi(change);
}

function seg<T extends string>(value: T, options: [T, string][]) {
  return options.map(([id, label]) => ({ id, label, on: id === value }));
}

async function chooseImage() {
  const path = await open({ multiple: false, filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }] });
  if (typeof path !== "string") return;
  try {
    store.backgroundImage = await api.setUiBackground(path);
    set((d) => (d.background.mode = "image"));
  } catch (e) {
    notify.error("Could not use that image", errorText(e));
  }
}

async function removeImage() {
  await api.clearUiBackground().catch(() => {});
  store.backgroundImage = "";
  set((d) => (d.background.mode = "theme"));
}

function resetAll() {
  set((d) => Object.assign(d, structuredClone(defaultUi)));
}
</script>

<template>
  <Modal title="Edit UI" :width="760" @close="emit('close')">
    <div class="editor">
      <nav class="sections">
        <button v-for="name in sections" :key="name" class="sec" :class="{ on: section === name }" @click="section = name">{{ name }}</button>
        <button class="sec reset" @click="resetAll">Reset all</button>
      </nav>
      <div class="pane scroll">
        <template v-if="section === 'Text'">
          <div class="row-label">Font</div>
          <div class="fonts">
            <button v-for="f in fonts" :key="f.id" class="font" :class="{ on: ui.font === f.id }" :style="{ fontFamily: f.stack || undefined }" @click="set((d) => (d.font = f.id))">
              <span class="aa">Aa</span>
              <span class="fname">{{ f.label }}</span>
            </button>
          </div>
          <div v-if="ui.font === 'custom'" class="opt">
            <div><b>Font name</b><small>Any font installed on this computer.</small></div>
            <input class="field w200" :value="ui.customFont" placeholder="e.g. Segoe UI" maxlength="60" @input="set((d) => (d.customFont = ($event.target as HTMLInputElement).value))" />
          </div>
          <div class="opt">
            <div><b>Text size</b><small>{{ Math.round(ui.textScale * 100) }}%</small></div>
            <input class="range" type="range" min="0.85" max="1.3" step="0.05" :value="ui.textScale" :style="rangeFill(ui.textScale, 0.85, 1.3)" @input="set((d) => (d.textScale = Number(($event.target as HTMLInputElement).value)))" />
          </div>
          <p class="sample">The quick creeper jumps over the lazy villager.</p>
        </template>

        <template v-else-if="section === 'Background'">
          <div class="opt">
            <div><b>Background</b><small>Shown behind every page.</small></div>
            <div class="seg">
              <button v-for="o in seg(ui.background.mode, [['theme', 'Theme'], ['color', 'Color'], ['image', 'Image']])" :key="o.id" :class="{ on: o.on }" @click="o.id === 'image' && !store.backgroundImage ? chooseImage() : set((d) => (d.background.mode = o.id))">{{ o.label }}</button>
            </div>
          </div>
          <div v-if="ui.background.mode === 'color'" class="opt">
            <div><b>Color</b></div>
            <div class="swatches">
              <button v-for="c in colors" :key="c" class="swatch" :class="{ on: ui.background.color === c }" :style="{ background: c }" @click="set((d) => (d.background.color = c))" />
              <label class="swatch custom" title="Custom color"><input type="color" :value="ui.background.color" @input="set((d) => (d.background.color = ($event.target as HTMLInputElement).value))" /></label>
            </div>
          </div>
          <div v-if="ui.background.mode === 'image'" class="opt">
            <div class="image-row">
              <div class="image-preview" :style="{ backgroundImage: store.backgroundImage ? `url(${store.backgroundImage})` : 'none' }" />
              <div><b>Your image</b><small>PNG, JPEG, GIF or WebP up to 15 MB.</small></div>
            </div>
            <div class="row">
              <button class="btn" @click="chooseImage">Choose image</button>
              <button class="btn ghost" :disabled="!store.backgroundImage" @click="removeImage">Remove</button>
            </div>
          </div>
          <template v-if="ui.background.mode !== 'theme'">
            <div class="opt">
              <div><b>Darken</b><small>{{ Math.round(ui.background.dim * 100) }}%</small></div>
              <input class="range" type="range" min="0" max="0.9" step="0.05" :value="ui.background.dim" :style="rangeFill(ui.background.dim, 0, 0.9)" @input="set((d) => (d.background.dim = Number(($event.target as HTMLInputElement).value)))" />
            </div>
            <div class="opt">
              <div><b>Blur</b><small>{{ ui.background.blur }} px</small></div>
              <input class="range" type="range" min="0" max="24" step="1" :value="ui.background.blur" :style="rangeFill(ui.background.blur, 0, 24)" @input="set((d) => (d.background.blur = Number(($event.target as HTMLInputElement).value)))" />
            </div>
            <div class="opt">
              <div><b>Panels</b><small>{{ Math.round(ui.background.panels * 100) }}% solid</small></div>
              <input class="range" type="range" min="0.35" max="1" step="0.05" :value="ui.background.panels" :style="rangeFill(ui.background.panels, 0.35, 1)" @input="set((d) => (d.background.panels = Number(($event.target as HTMLInputElement).value)))" />
            </div>
          </template>
        </template>

        <template v-else-if="section === 'Shape'">
          <div class="opt">
            <div><b>Corner roundness</b><small>{{ Math.round(ui.radius * 100) }}%</small></div>
            <input class="range" type="range" min="0" max="2" step="0.1" :value="ui.radius" :style="rangeFill(ui.radius, 0, 2)" @input="set((d) => (d.radius = Number(($event.target as HTMLInputElement).value)))" />
          </div>
          <div v-if="store.s" class="opt">
            <div><b>Navigation</b><small>Where the main tabs sit.</small></div>
            <div class="seg">
              <button v-for="o in seg(store.s.appearance.nav, [['top', 'Top bar'], ['side', 'Sidebar'], ['icons', 'Icons only']])" :key="o.id" :class="{ on: o.on }" @click="store.s!.appearance.nav = o.id">{{ o.label }}</button>
            </div>
          </div>
        </template>

        <template v-else-if="section === 'Home'">
          <div class="opt">
            <div><b>Top panel size</b></div>
            <div class="seg">
              <button v-for="o in seg(ui.home.hero, [['small', 'Small'], ['normal', 'Normal'], ['large', 'Large']])" :key="o.id" :class="{ on: o.on }" @click="set((d) => (d.home.hero = o.id))">{{ o.label }}</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Accounts panel</b></div>
            <Toggle :model-value="ui.home.accounts" label="Accounts panel" @update:model-value="set((d) => (d.home.accounts = $event))" />
          </div>
          <div class="opt">
            <div><b>News panel</b></div>
            <Toggle :model-value="ui.home.news" label="News panel" @update:model-value="set((d) => (d.home.news = $event))" />
          </div>
          <div class="opt">
            <div><b>Swap bottom panels</b><small>Put news on the left and accounts on the right.</small></div>
            <Toggle :model-value="ui.home.swap" label="Swap bottom panels" :disabled="!ui.home.accounts || !ui.home.news" @update:model-value="set((d) => (d.home.swap = $event))" />
          </div>
        </template>

        <template v-else-if="section === 'Instances'">
          <div class="opt">
            <div><b>Instance list side</b></div>
            <div class="seg">
              <button v-for="o in seg(ui.instances.side, [['left', 'Left'], ['right', 'Right']])" :key="o.id" :class="{ on: o.on }" @click="set((d) => (d.instances.side = o.id))">{{ o.label }}</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Instance list width</b></div>
            <div class="seg">
              <button v-for="o in seg(ui.instances.width, [['narrow', 'Narrow'], ['normal', 'Normal'], ['wide', 'Wide']])" :key="o.id" :class="{ on: o.on }" @click="set((d) => (d.instances.width = o.id))">{{ o.label }}</button>
            </div>
          </div>
        </template>

        <template v-else-if="section === 'Mods'">
          <div class="opt">
            <div><b>Installed view</b><small>How installed mods, packs and shaders show.</small></div>
            <div class="seg">
              <button v-for="o in seg(ui.mods.view, [['list', 'List'], ['grid', 'Grid']])" :key="o.id" :class="{ on: o.on }" @click="set((d) => (d.mods.view = o.id))">{{ o.label }}</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Descriptions</b><small>Show the short description on each result.</small></div>
            <Toggle :model-value="ui.mods.descriptions" label="Descriptions" @update:model-value="set((d) => (d.mods.descriptions = $event))" />
          </div>
          <div class="opt">
            <div><b>Compact results</b><small>Smaller rows so more fit on screen.</small></div>
            <Toggle :model-value="ui.mods.compact" label="Compact results" @update:model-value="set((d) => (d.mods.compact = $event))" />
          </div>
        </template>

        <template v-else-if="section === 'Skins'">
          <div class="opt">
            <div><b>Preview side</b></div>
            <div class="seg">
              <button v-for="o in seg(ui.skins.preview, [['left', 'Left'], ['right', 'Right']])" :key="o.id" :class="{ on: o.on }" @click="set((d) => (d.skins.preview = o.id))">{{ o.label }}</button>
            </div>
          </div>
          <div class="opt">
            <div><b>Card size</b></div>
            <div class="seg">
              <button v-for="o in seg(ui.skins.size, [['small', 'Small'], ['medium', 'Medium'], ['large', 'Large']])" :key="o.id" :class="{ on: o.on }" @click="set((d) => (d.skins.size = o.id))">{{ o.label }}</button>
            </div>
          </div>
        </template>

        <template v-else>
          <p class="note">Choose which sites show up when you browse mods, resource packs, shaders and modpacks.</p>
          <div class="opt">
            <div><b>Modrinth</b></div>
            <Toggle :model-value="ui.sources.modrinth" label="Modrinth" :disabled="ui.sources.modrinth && sourceCount <= 1" @update:model-value="set((d) => (d.sources.modrinth = $event))" />
          </div>
          <div class="opt">
            <div><b>CurseForge</b><small v-if="!curseforgeReady">Not available in this build.</small></div>
            <Toggle :model-value="ui.sources.curseforge && curseforgeReady" label="CurseForge" :disabled="!curseforgeReady || (ui.sources.curseforge && sourceCount <= 1)" @update:model-value="set((d) => (d.sources.curseforge = $event))" />
          </div>
        </template>
      </div>
    </div>
    <div class="foot">
      <span class="muted">Changes apply right away.</span>
      <button class="btn primary" @click="emit('close')">Done</button>
    </div>
  </Modal>
</template>

<style scoped>
.editor {
  display: grid;
  grid-template-columns: 150px 1fr;
  min-height: 0;
  height: min(520px, calc(100vh - 200px));
}

.sections {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 12px 8px;
  border-right: 1px solid var(--border-soft);
}

.sec {
  text-align: left;
  padding: 7px 10px;
  border-radius: var(--r-row);
  font-size: calc(13px * var(--text-scale, 1));
  color: var(--text-dim);
  border: 1px solid transparent;
  transition: background var(--t-fast), color var(--t-fast);
}

.sec:hover {
  background: var(--chip-hover);
  color: var(--text);
}

.sec.on {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
  color: var(--accent);
  font-weight: 600;
}

.sec.reset {
  margin-top: auto;
  color: var(--text-faint);
  font-size: calc(12px * var(--text-scale, 1));
}

.sec.reset:hover {
  color: var(--danger-text);
  background: var(--danger-bg);
}

.pane {
  padding: 14px 16px;
  min-height: 0;
}

.row-label {
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
  margin-bottom: 8px;
}

.fonts {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
  gap: 8px;
  margin-bottom: 10px;
}

.font {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 10px 8px 8px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  transition: border-color var(--t-fast), transform var(--t-fast) var(--ease);
}

.font:hover {
  border-color: var(--border-strong);
  transform: translateY(-1px);
}

.font.on {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-border);
}

.aa {
  font-size: calc(24px * var(--text-scale, 1));
  line-height: 1.1;
}

.fname {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-dim);
}

.sample {
  margin: 12px 2px 0;
  font-size: calc(15px * var(--text-scale, 1));
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
  margin-bottom: 6px;
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

.range {
  width: 200px;
  flex: none;
}

.w200 {
  width: 200px;
  flex: none;
}

.seg {
  display: flex;
  gap: 3px;
  padding: 3px;
  background: var(--panel-3);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  flex: none;
}

.seg button {
  padding: 5px 11px;
  border-radius: var(--r-chip);
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  transition: background var(--t-fast), color var(--t-fast);
}

.seg button:hover {
  color: var(--text);
}

.seg button.on {
  background: var(--accent-bg-hover);
  color: var(--accent);
  font-weight: 600;
}

.swatches {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.swatch {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid var(--border);
  transition: transform var(--t-fast) var(--ease);
}

.swatch:hover {
  transform: scale(1.12);
}

.swatch.on {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-border);
}

.swatch.custom {
  position: relative;
  overflow: hidden;
  background: conic-gradient(#ff6b6b, #ffd93d, #6bff95, #6bc5ff, #c56bff, #ff6b6b);
  cursor: pointer;
}

.swatch.custom input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.image-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.image-preview {
  width: 96px;
  height: 54px;
  border-radius: var(--r-small);
  border: 1px solid var(--border);
  background-color: var(--panel-3);
  background-size: cover;
  background-position: center;
  flex: none;
}

.note {
  margin: 0 0 10px;
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
}

.foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-top: 1px solid var(--border-soft);
  font-size: calc(11px * var(--text-scale, 1));
}
</style>
