<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api, errorText, type Instance, type World } from "../api";
import { useNotify } from "../stores/notify";

const props = defineProps<{ instance: Instance }>();
const emit = defineEmits<{ datapacks: [folder: string] }>();
const notify = useNotify();
const worlds = ref<World[]>([]);
const loaded = ref(false);

async function load() {
  try {
    worlds.value = await api.worlds(props.instance.id);
  } catch (e) {
    notify.error("Could not read worlds", errorText(e));
  }
  loaded.value = true;
}

function ago(ms: number) {
  if (!ms) return "Never";
  const s = Math.max(0, (Date.now() - ms) / 1000);
  if (s < 3600) return `${Math.max(1, Math.round(s / 60))} min ago`;
  if (s < 86400) return `${Math.round(s / 3600)} h ago`;
  const d = Math.round(s / 86400);
  if (d < 30) return d === 1 ? "yesterday" : `${d} days ago`;
  return new Date(ms).toLocaleDateString();
}

onMounted(load);
</script>

<template>
  <div class="pane">
    <div class="toolbar">
      <span class="muted grow">{{ worlds.length }} world{{ worlds.length === 1 ? "" : "s" }}</span>
      <button class="btn" @click="load">Refresh</button>
      <button class="btn" @click="api.openInstanceFolder(instance.id, 'saves')">Open saves folder</button>
    </div>
    <div v-if="worlds.length" class="scroll list">
      <div v-for="w in worlds" :key="w.folder" class="world">
        <img v-if="w.icon" :src="w.icon" class="icon" alt="" />
        <div v-else class="icon blank">{{ w.name.slice(0, 1).toUpperCase() }}</div>
        <div class="grow info">
          <div class="name ellipsis">{{ w.name }}</div>
          <div class="sub">
            <span class="tag" :class="{ hard: w.hardcore }">{{ w.hardcore ? "Hardcore" : w.mode }}</span>
            <span v-if="w.version">{{ w.version }}</span>
            <span>Played {{ ago(w.lastPlayed) }}</span>
            <span v-if="w.folder !== w.name" class="faint ellipsis">{{ w.folder }}</span>
          </div>
        </div>
        <button class="btn" @click="emit('datapacks', w.folder)">Datapacks · {{ w.datapacks }}</button>
        <button class="btn" @click="api.openInstanceFolder(instance.id, `saves/${w.folder}`)">Open folder</button>
      </div>
    </div>
    <div v-else-if="loaded" class="empty">No worlds yet. Create one in game and it shows up here.</div>
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

.list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.world {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 10px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  transition: border-color var(--t-fast);
}

.world:hover {
  border-color: var(--accent-border);
}

.icon {
  width: 48px;
  height: 48px;
  border-radius: var(--r-small);
  image-rendering: pixelated;
  flex: none;
}

.icon.blank {
  display: grid;
  place-items: center;
  background: var(--panel-3);
  color: var(--text-dim);
  font-weight: 700;
  font-size: calc(18px * var(--text-scale, 1));
}

.info {
  min-width: 0;
}

.name {
  font-weight: 600;
  font-size: calc(13px * var(--text-scale, 1));
}

.sub {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-dim);
  min-width: 0;
}

.tag {
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--accent-bg);
  color: var(--accent);
  font-weight: 600;
}

.tag.hard {
  background: color-mix(in srgb, var(--danger) 15%, transparent);
  color: var(--danger-text);
}

.faint {
  color: var(--text-faint);
}

.empty {
  flex: 1;
  display: grid;
  place-items: center;
  color: var(--text-dim);
  font-size: calc(12px * var(--text-scale, 1));
}
</style>
