<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { icons } from "../icons";
import { useApp } from "../stores/app";
import { useSettings } from "../stores/settings";

const emit = defineEmits<{ close: [] }>();
const app = useApp();
const settings = useSettings();
const yes = ref<HTMLButtonElement | null>(null);
const closing = ref(false);

async function exit(remember: boolean) {
  closing.value = true;
  if (remember && settings.s) settings.s.general.confirm_exit = false;
  await settings.flush().catch(() => {});
  await getCurrentWindow().destroy();
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

onMounted(() => {
  window.addEventListener("keydown", onKey);
  yes.value?.focus();
});
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="overlay" @mousedown.self="emit('close')">
    <div class="confirm" role="alertdialog" aria-labelledby="exit-title">
      <div class="head">
        <div class="badge"><img :src="icons.logo" class="icon" alt="" /></div>
        <div>
          <div id="exit-title" class="question">Are you sure you want to close the app?</div>
          <div class="note">{{ app.running.length || app.busy.length ? "Running games keep running after Zip closes." : "Your settings are saved." }}</div>
        </div>
      </div>
      <div class="buttons">
        <button ref="yes" class="choice yes" :disabled="closing" @click="exit(false)">
          <span class="chip"><img :src="icons.check" class="icon" alt="" /></span>
          <span>Yes</span>
        </button>
        <button class="choice cancel" :disabled="closing" @click="emit('close')">
          <span class="chip"><img :src="icons.close" class="icon" alt="" /></span>
          <span>Cancel</span>
        </button>
        <button class="choice never" :disabled="closing" title="Close now and stop asking" @click="exit(true)">
          <span class="chip"><img :src="icons.close" class="icon" alt="" /></span>
          <span class="small">Don't ask<br />again.</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: var(--overlay);
  backdrop-filter: blur(3px);
  display: grid;
  place-items: center;
  z-index: 300;
  animation: fade var(--t-med) var(--ease);
}

@keyframes fade {
  from {
    opacity: 0;
  }
}

.confirm {
  width: 420px;
  max-width: calc(100vw - 24px);
  background: var(--panel);
  border: 1px solid var(--border-soft);
  border-radius: 12px;
  padding: 16px 14px 14px;
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
  animation: rise var(--t-slow) var(--ease);
}

@keyframes rise {
  from {
    transform: translateY(10px) scale(0.98);
    opacity: 0;
  }
}

.head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 2px;
}

.badge {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: var(--panel-3);
  display: grid;
  place-items: center;
  flex: none;
}

.badge .icon {
  width: 22px;
  height: 22px;
}

.question {
  font-size: calc(15px * var(--text-scale, 1));
  font-weight: 600;
}

.note {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-dim);
  margin-top: 3px;
}

.buttons {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin-top: 16px;
}

.choice {
  height: 46px;
  border-radius: 10px;
  border: 1px solid var(--border);
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 10px;
  font-size: calc(15px * var(--text-scale, 1));
  font-style: italic;
  transition: background var(--t-fast), transform var(--t-fast) var(--ease), border-color var(--t-fast);
}

.choice:hover:not(:disabled) {
  transform: translateY(-1px);
  border-color: var(--border-strong);
}

.choice:active:not(:disabled) {
  transform: scale(0.97);
}

.choice:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--accent) 70%, transparent);
}

.chip {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  display: grid;
  place-items: center;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  flex: none;
}

.chip .icon {
  width: 14px;
  height: 14px;
}

.yes {
  background: var(--accent-bg);
}

.yes:hover:not(:disabled) {
  background: var(--accent-bg-hover);
}

.cancel {
  background: var(--danger-bg);
}

.cancel:hover:not(:disabled) {
  background: var(--danger-bg-hover);
}

.never {
  background: var(--panel-2);
}

.never:hover:not(:disabled) {
  background: var(--chip-hover);
}

.small {
  font-size: calc(11px * var(--text-scale, 1));
  line-height: 1.2;
  text-align: left;
}
</style>
