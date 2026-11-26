<script setup lang="ts">
import KebabMenu, { type MenuItem } from "./KebabMenu.vue";

const props = withDefaults(
  defineProps<{
    icon?: string;
    title: string;
    subtitle?: string;
    description?: string;
    chips?: string[];
    busy?: boolean;
    disabled?: boolean;
    installed?: boolean;
    installLabel?: string;
    versionLabel?: string;
    versionDone?: boolean;
    menu?: MenuItem[];
    compact?: boolean;
    expanded?: boolean;
    extended?: string;
    extendedLoading?: boolean;
    truncated?: boolean;
  }>(),
  { installLabel: "+ Install", menu: () => [], chips: () => [] },
);
const emit = defineEmits<{ install: []; version: []; open: []; more: []; all: [] }>();
</script>

<template>
  <div class="mod" :class="{ done: installed, compact, expanded }" @click="emit('open')">
    <div class="mod-row">
      <div class="mod-icon">
        <img v-if="icon" :src="icon" loading="lazy" decoding="async" alt="" @error="($event.target as HTMLImageElement).style.visibility = 'hidden'" />
        <span v-else class="letter">{{ title.charAt(0).toUpperCase() }}</span>
      </div>
      <div class="mod-text">
        <div class="mod-head">
          <span class="mod-title ellipsis">{{ title }}</span>
          <span v-if="subtitle" class="mod-sub ellipsis">{{ subtitle }}</span>
        </div>
        <div v-if="props.description" class="mod-desc">
          <span class="ellipsis">{{ props.description }}</span>
          <button v-if="!expanded" class="more-link" @click.stop="emit('more')">See more</button>
        </div>
        <div v-if="chips.length && !compact" class="chips">
          <span v-for="chip in chips" :key="chip" class="chip">{{ chip }}</span>
        </div>
      </div>
      <slot name="extra" />
      <div class="actions" @click.stop>
        <button v-if="versionLabel !== undefined" class="ver" :class="{ done: versionDone }" :disabled="disabled || busy" :title="versionDone ? 'Installed version · click to pick another' : 'Pick a version'" @click="emit('version')">
          {{ versionLabel || "+ Ver" }}
        </button>
        <button class="install" :class="{ installed }" :disabled="disabled || busy || installed" @click="emit('install')">
          <span v-if="busy" class="spinner" />
          <span v-else-if="installed" class="ok">✓ Installed</span>
          <span v-else>{{ installLabel }}</span>
        </button>
        <KebabMenu v-if="menu.length" :items="menu" />
      </div>
    </div>
    <div v-if="expanded" class="more" @click.stop>
      <div v-if="extendedLoading" class="more-loading"><span class="spinner" /></div>
      <p v-else class="more-text">{{ extended || description }}</p>
      <div class="more-links">
        <button v-if="truncated" class="more-link strong" @click="emit('all')">See all</button>
        <button class="more-link" @click="emit('more')">See less</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mod {
  height: 100%;
  flex: none;
  display: flex;
  flex-direction: column;
  background: var(--panel);
  border: 1px solid transparent;
  border-radius: var(--r-panel);
  cursor: pointer;
  transition: background var(--t-fast), border-color var(--t-fast), transform var(--t-med) var(--ease), filter var(--t-med) var(--ease), box-shadow var(--t-med) var(--ease);
}

.mod:hover {
  border-color: var(--border-soft);
  transform: translateY(-2px);
  filter: brightness(1.1);
  box-shadow: var(--shadow);
}

.mod.done {
  background: color-mix(in srgb, var(--accent) 4%, var(--panel));
}

.mod-row {
  height: 84px;
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 11px 0 14px;
}

.compact .mod-row {
  height: 64px;
}

.mod-icon {
  width: 52px;
  height: 52px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--field);
  display: grid;
  place-items: center;
  overflow: hidden;
  flex: none;
}

.compact .mod-icon {
  width: 40px;
  height: 40px;
  border-radius: 8px;
}

.mod-icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.letter {
  font-size: calc(22px * var(--text-scale, 1));
  font-weight: 600;
  color: var(--text-faint);
}

.mod-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.mod-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.mod-title {
  font-size: calc(14px * var(--text-scale, 1));
  font-weight: 600;
  flex: none;
  max-width: 60%;
}

.mod-sub {
  font-size: calc(11px * var(--text-scale, 1));
  font-style: italic;
  color: var(--text-dim);
  min-width: 0;
}

.mod-desc {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
}

.more-link {
  flex: none;
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--accent);
  opacity: 0.85;
  transition: opacity var(--t-fast);
}

.more-link:hover {
  opacity: 1;
  text-decoration: underline;
}

.more-link.strong {
  font-weight: 600;
}

.chips {
  display: flex;
  gap: 4px;
  overflow: hidden;
}

.chip {
  font-size: calc(10px * var(--text-scale, 1));
  padding: 1px 7px;
  border-radius: 10px;
  background: var(--panel-3);
  color: var(--text-faint);
  white-space: nowrap;
}

.more {
  flex: 1;
  min-height: 0;
  margin: 0 14px 10px 76px;
  padding-top: 8px;
  border-top: 1px solid var(--border-soft);
  display: flex;
  flex-direction: column;
  gap: 6px;
  cursor: default;
}

.compact .more {
  margin-left: 64px;
}

.more-text {
  margin: 0;
  font-size: calc(12px * var(--text-scale, 1));
  line-height: 1.5;
  color: var(--text-dim);
  white-space: pre-line;
  display: -webkit-box;
  -webkit-line-clamp: 5;
  -webkit-box-orient: vertical;
  overflow: hidden;
  user-select: text;
  -webkit-user-select: text;
}

.more-loading {
  flex: 1;
  display: grid;
  place-items: center;
}

.more-links {
  display: flex;
  gap: 12px;
  margin-top: auto;
}

.actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: none;
  cursor: default;
}

.ver,
.install {
  height: 36px;
  border-radius: var(--r-row);
  border: 1px solid transparent;
  flex: none;
  transition: background var(--t-fast), border-color var(--t-fast), color var(--t-fast), transform var(--t-fast), opacity var(--t-fast);
}

.compact .ver,
.compact .install {
  height: 32px;
}

.ver {
  min-width: 64px;
  max-width: 130px;
  padding: 0 11px;
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  background: transparent;
  border-color: var(--border-soft);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ver:hover:not(:disabled) {
  color: var(--text);
  border-color: var(--border-strong);
  background: var(--chip-hover);
}

.ver.done {
  color: var(--accent);
  border-color: var(--accent-border);
  background: var(--accent-bg);
}

.install {
  min-width: 104px;
  padding: 0 14px;
  font-size: calc(13px * var(--text-scale, 1));
  font-weight: 600;
  display: grid;
  place-items: center;
  color: var(--accent);
  background: var(--accent-bg);
  border-color: var(--accent-border);
}

.install:hover:not(:disabled) {
  background: var(--accent-bg-hover);
  border-color: var(--accent);
  box-shadow: 0 2px 12px var(--accent-glow);
}

.install.installed {
  color: var(--accent);
  background: transparent;
  border-color: var(--border-soft);
  font-weight: 500;
}

.install.installed:disabled {
  opacity: 1;
  cursor: default;
}

.ver:active:not(:disabled),
.install:active:not(:disabled) {
  transform: scale(0.96);
}

button:disabled {
  opacity: 0.5;
}
</style>
