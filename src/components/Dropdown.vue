<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref } from "vue";

export interface Option {
  value: string;
  label: string;
  hint?: string;
  disabled?: boolean;
}

const props = withDefaults(defineProps<{ modelValue: string; options: Option[]; placeholder?: string; searchable?: boolean; disabled?: boolean }>(), {
  placeholder: "Select",
});
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const open = ref(false);
const trigger = ref<HTMLElement | null>(null);
const list = ref<HTMLElement | null>(null);
const query = ref("");
const highlighted = ref(0);
const position = ref({ left: 0, top: 0, width: 0, maxHeight: 260, above: false });

const selected = computed(() => props.options.find((o) => o.value === props.modelValue));
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  const items = q ? props.options.filter((o) => o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q)) : props.options;
  return items;
});

function place() {
  const rect = trigger.value!.getBoundingClientRect();
  const below = window.innerHeight - rect.bottom - 12;
  const above = rect.top - 58;
  const useAbove = below < 180 && above > below;
  position.value = {
    left: rect.left,
    top: useAbove ? rect.top - 4 : rect.bottom + 4,
    width: Math.max(rect.width, 160),
    maxHeight: Math.min(280, useAbove ? above : below),
    above: useAbove,
  };
}

function outside(event: MouseEvent) {
  const target = event.target as Node;
  if (!trigger.value?.contains(target) && !list.value?.contains(target)) close();
}

async function toggle() {
  if (props.disabled) return;
  if (open.value) return close();
  place();
  query.value = "";
  open.value = true;
  highlighted.value = Math.max(0, filtered.value.findIndex((o) => o.value === props.modelValue));
  window.addEventListener("mousedown", outside, true);
  window.addEventListener("resize", close);
  await nextTick();
  list.value?.querySelector<HTMLElement>(".opt.sel")?.scrollIntoView({ block: "nearest" });
  list.value?.querySelector<HTMLInputElement>("input")?.focus();
}

function close() {
  open.value = false;
  window.removeEventListener("mousedown", outside, true);
  window.removeEventListener("resize", close);
}

function choose(option: Option) {
  if (option.disabled) return;
  emit("update:modelValue", option.value);
  close();
  trigger.value?.focus();
}

function onKey(event: KeyboardEvent) {
  if (!open.value) {
    if (["ArrowDown", "Enter", " "].includes(event.key)) {
      event.preventDefault();
      toggle();
    }
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    close();
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    highlighted.value = Math.min(filtered.value.length - 1, highlighted.value + 1);
    scrollToHighlighted();
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    highlighted.value = Math.max(0, highlighted.value - 1);
    scrollToHighlighted();
  } else if (event.key === "Enter") {
    event.preventDefault();
    const option = filtered.value[highlighted.value];
    if (option) choose(option);
  }
}

function scrollToHighlighted() {
  nextTick(() => list.value?.querySelectorAll<HTMLElement>(".opt")[highlighted.value]?.scrollIntoView({ block: "nearest" }));
}

onUnmounted(close);
</script>

<template>
  <div class="dd-wrap">
  <button ref="trigger" type="button" class="dd" :class="{ open, disabled }" :disabled="disabled" @click="toggle" @keydown="onKey">
    <span class="dd-label ellipsis" :class="{ placeholder: !selected }">{{ selected?.label ?? placeholder }}</span>
    <svg class="chev" viewBox="0 0 10 6" aria-hidden="true"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" /></svg>
  </button>
  <Teleport to="body">
    <Transition name="dd">
      <div
        v-if="open"
        ref="list"
        class="dd-menu"
        :class="{ above: position.above }"
        :style="{
          left: `${position.left}px`,
          top: `${position.top}px`,
          width: `${position.width}px`,
          maxHeight: `${position.maxHeight}px`,
        }"
        @keydown="onKey"
      >
        <input v-if="searchable" v-model="query" class="field dd-search" placeholder="Search.." @input="highlighted = 0" />
        <div class="dd-items scroll">
          <button
            v-for="(option, index) in filtered"
            :key="option.value"
            type="button"
            class="opt"
            :class="{ sel: option.value === modelValue, hl: index === highlighted, off: option.disabled }"
            @mouseenter="highlighted = index"
            @click="choose(option)"
          >
            <span class="ellipsis">{{ option.label }}</span>
            <span v-if="option.hint" class="hint">{{ option.hint }}</span>
          </button>
          <div v-if="!filtered.length" class="none">Nothing found</div>
        </div>
      </div>
    </Transition>
  </Teleport>
  </div>
</template>

<style scoped>
.dd-wrap {
  min-width: 0;
}

.dd {
  height: 30px;
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px 0 12px;
  border-radius: var(--r-small);
  border: 1px solid var(--border);
  background: var(--field);
  font-size: 12px;
  text-align: left;
  transition: border-color var(--t-fast), background var(--t-fast);
}

.dd:hover:not(:disabled) {
  background: var(--chip-hover);
}

.dd.open {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}

.dd:disabled {
  opacity: 0.5;
}

.dd-label {
  flex: 1;
}

.placeholder {
  color: var(--text-faint);
  font-style: italic;
}

.chev {
  width: 10px;
  height: 6px;
  flex: none;
  color: var(--text-dim);
  transition: transform var(--t-med) var(--ease);
}

.dd.open .chev {
  transform: rotate(180deg);
}
</style>

<style>
.dd-menu {
  position: fixed;
  z-index: 500;
  display: flex;
  flex-direction: column;
  background: var(--panel);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  box-shadow: var(--shadow-pop), inset 0 1px 0 var(--hairline);
  padding: 4px;
  font-size: 12px;
  transform-origin: top;
}

.dd-menu.above {
  transform: translateY(-100%);
  transform-origin: bottom;
}

.dd-search {
  margin-bottom: 4px;
  height: 26px;
  flex: none;
}

.dd-items {
  overflow-y: auto;
  min-height: 0;
}

.dd-menu .opt {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: var(--r-small);
  text-align: left;
}

.dd-menu .opt.hl {
  background: var(--chip-hover);
}

.dd-menu .opt.sel {
  background: var(--accent-bg-hover);
  color: var(--accent);
  font-weight: 600;
}

.dd-menu .opt.off {
  opacity: 0.4;
  cursor: not-allowed;
}

.dd-menu .hint {
  margin-left: auto;
  color: var(--text-faint);
  font-size: 10px;
  flex: none;
}

.dd-menu .none {
  padding: 8px;
  color: var(--text-faint);
  font-style: italic;
}

.dd-enter-active,
.dd-leave-active {
  transition: opacity var(--t-fast) var(--ease), scale var(--t-fast) var(--ease);
}

.dd-enter-from,
.dd-leave-to {
  opacity: 0;
  scale: 0.98;
}
</style>
