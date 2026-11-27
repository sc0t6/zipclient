<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from "vue";
import Modal from "./Modal.vue";
import VirtualList from "./VirtualList.vue";
import { api, errorText, formatPlaytime, type Instance, type LaunchLog, type PlaySession } from "../api";

const props = defineProps<{ instance: Instance; session: PlaySession }>();
const emit = defineEmits<{ close: [] }>();

const log = shallowRef<LaunchLog | null>(null);
const error = ref("");
const filter = ref("");
const onlyProblems = ref(false);
const crashed = computed(() => props.session.exit_code !== 0 && props.session.exit_code !== null);

function level(line: string) {
  if (/\b(ERROR|FATAL|Exception)\b|^\s+at |^Caused by/.test(line)) return "err";
  if (/\bWARN/.test(line)) return "warn";
  return "";
}

const counts = computed(() => {
  let err = 0;
  let warn = 0;
  for (const l of log.value?.lines ?? []) {
    const lv = level(l);
    if (lv === "err") err++;
    else if (lv === "warn") warn++;
  }
  return { err, warn };
});

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return (log.value?.lines ?? []).filter((l) => (!onlyProblems.value || level(l)) && (!q || l.toLowerCase().includes(q)));
});

function copy() {
  navigator.clipboard.writeText(shown.value.join("\n")).catch(() => {});
}

onMounted(async () => {
  try {
    log.value = await api.launchLog(props.instance.id, props.session.started, crashed.value);
    if (crashed.value && counts.value.err) onlyProblems.value = true;
  } catch (e) {
    error.value = errorText(e);
  }
});
</script>

<template>
  <Modal :title="new Date(session.started * 1000).toLocaleString()" :width="980" @close="emit('close')">
    <div class="body">
      <div class="meta">
        <span class="badge" :class="session.exit_code === 0 ? 'ok' : session.exit_code === null ? '' : 'bad'">
          {{ session.exit_code === 0 ? "Exited normally" : session.exit_code === null ? "Stopped from Zip" : `Crashed · exit code ${session.exit_code}` }}
        </span>
        <span class="muted">Played {{ formatPlaytime(session.duration) }}</span>
        <span v-if="log?.found" class="muted">{{ log.lines.length.toLocaleString() }} lines</span>
        <span v-if="counts.err" class="count err">{{ counts.err }} errors</span>
        <span v-if="counts.warn" class="count warn">{{ counts.warn }} warnings</span>
      </div>
      <div v-if="log?.summary" class="summary selectable">{{ log.summary }}</div>
      <template v-if="log?.found">
        <div class="toolbar">
          <input v-model="filter" class="field pill grow" placeholder="Filter log.." />
          <button class="btn" :class="{ on: onlyProblems }" @click="onlyProblems = !onlyProblems">Errors and warnings</button>
          <button class="btn" :disabled="!shown.length" @click="copy">Copy</button>
          <button class="btn" @click="api.openFile(log.path)">Open file</button>
        </div>
        <VirtualList v-if="shown.length" class="console selectable" :items="shown" :item-height="16">
          <template #default="{ item }">
            <div class="line" :class="level(item)" :title="item">{{ item }}</div>
          </template>
        </VirtualList>
        <div v-else class="empty">Nothing matches.</div>
      </template>
      <div v-else-if="log" class="empty">
        This log is gone. Zip keeps the last 10 launch logs per instance.
        <button class="btn" @click="api.openInstanceFolder(instance.id, 'crash-reports')">Crash reports</button>
      </div>
      <div v-else-if="error" class="empty bad">{{ error }}</div>
      <div v-else class="empty"><span class="spinner" /></div>
    </div>
  </Modal>
</template>

<style scoped>
.body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 0 16px 16px;
  height: min(640px, calc(100vh - 160px));
}

.meta {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: calc(12px * var(--text-scale, 1));
}

.badge {
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  font-weight: 600;
}

.badge.ok {
  color: var(--running);
}

.badge.bad {
  color: var(--danger-text);
  border-color: color-mix(in srgb, var(--danger) 45%, transparent);
  background: color-mix(in srgb, var(--danger) 12%, transparent);
}

.count.err {
  color: var(--danger-text);
}

.count.warn {
  color: var(--warn);
}

.summary {
  white-space: pre-wrap;
  font-size: calc(12px * var(--text-scale, 1));
  line-height: 1.5;
  padding: 10px 12px;
  border-radius: var(--r-row);
  background: color-mix(in srgb, var(--danger) 9%, var(--panel-2));
  border: 1px solid color-mix(in srgb, var(--danger) 30%, transparent);
  max-height: 140px;
  overflow: auto;
  user-select: text;
  -webkit-user-select: text;
}

.toolbar {
  display: flex;
  gap: 6px;
}

.btn.on {
  border-color: var(--accent-border);
  color: var(--accent);
  background: var(--accent-bg);
}

.console {
  flex: 1;
  min-height: 0;
  background: var(--panel-3);
  border-radius: var(--r-row);
  padding: 6px 8px;
  font-family: ui-monospace, Consolas, monospace;
  font-size: calc(11px * var(--text-scale, 1));
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

.empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-dim);
  font-size: calc(12px * var(--text-scale, 1));
}

.empty.bad {
  color: var(--danger-text);
}
</style>
