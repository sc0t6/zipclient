<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { navTabs } from "../nav";
import { useApp } from "../stores/app";

const props = defineProps<{ compact?: boolean }>();
const app = useApp();
const items = ref<HTMLElement[]>([]);
const box = ref({ top: 0, height: 0, ready: false });
const index = computed(() => navTabs.findIndex((t) => t.id === app.page));

function measure() {
  const el = items.value[index.value];
  if (!el) {
    box.value = { ...box.value, height: 0 };
    return;
  }
  box.value = { top: el.offsetTop, height: el.offsetHeight, ready: box.value.ready || box.value.height > 0 };
}

const observer = new ResizeObserver(() => measure());

watch([index, () => props.compact], () => nextTick(measure));
onMounted(() => {
  measure();
  items.value.forEach((el) => observer.observe(el));
  requestAnimationFrame(() => (box.value.ready = true));
});
onUnmounted(() => observer.disconnect());
</script>

<template>
  <nav class="side panel" :class="{ compact }" aria-label="Main">
    <span class="glider" :class="{ ready: box.ready, hidden: !box.height }" :style="{ transform: `translateY(${box.top}px)`, height: `${box.height}px` }" />
    <button
      v-for="tab in navTabs"
      :key="tab.id"
      ref="items"
      class="item"
      :class="{ active: app.page === tab.id }"
      :aria-current="app.page === tab.id ? 'page' : undefined"
      :aria-label="tab.label"
      :title="tab.label"
      @click="app.go(tab.id)"
    >
      <img :src="tab.icon" class="icon" alt="" />
      <span v-if="!compact">{{ tab.label }}</span>
    </button>
  </nav>
</template>

<style scoped>
.side {
  position: relative;
  width: 96px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 6px;
  margin: 0 0 12px 12px;
  border-radius: 14px;
}

.side.compact {
  width: 52px;
}

.compact .item {
  padding: 10px 0;
}

.glider {
  position: absolute;
  top: 0;
  left: 6px;
  right: 6px;
  border-radius: var(--r-row);
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 20%, transparent), color-mix(in srgb, var(--accent) 9%, transparent));
  box-shadow: inset 0 1px 0 color-mix(in srgb, var(--accent) 30%, transparent), 0 6px 18px -8px var(--accent-glow);
  pointer-events: none;
}

.glider::before {
  content: "";
  position: absolute;
  left: -6px;
  top: 50%;
  width: 3px;
  height: 22px;
  margin-top: -11px;
  border-radius: 0 3px 3px 0;
  background: var(--accent);
  box-shadow: 0 0 10px var(--accent-glow);
}

.glider.ready {
  transition: transform 320ms cubic-bezier(0.22, 1, 0.36, 1), height 320ms cubic-bezier(0.22, 1, 0.36, 1), opacity var(--t-fast);
}

.glider.hidden {
  opacity: 0;
}

.item {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  padding: 9px 0 7px;
  border-radius: var(--r-row);
  color: var(--text-dim);
  font-size: calc(10px * var(--text-scale, 1));
  font-weight: 600;
  transition: background var(--t-fast) var(--ease), color var(--t-fast), transform var(--t-fast) var(--ease);
}

.item span {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item .icon {
  width: 18px;
  height: 18px;
  transition: transform var(--t-med) var(--ease), filter var(--t-fast);
}

.item:hover:not(.active) {
  background: var(--chip-hover);
  color: var(--text);
}

.item:active {
  transform: scale(0.95);
}

.item.active {
  color: var(--accent);
}

.item.active .icon {
  filter: var(--icon-filter) drop-shadow(0 0 5px var(--accent-glow));
  transform: scale(1.08);
}

@media (prefers-reduced-motion: reduce) {
  .glider.ready {
    transition: none;
  }
}
</style>
