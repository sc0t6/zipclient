<script setup lang="ts">
import { icons } from "../icons";
import { useNotify, type Level } from "../stores/notify";

const notify = useNotify();

const levelIcon: Record<Level, string> = {
  info: icons.info,
  success: icons.verified,
  warn: icons.warning,
  error: icons.shield,
};
</script>

<template>
  <div class="toasts" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="toast in notify.toasts" :key="toast.id" class="toast" :class="toast.level" role="status">
        <div class="head">
          <img :src="icons.bell" class="icon bell" alt="" />
          <span class="title ellipsis">{{ toast.title }}</span>
          <button class="x" title="Dismiss" @click="notify.dismiss(toast.id)">
            <img :src="icons.close" class="icon" alt="" />
          </button>
        </div>
        <div v-if="toast.message" class="line">
          <img :src="levelIcon[toast.level]" class="icon lvl" :class="{ keep: toast.level === 'error' || toast.level === 'warn' }" alt="" />
          <span class="selectable">{{ toast.message }}</span>
        </div>
        <button
          v-if="toast.action"
          class="action"
          @click="
            toast.action.run();
            notify.dismiss(toast.id);
          "
        >
          {{ toast.action.label }}
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toasts {
  position: fixed;
  right: 12px;
  bottom: 12px;
  width: 280px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  z-index: 200;
  pointer-events: none;
}

.toast {
  pointer-events: auto;
  background: var(--panel);
  border-radius: var(--r-row);
  padding: 10px 10px 10px 12px;
  box-shadow: var(--shadow-pop), inset 0 1px 0 var(--hairline);
  border: 1px solid var(--border-soft);
  border-left: 3px solid var(--info);
}

.toast.success {
  border-left-color: var(--running);
}

.toast.warn {
  border-left-color: var(--warn);
}

.toast.error {
  border-left-color: var(--danger-text);
}

.head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bell {
  width: 14px;
  height: 14px;
}

.title {
  flex: 1;
  font-weight: 600;
  font-size: calc(12px * var(--text-scale, 1));
}

.x {
  width: 16px;
  height: 16px;
  display: grid;
  place-items: center;
  border-radius: 4px;
  opacity: 0.8;
  transition: opacity var(--t-fast), background var(--t-fast);
}

.x:hover {
  opacity: 1;
  background: var(--chip-hover);
}

.x .icon {
  width: 11px;
  height: 11px;
}

.line span {
  white-space: pre-line;
}

.line {
  display: flex;
  gap: 6px;
  align-items: flex-start;
  margin-top: 6px;
  font-size: calc(12px * var(--text-scale, 1));
  line-height: 1.35;
  word-break: break-word;
  max-height: 120px;
  overflow: auto;
}

.lvl {
  width: 11px;
  height: 11px;
  margin-top: 2px;
}

.error .line {
  color: var(--danger-text);
}

.warn .line {
  color: var(--warn);
}

.action {
  margin-top: 8px;
  font-size: calc(11px * var(--text-scale, 1));
  padding: 4px 8px;
  border-radius: var(--r-small);
  border: 1px solid var(--border);
  background: var(--accent-bg);
  transition: background var(--t-fast);
}

.action:hover {
  background: var(--accent-bg-hover);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity var(--t-slow) var(--ease), transform var(--t-slow) var(--ease);
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(24px);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(24px);
}

.toast-move {
  transition: transform var(--t-slow) var(--ease);
}
</style>
