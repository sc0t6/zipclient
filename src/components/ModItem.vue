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
  }>(),
  { installLabel: "+ Install", menu: () => [], chips: () => [] },
);
const emit = defineEmits<{ install: []; version: [] }>();
</script>

<template>
  <div class="mod" :class="{ done: installed }">
    <div class="mod-icon">
      <img v-if="icon" :src="icon" loading="lazy" decoding="async" alt="" @error="($event.target as HTMLImageElement).style.visibility = 'hidden'" />
      <span v-else class="letter">{{ title.charAt(0).toUpperCase() }}</span>
    </div>
    <div class="mod-text">
      <div class="mod-head">
        <span class="mod-title ellipsis">{{ title }}</span>
        <span v-if="subtitle" class="mod-sub ellipsis">{{ subtitle }}</span>
      </div>
      <div v-if="props.description" class="mod-desc ellipsis" :title="props.description">{{ props.description }}</div>
      <div v-if="chips.length" class="chips">
        <span v-for="chip in chips" :key="chip" class="chip">{{ chip }}</span>
      </div>
    </div>
    <slot name="extra" />
    <div class="actions">
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
</template>

<style scoped>
.mod {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 84px;
  padding: 0 11px 0 14px;
  background: var(--panel);
  border: 1px solid transparent;
  border-radius: var(--r-panel);
  flex: none;
  transition: background var(--t-fast), border-color var(--t-fast), transform var(--t-fast) var(--ease);
}

.mod:hover {
  border-color: var(--border-soft);
  background: color-mix(in srgb, var(--panel) 85%, var(--chip-hover));
}

.mod.done {
  background: color-mix(in srgb, var(--accent) 4%, var(--panel));
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

.mod-icon img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.letter {
  font-size: 22px;
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
  font-size: 14px;
  font-weight: 600;
  flex: none;
  max-width: 60%;
}

.mod-sub {
  font-size: 11px;
  font-style: italic;
  color: var(--text-dim);
  min-width: 0;
}

.mod-desc {
  font-size: 12px;
  color: var(--text-dim);
}

.chips {
  display: flex;
  gap: 4px;
  overflow: hidden;
}

.chip {
  font-size: 10px;
  padding: 1px 7px;
  border-radius: 10px;
  background: var(--panel-3);
  color: var(--text-faint);
  white-space: nowrap;
}

.actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: none;
}

.ver,
.install {
  height: 36px;
  border-radius: var(--r-row);
  border: 1px solid transparent;
  flex: none;
  transition: background var(--t-fast), border-color var(--t-fast), color var(--t-fast), transform var(--t-fast), opacity var(--t-fast);
}

.ver {
  min-width: 64px;
  max-width: 130px;
  padding: 0 11px;
  font-size: 12px;
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
  font-size: 13px;
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
