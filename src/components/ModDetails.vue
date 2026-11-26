<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Modal from "./Modal.vue";
import RichText from "./RichText.vue";
import { errorText, formatCount } from "../api";
import { icons } from "../icons";
import { project, type Project } from "../projects";

const props = defineProps<{
  source: string;
  projectId: string;
  title: string;
  iconUrl?: string;
  author?: string;
  summary?: string;
  pageUrl?: string;
  action?: { label: string; busy?: boolean; disabled?: boolean; done?: boolean };
}>();
const emit = defineEmits<{ close: []; action: [] }>();

const details = ref<Project | null>(null);
const error = ref("");
const shot = ref(0);
const zoomed = ref("");

const gallery = computed(() => details.value?.gallery ?? []);
const current = computed(() => gallery.value[shot.value]);
const icon = computed(() => details.value?.iconUrl || props.iconUrl || "");
const updated = computed(() => (details.value?.updated ? new Date(details.value.updated).toLocaleDateString() : ""));

function step(delta: number) {
  const n = gallery.value.length;
  if (n) shot.value = (shot.value + delta + n) % n;
}

function onKey(e: KeyboardEvent) {
  if (zoomed.value && e.key === "Escape") {
    e.stopImmediatePropagation();
    zoomed.value = "";
  } else if (e.key === "ArrowRight") step(1);
  else if (e.key === "ArrowLeft") step(-1);
}

onMounted(async () => {
  window.addEventListener("keydown", onKey, true);
  try {
    details.value = await project(props.source, props.projectId);
  } catch (e) {
    error.value = errorText(e);
  }
});

onUnmounted(() => window.removeEventListener("keydown", onKey, true));
</script>

<template>
  <Modal :width="860" :close-button="false" @close="emit('close')">
    <div class="details">
      <header class="head">
        <div class="icon-box">
          <img v-if="icon" :src="icon" alt="" />
          <span v-else>{{ title.charAt(0).toUpperCase() }}</span>
        </div>
        <div class="grow">
          <h2 class="ellipsis">{{ details?.title || title }}</h2>
          <div class="sub">
            <span v-if="author">by {{ author }}</span>
            <span v-if="details?.downloads">↓ {{ formatCount(details.downloads) }}</span>
            <span v-if="updated">Updated {{ updated }}</span>
          </div>
          <p class="summary">{{ details?.summary || summary }}</p>
        </div>
        <div class="actions">
          <button v-if="action" class="btn primary" :disabled="action.disabled || action.busy || action.done" @click="emit('action')">
            <span v-if="action.busy" class="spinner" />
            {{ action.done ? "✓ Installed" : action.label }}
          </button>
          <button v-if="pageUrl" class="btn" @click="openUrl(pageUrl)">Open page</button>
        </div>
        <button class="chip-btn close" title="Close" @click="emit('close')">
          <img :src="icons.close" class="icon" alt="" />
        </button>
      </header>

      <div class="body scroll">
        <div v-if="details?.categories.length" class="chips">
          <span v-for="c in details.categories" :key="c" class="chip">{{ c }}</span>
        </div>

        <section v-if="gallery.length" class="gallery">
          <div class="stage">
            <img :key="current.url" :src="current.url" :alt="current.title" class="shot" @click="zoomed = current.url" />
            <template v-if="gallery.length > 1">
              <button class="nav prev" title="Previous image" @click="step(-1)">‹</button>
              <button class="nav next" title="Next image" @click="step(1)">›</button>
            </template>
            <div v-if="current.title" class="caption ellipsis">{{ current.title }}</div>
          </div>
          <div v-if="gallery.length > 1" class="thumbs scroll">
            <button v-for="(g, i) in gallery" :key="g.url" class="thumb" :class="{ on: i === shot }" :title="g.title" @click="shot = i">
              <img :src="g.url" loading="lazy" alt="" />
            </button>
          </div>
        </section>

        <div v-if="details" class="desc">
          <RichText v-if="details.blocks.length" :blocks="details.blocks" @image="zoomed = $event" />
          <p v-else class="muted">No description.</p>
        </div>
        <div v-else-if="error" class="empty">{{ error }}</div>
        <div v-else class="empty"><span class="spinner" /></div>
      </div>

      <Transition name="fade">
        <div v-if="zoomed" class="lightbox" @click="zoomed = ''">
          <img :src="zoomed" alt="" />
        </div>
      </Transition>
    </div>
  </Modal>
</template>

<style scoped>
.details {
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: 0;
  max-height: calc(100vh - 140px);
}

.head {
  display: flex;
  gap: 14px;
  padding: 16px 18px 14px;
  border-bottom: 1px solid var(--border-soft);
  align-items: flex-start;
}

.icon-box {
  width: 72px;
  height: 72px;
  border-radius: 14px;
  border: 1px solid var(--border);
  background: var(--field);
  overflow: hidden;
  display: grid;
  place-items: center;
  flex: none;
  font-size: calc(28px * var(--text-scale, 1));
  font-weight: 600;
  color: var(--text-faint);
}

.icon-box img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

h2 {
  margin: 2px 0 4px;
  font-size: calc(20px * var(--text-scale, 1));
  font-weight: 650;
}

.sub {
  display: flex;
  gap: 12px;
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
}

.summary {
  margin: 8px 0 0;
  font-size: calc(13px * var(--text-scale, 1));
  color: var(--text-dim);
  user-select: text;
  -webkit-user-select: text;
}

.actions {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: none;
  min-width: 120px;
  margin-top: 4px;
}

.close {
  flex: none;
  background: var(--panel-2);
}

.close .icon {
  width: 13px;
  height: 13px;
}

.body {
  flex: 1;
  min-height: 0;
  padding: 14px 18px 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.chip {
  font-size: calc(10px * var(--text-scale, 1));
  padding: 2px 8px;
  border-radius: 10px;
  background: var(--panel-3);
  color: var(--text-dim);
  text-transform: capitalize;
}

.gallery {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: none;
}

.stage {
  position: relative;
  aspect-ratio: 16 / 9;
  max-height: 380px;
  border-radius: var(--r-row);
  background: var(--panel-3);
  overflow: hidden;
  display: grid;
  place-items: center;
}

.shot {
  width: 100%;
  height: 100%;
  object-fit: contain;
  cursor: zoom-in;
  animation: shot-in var(--t-med) var(--ease);
}

@keyframes shot-in {
  from {
    opacity: 0;
  }
}

.nav {
  position: absolute;
  top: 50%;
  translate: 0 -50%;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  background: color-mix(in srgb, #000 55%, transparent);
  color: #fff;
  font-size: calc(22px * var(--text-scale, 1));
  line-height: 1;
  display: grid;
  place-items: center;
  opacity: 0.75;
  transition: opacity var(--t-fast), background var(--t-fast);
}

.nav:hover {
  opacity: 1;
  background: color-mix(in srgb, #000 75%, transparent);
}

.prev {
  left: 10px;
}

.next {
  right: 10px;
}

.caption {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  padding: 18px 12px 8px;
  font-size: calc(12px * var(--text-scale, 1));
  color: #fff;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.7));
}

.thumbs {
  display: flex;
  gap: 6px;
  padding-bottom: 2px;
}

.thumb {
  width: 96px;
  height: 54px;
  flex: none;
  border-radius: var(--r-small);
  overflow: hidden;
  border: 2px solid transparent;
  opacity: 0.6;
  transition: opacity var(--t-fast), border-color var(--t-fast);
}

.thumb:hover {
  opacity: 0.9;
}

.thumb.on {
  opacity: 1;
  border-color: var(--accent);
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.lightbox {
  position: absolute;
  inset: 0;
  z-index: 5;
  background: rgba(0, 0, 0, 0.85);
  display: grid;
  place-items: center;
  padding: 16px;
  cursor: zoom-out;
  border-radius: var(--r-panel);
}

.lightbox img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}
</style>
