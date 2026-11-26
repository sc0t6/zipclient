<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { icons } from "../icons";

const props = withDefaults(defineProps<{ title?: string; width?: number; dismissable?: boolean; closeButton?: boolean }>(), {
  width: 560,
  dismissable: true,
  closeButton: true,
});
const emit = defineEmits<{ close: [] }>();

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape" && props.dismissable) emit("close");
}

onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="overlay" @mousedown.self="dismissable && emit('close')">
    <div class="dialog" role="dialog" :aria-label="title" :style="{ width: `min(${width}px, calc(100vw - 32px))` }">
      <div v-if="title || closeButton" class="dialog-head">
        <span class="dialog-title">{{ title }}</span>
        <button v-if="closeButton" class="chip-btn dialog-x" title="Close" @click="emit('close')">
          <img :src="icons.close" class="icon" alt="" />
        </button>
      </div>
      <slot />
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 53px 0 0;
  background: var(--overlay);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  display: grid;
  place-items: center;
  z-index: 100;
  padding: 16px;
}

.dialog {
  position: relative;
  background: var(--panel);
  border-radius: var(--r-panel);
  box-shadow: var(--shadow-modal), inset 0 1px 0 var(--hairline);
  border: 1px solid var(--border-soft);
  max-height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
  animation: rise var(--t-slow) var(--ease);
}

@keyframes rise {
  from {
    transform: translateY(10px) scale(0.97);
    opacity: 0;
  }
}

.dialog-head {
  position: relative;
  height: 50px;
  flex: none;
  display: grid;
  place-items: center;
  border-bottom: 1px solid var(--border-soft);
}

.dialog-title {
  font-size: calc(18px * var(--text-scale, 1));
  font-weight: 600;
  letter-spacing: -0.01em;
}

.dialog-x {
  position: absolute;
  right: 12px;
  top: 12px;
  background: var(--panel-2);
}

.dialog-x .icon {
  width: 14px;
  height: 14px;
}
</style>
