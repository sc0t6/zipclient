<script setup lang="ts">
import { ref, watch } from "vue";
import { face, texture } from "../skin";

const props = withDefaults(defineProps<{ url?: string | null; variant?: string | null; data?: string | null; size?: number; radius?: number }>(), {
  size: 30,
  radius: 2,
});
const src = ref("");

watch(
  () => [props.url, props.variant, props.data],
  async () => {
    try {
      const tex = props.data ?? (await texture(props.url, props.variant));
      src.value = await face(tex);
    } catch {
      src.value = "";
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="head" :style="{ width: `${size}px`, height: `${size}px`, borderRadius: `${radius}px` }">
    <img v-if="src" :src="src" class="pixel" alt="" />
  </div>
</template>

<style scoped>
.head {
  flex: none;
  overflow: hidden;
  background: var(--panel-3);
}

img {
  width: 100%;
  height: 100%;
  display: block;
}
</style>
