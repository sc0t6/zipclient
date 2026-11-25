<script setup lang="ts" generic="T">
import { computed, onMounted, onUnmounted, ref } from "vue";

const props = withDefaults(defineProps<{ items: T[]; itemHeight: number; gap?: number; overscan?: number; keyOf?: (item: T, index: number) => string | number }>(), {
  gap: 0,
  overscan: 6,
});
const emit = defineEmits<{ end: [] }>();

const viewport = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(400);
let frame = 0;
let observer: ResizeObserver | null = null;

const stride = computed(() => props.itemHeight + props.gap);
const start = computed(() => Math.max(0, Math.floor(scrollTop.value / stride.value) - props.overscan));
const end = computed(() => Math.min(props.items.length, Math.ceil((scrollTop.value + height.value) / stride.value) + props.overscan));
const visible = computed(() => props.items.slice(start.value, end.value).map((item, i) => ({ item, index: start.value + i })));
const total = computed(() => Math.max(0, props.items.length * stride.value - props.gap));

function onScroll() {
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    const el = viewport.value!;
    scrollTop.value = el.scrollTop;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - stride.value * 3) emit("end");
  });
}

defineExpose({
  scrollToTop: () => viewport.value?.scrollTo({ top: 0 }),
  scrollToBottom: () => {
    const el = viewport.value;
    if (el) el.scrollTop = el.scrollHeight;
  },
  atBottom: () => {
    const el = viewport.value;
    return !el || el.scrollTop + el.clientHeight >= el.scrollHeight - stride.value * 2;
  },
});

onMounted(() => {
  observer = new ResizeObserver(() => {
    if (viewport.value) height.value = viewport.value.clientHeight;
  });
  observer.observe(viewport.value!);
  height.value = viewport.value!.clientHeight;
});

onUnmounted(() => {
  observer?.disconnect();
  cancelAnimationFrame(frame);
});
</script>

<template>
  <div ref="viewport" class="vlist scroll" @scroll.passive="onScroll">
    <div class="vlist-inner" :style="{ height: `${total}px` }">
      <div
        v-for="entry in visible"
        :key="keyOf ? keyOf(entry.item, entry.index) : entry.index"
        class="vlist-item"
        :style="{ transform: `translateY(${entry.index * stride}px)`, height: `${itemHeight}px` }"
      >
        <slot :item="entry.item" :index="entry.index" />
      </div>
    </div>
    <slot name="footer" />
  </div>
</template>

<style scoped>
.vlist {
  position: relative;
  overflow-y: auto;
  scroll-behavior: auto;
  contain: strict;
}

.vlist-inner {
  position: relative;
}

.vlist-item {
  position: absolute;
  left: 0;
  right: 0;
  top: 0;
  will-change: transform;
}
</style>
