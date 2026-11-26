<script setup lang="ts" generic="T">
import { computed, onMounted, onUnmounted, ref } from "vue";

const props = withDefaults(
  defineProps<{ items: T[]; itemHeight: number; heightOf?: (item: T) => number; gap?: number; overscan?: number; keyOf?: (item: T, index: number) => string | number }>(),
  {
    gap: 0,
    overscan: 6,
  },
);
const emit = defineEmits<{ end: [] }>();

const viewport = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(400);
let frame = 0;
let observer: ResizeObserver | null = null;

const stride = computed(() => props.itemHeight + props.gap);
const sizeOf = (item: T) => (props.heightOf ? props.heightOf(item) : props.itemHeight);
const offsets = computed(() => {
  const out = [0];
  for (const item of props.items) out.push(out[out.length - 1] + sizeOf(item) + props.gap);
  return out;
});

function indexAt(y: number) {
  const list = offsets.value;
  let lo = 0;
  let hi = props.items.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (list[mid + 1] <= y) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

const start = computed(() => Math.max(0, indexAt(scrollTop.value) - props.overscan));
const end = computed(() => Math.min(props.items.length, indexAt(scrollTop.value + height.value) + 1 + props.overscan));
const visible = computed(() => props.items.slice(start.value, end.value).map((item, i) => ({ item, index: start.value + i })));
const total = computed(() => Math.max(0, offsets.value[props.items.length] - props.gap));

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
        :style="{ transform: `translateY(${offsets[entry.index]}px)`, height: `${sizeOf(entry.item)}px` }"
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
