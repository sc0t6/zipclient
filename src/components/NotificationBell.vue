<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { icons } from "../icons";
import { useNotify } from "../stores/notify";

const notify = useNotify();
const open = ref(false);
const root = ref<HTMLElement | null>(null);
const now = ref(Date.now());
let timer = 0;

function toggle() {
  open.value = !open.value;
  now.value = Date.now();
  if (open.value) notify.markRead();
}

watch(
  () => notify.unread,
  (n) => {
    if (open.value && n) notify.markRead();
  },
);

function ago(t: number) {
  const s = Math.max(0, Math.round((now.value - t) / 1000));
  if (s < 45) return "just now";
  if (s < 3600) return `${Math.max(1, Math.round(s / 60))} min ago`;
  return new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function outside(e: MouseEvent) {
  if (open.value && root.value && !root.value.contains(e.target as Node)) open.value = false;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") open.value = false;
}

onMounted(() => {
  window.addEventListener("mousedown", outside);
  window.addEventListener("keydown", onKey);
  timer = window.setInterval(() => (now.value = Date.now()), 30000);
});
onUnmounted(() => {
  window.removeEventListener("mousedown", outside);
  window.removeEventListener("keydown", onKey);
  window.clearInterval(timer);
});
</script>

<template>
  <div ref="root" class="bell-wrap">
    <button class="bell" :class="{ on: open }" :title="notify.unread ? `${notify.unread} new notifications` : 'Notifications'" @click="toggle">
      <img :src="icons.bell" class="icon" alt="" />
      <span v-if="notify.unread" class="badge">{{ notify.unread > 9 ? "9+" : notify.unread }}</span>
    </button>
    <Transition name="drop">
      <div v-if="open" class="panel menu">
        <div class="head">
          <strong>Notifications</strong>
          <span class="muted grow">since Zip opened</span>
          <button v-if="notify.history.length" class="btn ghost small" @click="notify.clearHistory()">Clear</button>
        </div>
        <div v-if="notify.history.length" class="scroll list">
          <div v-for="n in notify.history" :key="n.id" class="entry" :class="n.level">
            <span class="mark" />
            <div class="grow text">
              <div class="title">{{ n.title }}</div>
              <div v-if="n.message" class="msg selectable">{{ n.message }}</div>
            </div>
            <span class="time">{{ ago(n.time) }}</span>
          </div>
        </div>
        <div v-else class="empty">Nothing yet. Downloads, updates and errors show up here.</div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.bell-wrap {
  position: relative;
  -webkit-app-region: no-drag;
}

.bell {
  position: relative;
  width: 41px;
  height: 39px;
  display: grid;
  place-items: center;
  background: var(--panel);
  border: 1px solid var(--panel-border);
  border-radius: 14px;
  box-shadow: inset 0 1px 0 var(--hairline), var(--shadow);
  transition: background var(--t-fast), border-color var(--t-fast);
}

.bell:hover,
.bell.on {
  border-color: var(--accent-border);
}

.bell .icon {
  width: 15px;
  height: 15px;
}

.badge {
  position: absolute;
  top: 4px;
  right: 4px;
  min-width: 15px;
  height: 15px;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--accent);
  color: #fff;
  font-size: 9px;
  font-weight: 700;
  line-height: 15px;
  text-align: center;
  box-shadow: 0 0 8px var(--accent-glow);
}

.menu {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  width: 360px;
  max-height: 460px;
  display: flex;
  flex-direction: column;
  z-index: 150;
  background: linear-gradient(var(--panel), var(--panel)), var(--bg);
  backdrop-filter: blur(20px);
  border-radius: var(--r-panel);
  box-shadow: var(--shadow-modal);
  overflow: hidden;
}

.head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px 10px;
  border-bottom: 1px solid var(--border-soft);
  font-size: calc(12px * var(--text-scale, 1));
}

.head strong {
  font-size: calc(13px * var(--text-scale, 1));
}

.small {
  padding: 3px 10px;
  font-size: calc(11px * var(--text-scale, 1));
}

.list {
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.entry {
  display: flex;
  gap: 10px;
  padding: 8px 10px;
  border-radius: var(--r-small);
  font-size: calc(12px * var(--text-scale, 1));
}

.entry:hover {
  background: var(--chip-hover);
}

.mark {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  margin-top: 5px;
  flex: none;
  background: var(--accent);
}

.success .mark {
  background: var(--running);
}

.warn .mark {
  background: var(--warn);
}

.error .mark {
  background: var(--danger);
}

.text {
  min-width: 0;
}

.title {
  font-weight: 600;
}

.msg {
  color: var(--text-dim);
  margin-top: 2px;
  line-height: 1.4;
  word-break: break-word;
  user-select: text;
  -webkit-user-select: text;
}

.time {
  color: var(--text-faint);
  font-size: calc(10px * var(--text-scale, 1));
  white-space: nowrap;
  margin-top: 2px;
}

.empty {
  padding: 28px 18px;
  text-align: center;
  color: var(--text-dim);
  font-size: calc(12px * var(--text-scale, 1));
}

.drop-enter-active,
.drop-leave-active {
  transition: opacity var(--t-fast), transform var(--t-fast) var(--ease);
}

.drop-enter-from,
.drop-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}
</style>
