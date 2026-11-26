<script setup lang="ts">
import { ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Modal from "./Modal.vue";
import legal from "../../legal/legal.json";
import { useApp } from "../stores/app";
import { useSettings } from "../stores/settings";

const app = useApp();
const settings = useSettings();
const agreed = ref(false);

function accept() {
  if (settings.s && agreed.value) settings.s.setup.legal = legal.version;
}
</script>

<template>
  <Modal title="Updated terms" :width="460" :dismissable="false" :close-button="false">
    <div class="gate">
      <p>Zip's legal documents were updated on {{ legal.effective }}. Please read them and agree to keep using Zip.</p>
      <div class="docs">
        <button class="btn" @click="app.legalDoc = 'terms'">Terms of Use</button>
        <button class="btn" @click="app.legalDoc = 'privacy'">Privacy Policy</button>
        <button class="btn" @click="app.legalDoc = 'disclaimer'">Legal Disclaimer</button>
      </div>
      <label class="agree"><input v-model="agreed" type="checkbox" /> I have read and agree to the Terms of Use and the Privacy Policy, and I have read the Legal Disclaimer.</label>
      <div class="row end">
        <button class="btn ghost" @click="getCurrentWindow().destroy()">Quit</button>
        <button class="btn primary" :disabled="!agreed" @click="accept">Agree and continue</button>
      </div>
    </div>
  </Modal>
</template>

<style scoped>
.gate {
  padding: 0 22px 20px;
  font-size: calc(12.5px * var(--text-scale, 1));
  color: var(--text-dim);
}

.docs {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin: 12px 0;
}

.agree {
  display: flex;
  gap: 8px;
  align-items: flex-start;
  color: var(--text);
  margin-bottom: 14px;
  line-height: 1.45;
}

.agree input {
  accent-color: var(--accent);
  margin-top: 3px;
}

.end {
  justify-content: flex-end;
}
</style>
