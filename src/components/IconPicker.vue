<script setup lang="ts">
import { presetIcons } from "../presets";

defineProps<{ modelValue: string; letter?: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
</script>

<template>
  <div class="picker" role="radiogroup" aria-label="Icon">
    <button
      v-for="p in presetIcons"
      :key="p.id"
      type="button"
      class="pick"
      role="radio"
      :aria-checked="modelValue === `preset:${p.id}`"
      :class="{ on: modelValue === `preset:${p.id}` }"
      :title="p.id"
      @click="emit('update:modelValue', `preset:${p.id}`)"
    >
      <img :src="p.url" alt="" />
    </button>
    <button type="button" class="pick letter" role="radio" :aria-checked="!modelValue" :class="{ on: !modelValue }" title="Letter" @click="emit('update:modelValue', '')">
      {{ (letter || "A").charAt(0).toUpperCase() }}
    </button>
  </div>
</template>

<style scoped>
.picker {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.pick {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  border-radius: 9px;
  background: var(--panel);
  border: 1px solid var(--border-soft);
  transition: background var(--t-fast), border-color var(--t-fast), transform var(--t-fast) var(--ease-spring);
}

.pick img {
  width: 26px;
  height: 26px;
}

.pick:hover {
  background: var(--chip-hover);
  transform: translateY(-1px);
}

.pick.on {
  background: var(--accent-bg-hover);
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-glow);
}

.pick.letter {
  font-weight: 700;
  font-size: 16px;
  color: var(--text-dim);
}
</style>
