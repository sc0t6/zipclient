<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Block } from "../markdown";

defineProps<{ blocks: Block[] }>();
const emit = defineEmits<{ image: [src: string] }>();
</script>

<template>
  <div class="rich">
    <template v-for="(b, i) in blocks" :key="i">
      <component :is="`h${b.level + 2}`" v-if="b.t === 'h'">
        <template v-for="(p, j) in b.inline" :key="j">
          <a v-if="p.t === 'link'" href="#" @click.prevent="openUrl(p.href)">{{ p.v }}</a>
          <span v-else :class="p.t">{{ p.v }}</span>
        </template>
      </component>
      <p v-else-if="b.t === 'p' || b.t === 'quote'" :class="{ quote: b.t === 'quote' }">
        <template v-for="(p, j) in b.inline" :key="j">
          <a v-if="p.t === 'link'" href="#" :title="p.href" @click.prevent="openUrl(p.href)">{{ p.v }}</a>
          <span v-else :class="p.t">{{ p.v }}</span>
        </template>
      </p>
      <component :is="b.ordered ? 'ol' : 'ul'" v-else-if="b.t === 'list'">
        <li v-for="(item, j) in b.items" :key="j">
          <template v-for="(p, k) in item" :key="k">
            <a v-if="p.t === 'link'" href="#" :title="p.href" @click.prevent="openUrl(p.href)">{{ p.v }}</a>
            <span v-else :class="p.t">{{ p.v }}</span>
          </template>
        </li>
      </component>
      <img v-else-if="b.t === 'img'" class="img" :src="b.src" :alt="b.alt" loading="lazy" @click="emit('image', b.src)" @error="($event.target as HTMLImageElement).style.display = 'none'" />
      <pre v-else-if="b.t === 'code'">{{ b.text }}</pre>
      <hr v-else-if="b.t === 'hr'" />
    </template>
  </div>
</template>

<style scoped>
.rich {
  font-size: calc(13px * var(--text-scale, 1));
  line-height: 1.55;
  color: var(--text-dim);
  user-select: text;
  -webkit-user-select: text;
  overflow-wrap: anywhere;
}

.rich h3,
.rich h4,
.rich h5 {
  color: var(--text);
  margin: 18px 0 6px;
  line-height: 1.3;
}

.rich h3 {
  font-size: calc(18px * var(--text-scale, 1));
}

.rich h4 {
  font-size: calc(15px * var(--text-scale, 1));
}

.rich h5 {
  font-size: calc(13px * var(--text-scale, 1));
}

.rich p {
  margin: 0 0 10px;
}

.rich p.quote {
  border-left: 3px solid var(--accent-border);
  padding-left: 10px;
  color: var(--text-faint);
}

.rich ul,
.rich ol {
  margin: 0 0 10px;
  padding-left: 20px;
}

.rich li {
  margin: 2px 0;
}

.rich .b {
  font-weight: 650;
  color: var(--text);
}

.rich .i {
  font-style: italic;
}

.rich .code,
.rich pre {
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 0.92em;
  background: var(--panel-3);
  border-radius: var(--r-small);
}

.rich .code {
  padding: 1px 5px;
}

.rich pre {
  padding: 10px 12px;
  overflow-x: auto;
  white-space: pre;
  margin: 0 0 10px;
}

.rich a {
  color: var(--link);
  text-decoration: none;
}

.rich a:hover {
  text-decoration: underline;
}

.rich .img {
  display: block;
  max-width: 100%;
  max-height: 420px;
  margin: 6px 0 12px;
  border-radius: var(--r-row);
  cursor: zoom-in;
}

.rich hr {
  border: 0;
  border-top: 1px solid var(--border-soft);
  margin: 14px 0;
}
</style>
