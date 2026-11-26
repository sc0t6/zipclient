<script setup lang="ts">
import { navTabs } from "../nav";
import { useApp } from "../stores/app";

defineProps<{ compact?: boolean }>();
const app = useApp();
</script>

<template>
  <nav class="side panel" :class="{ compact }" aria-label="Main">
    <button
      v-for="tab in navTabs"
      :key="tab.id"
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

.item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  padding: 9px 0 7px;
  border-radius: var(--r-row);
  border: 1px solid transparent;
  color: var(--text-dim);
  font-size: calc(10px * var(--text-scale, 1));
  font-weight: 600;
  transition: background var(--t-fast) var(--ease), color var(--t-fast), border-color var(--t-fast), transform var(--t-fast) var(--ease);
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
}

.item:hover {
  background: var(--chip-hover);
  color: var(--text);
}

.item:active {
  transform: scale(0.95);
}

.item.active {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
  color: var(--accent);
  box-shadow: inset 2px 0 0 var(--accent), 0 0 12px color-mix(in srgb, var(--accent) 16%, transparent);
}

.item.active .icon {
  filter: var(--icon-filter) drop-shadow(0 0 4px var(--accent-glow));
}
</style>
