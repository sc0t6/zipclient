<script setup lang="ts">
import { computed } from "vue";
import type { Instance } from "../api";
import { useApp } from "../stores/app";
import { presetUrl } from "../presets";

const props = withDefaults(defineProps<{ instance: Instance; size?: number }>(), { size: 30 });
const app = useApp();

const tones: Record<string, string> = {
  vanilla: "#6f9a5a",
  fabric: "#c8b48a",
  forge: "#5a6f9a",
  neoforge: "#d0743c",
  quilt: "#9a6fb0",
  horizon: "#b7410e",
};

const preset = computed(() => presetUrl(props.instance.icon));
const src = computed(() => preset.value ?? app.iconFor(props.instance));
const tone = computed(() => tones[props.instance.loader] ?? "#777");
</script>

<template>
  <div class="inst-icon" :style="{ width: `${size}px`, height: `${size}px`, '--tone': tone, fontSize: `${Math.round(size * 0.45)}px` }">
    <img v-if="src" :src="src" :class="{ preset }" alt="" />
    <span v-else>{{ instance.name.charAt(0).toUpperCase() }}</span>
  </div>
</template>

<style scoped>
.inst-icon {
  border-radius: 6px;
  flex: none;
  overflow: hidden;
  display: grid;
  place-items: center;
  font-weight: 700;
  color: color-mix(in srgb, var(--tone) 85%, #fff);
  background: color-mix(in srgb, var(--tone) 22%, var(--panel-3));
  border: 1px solid color-mix(in srgb, var(--tone) 30%, transparent);
}

img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

img.preset {
  width: 74%;
  height: 74%;
  object-fit: contain;
}
</style>
