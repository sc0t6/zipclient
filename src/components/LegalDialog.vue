<script setup lang="ts">
import { computed } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Modal from "./Modal.vue";
import legal from "../../legal/legal.json";

export type LegalKey = keyof typeof legal.documents;

const props = defineProps<{ doc: LegalKey }>();
const emit = defineEmits<{ close: [] }>();
const document = computed(() => legal.documents[props.doc]);
</script>

<template>
  <Modal :title="document.title" :width="640" @close="emit('close')">
    <div class="legal scroll selectable">
      <p class="meta">Effective {{ legal.effective }} · {{ legal.provider }}</p>
      <p class="intro">{{ document.intro }}</p>
      <section v-for="section in document.sections" :key="section.heading">
        <h3>{{ section.heading }}</h3>
        <p v-for="(text, i) in section.body" :key="i">{{ text }}</p>
      </section>
      <p class="meta">
        <button class="link" @click="openUrl('mailto:' + legal.contactEmail)">{{ legal.contactEmail }}</button> ·
        <button class="link" @click="openUrl(legal.providerUrl)">krizuvo.xyz</button>
      </p>
    </div>
  </Modal>
</template>

<style scoped>
.legal {
  padding: 0 24px 22px;
  max-height: 64vh;
  line-height: 1.55;
  font-size: 12.5px;
  color: var(--text-dim);
  user-select: text;
  -webkit-user-select: text;
}

.intro {
  color: var(--text);
}

h3 {
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  margin: 16px 0 4px;
}

p {
  margin: 0 0 8px;
}

.meta {
  font-size: 11px;
  color: var(--text-faint);
}

.link {
  color: var(--link);
  font-size: 11px;
  text-decoration: underline;
}
</style>
