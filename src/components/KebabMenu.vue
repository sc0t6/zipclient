<script setup lang="ts">
import { nextTick, onUnmounted, ref } from "vue";

export interface MenuItem {
  label: string;
  run: () => void;
  danger?: boolean;
  disabled?: boolean;
}

const props = withDefaults(defineProps<{ items: MenuItem[]; size?: number }>(), { size: 36 });

const open = ref(false);
const button = ref<HTMLElement | null>(null);
const menu = ref<HTMLElement | null>(null);
const pos = ref({ top: 0, left: 0, above: false });

function place() {
  const anchor = button.value?.getBoundingClientRect();
  if (!anchor) return;
  const width = menu.value?.offsetWidth ?? 180;
  const height = menu.value?.offsetHeight ?? props.items.length * 31 + 10;
  const above = window.innerHeight - anchor.bottom < height + 14 && anchor.top > height + 14;
  pos.value = {
    left: Math.min(window.innerWidth - width - 8, Math.max(8, anchor.right - width)),
    top: above ? anchor.top - height - 6 : anchor.bottom + 6,
    above,
  };
}

function outside(event: Event) {
  const target = event.target as Node;
  if (!button.value?.contains(target) && !menu.value?.contains(target)) close();
}

function onScroll(event: Event) {
  if (!menu.value?.contains(event.target as Node)) close();
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") close();
}

async function toggle() {
  if (open.value) return close();
  place();
  open.value = true;
  window.addEventListener("mousedown", outside, true);
  window.addEventListener("scroll", onScroll, true);
  window.addEventListener("resize", close);
  window.addEventListener("keydown", onKey);
  await nextTick();
  place();
}

function close() {
  open.value = false;
  window.removeEventListener("mousedown", outside, true);
  window.removeEventListener("scroll", onScroll, true);
  window.removeEventListener("resize", close);
  window.removeEventListener("keydown", onKey);
}

function pick(item: MenuItem) {
  if (item.disabled) return;
  close();
  item.run();
}

onUnmounted(close);
</script>

<template>
  <button ref="button" type="button" class="kebab" :class="{ on: open }" title="More" :style="{ width: `${size}px`, height: `${size}px` }" @click.stop="toggle">
    <span /><span /><span />
  </button>
  <Teleport to="body">
    <Transition name="pop">
      <div v-if="open" ref="menu" class="kmenu" :class="{ above: pos.above }" :style="{ top: `${pos.top}px`, left: `${pos.left}px` }" role="menu">
        <button v-for="item in items" :key="item.label" type="button" role="menuitem" :class="{ danger: item.danger }" :disabled="item.disabled" @click="pick(item)">
          {{ item.label }}
        </button>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.kebab {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  border-radius: var(--r-row);
  background: var(--panel-3);
  border: 1px solid transparent;
  transition: background var(--t-fast), border-color var(--t-fast), transform var(--t-fast);
}

.kebab:hover,
.kebab.on {
  background: var(--chip-hover);
  border-color: var(--border-soft);
}

.kebab:active {
  transform: scale(0.94);
}

.kebab span {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--text-dim);
}

.kmenu {
  position: fixed;
  z-index: 600;
  min-width: 180px;
  padding: 4px;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: var(--r-row);
  box-shadow: var(--shadow-pop);
  transform-origin: top right;
}

.kmenu.above {
  transform-origin: bottom right;
}

.kmenu button {
  display: block;
  width: 100%;
  text-align: left;
  padding: 7px 10px;
  border-radius: var(--r-small);
  font-size: 12px;
  white-space: nowrap;
  transition: background var(--t-fast);
}

.kmenu button:hover:not(:disabled) {
  background: var(--chip-hover);
}

.kmenu button:disabled {
  opacity: 0.45;
}

.kmenu button.danger {
  color: var(--danger-text);
}
</style>
