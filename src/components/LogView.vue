<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import VirtualList from "./VirtualList.vue";
import { api, type Instance } from "../api";
import { useApp } from "../stores/app";

const props = defineProps<{ instance: Instance }>();
const app = useApp();
const lines = shallowRef<string[]>([]);
const filter = ref("");
const follow = ref(true);
const list = ref<{ scrollToBottom: () => void; atBottom: () => boolean } | null>(null);
let unlisten: UnlistenFn | null = null;

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return q ? lines.value.filter((l) => l.toLowerCase().includes(q)) : lines.value;
});

function level(line: string) {
  if (/\b(ERROR|FATAL|Exception)\b|^\s+at /.test(line)) return "err";
  if (/\bWARN/.test(line)) return "warn";
  return "";
}

async function scrollDown() {
  await nextTick();
  if (follow.value) list.value?.scrollToBottom();
}

function copy() {
  navigator.clipboard.writeText(shown.value.join("\n")).catch(() => {});
}

onMounted(async () => {
  lines.value = await api.gameLogs(props.instance.id);
  scrollDown();
  unlisten = await listen<{ instanceId: string; lines: string[] }>("game-log", ({ payload }) => {
    if (payload.instanceId !== props.instance.id) return;
    follow.value = list.value?.atBottom() ?? true;
    const next = lines.value.concat(payload.lines);
    lines.value = next.length > 4000 ? next.slice(next.length - 4000) : next;
    scrollDown();
  });
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="logs">
    <div class="toolbar">
      <input v-model="filter" class="field pill grow" placeholder="Filter log.." />
      <button class="btn" :disabled="!lines.length" @click="copy">Copy</button>
      <button class="btn" @click="api.openInstanceFolder(instance.id, 'logs')">Game logs</button>
      <button class="btn" @click="api.openInstanceFolder(instance.id, 'crash-reports')">Crash reports</button>
      <button class="btn ghost" :disabled="!lines.length" @click="lines = []">Clear</button>
    </div>
    <div class="state" v-if="app.games[instance.id]">Live · {{ app.games[instance.id].state }}</div>
    <VirtualList v-if="shown.length" ref="list" class="console selectable" :items="shown" :item-height="16">
      <template #default="{ item }">
        <div class="line" :class="level(item)">{{ item }}</div>
      </template>
    </VirtualList>
    <div v-else class="empty">No output yet. Launch the instance to see its log here.</div>
  </div>
</template>

<style scoped>
.logs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px 12px;
}

.toolbar {
  display: flex;
  gap: 6px;
}

.state {
  font-size: 11px;
  color: var(--running);
}

.console {
  flex: 1;
  min-height: 0;
  background: var(--panel-3);
  border-radius: var(--r-row);
  padding: 6px 8px;
  font-family: ui-monospace, Consolas, monospace;
  font-size: 11px;
  user-select: text;
  -webkit-user-select: text;
}

.line {
  white-space: pre;
  line-height: 16px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
}

.line.err {
  color: var(--danger-text);
}

.line.warn {
  color: var(--warn);
}
</style>
