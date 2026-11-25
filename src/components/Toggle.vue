<script setup lang="ts">
defineProps<{ modelValue: boolean; disabled?: boolean; label?: string }>();
const emit = defineEmits<{ "update:modelValue": [value: boolean] }>();
</script>

<template>
  <button
    class="toggle"
    role="switch"
    :aria-checked="modelValue"
    :aria-label="label"
    :class="{ on: modelValue }"
    :disabled="disabled"
    @click="emit('update:modelValue', !modelValue)"
  >
    <span class="knob" />
  </button>
</template>

<style scoped>
.toggle {
  width: 34px;
  height: 19px;
  border-radius: 10px;
  background: var(--panel-3);
  border: 1px solid var(--border);
  position: relative;
  flex: none;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.2);
  transition: background var(--t-med) var(--ease), border-color var(--t-med) var(--ease), box-shadow var(--t-med) var(--ease), opacity var(--t-fast);
}

.toggle:hover:not(:disabled) {
  border-color: var(--border-strong);
}

.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: var(--text-faint);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
  transition: transform var(--t-med) var(--ease-spring), background var(--t-med) var(--ease), box-shadow var(--t-med) var(--ease);
}

.toggle.on {
  background: color-mix(in srgb, var(--accent) 30%, var(--panel));
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.15), 0 0 10px var(--accent-glow);
}

.toggle.on .knob {
  transform: translateX(15px);
  background: var(--accent);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4), 0 0 6px var(--accent-glow);
}

.toggle:disabled {
  opacity: 0.4;
}
</style>
