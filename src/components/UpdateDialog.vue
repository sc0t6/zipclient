<script setup lang="ts">
import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Modal from "./Modal.vue";
import { icons } from "../icons";
import { api, errorText } from "../api";
import { useApp } from "../stores/app";
import { useSettings } from "../stores/settings";

const emit = defineEmits<{ close: [] }>();
const app = useApp();
const settings = useSettings();
const installing = ref(false);
const error = ref("");

async function install() {
  if (app.update?.portable) {
    openUrl("https://github.com/sc0t6/zipclient/releases/latest");
    emit("close");
    return;
  }
  installing.value = true;
  error.value = "";
  try {
    await settings.flush().catch(() => {});
    await api.installUpdate();
  } catch (e) {
    error.value = errorText(e);
    installing.value = false;
  }
}

function skip() {
  if (settings.s && app.update?.version) settings.s.general.skipped_update = app.update.version;
  if (app.update) app.update = { ...app.update, skipped: true };
  emit("close");
}
</script>

<template>
  <Modal :width="440" :close-button="!installing" :dismissable="!installing" @close="emit('close')">
    <div class="update">
      <div class="badge"><img :src="icons.logo" class="icon" alt="" /></div>
      <h2>Hey! There's a update for Zip Client.</h2>
      <p class="ask">Would you like to install it?</p>
      <p class="versions">{{ app.update?.current }} → <b>{{ app.update?.version }}</b></p>
      <pre v-if="app.update?.notes" class="notes selectable">{{ app.update.notes }}</pre>
      <p v-if="app.update?.portable" class="hint">This copy of Zip can't update itself (portable or Flatpak). Download the new version, or update the Flatpak with your software center.</p>
      <p v-if="error" class="err">{{ error }}</p>
      <div class="buttons">
        <button class="btn primary big" :disabled="installing" @click="install">
          <span v-if="installing" class="spinner" />
          {{ installing ? "Installing.." : app.update?.portable ? "Download" : "Install now" }}
        </button>
        <button class="btn big" :disabled="installing" @click="emit('close')">Later</button>
        <button class="btn ghost big" :disabled="installing" @click="skip">Skip this version</button>
      </div>
      <p class="fine">Updates come from the official Zip releases on GitHub and are only installed if their signature matches Zip's release key. Zip restarts after installing.</p>
    </div>
  </Modal>
</template>

<style scoped>
.update {
  padding: 4px 24px 20px;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.badge {
  width: 52px;
  height: 52px;
  border-radius: 14px;
  background: var(--panel);
  display: grid;
  place-items: center;
}

.badge .icon {
  width: 30px;
  height: 30px;
}

h2 {
  font-size: 17px;
  font-weight: 600;
  margin: 14px 0 2px;
}

.ask {
  margin: 0;
  color: var(--text-dim);
}

.versions {
  font-size: 12px;
  color: var(--text-faint);
  margin: 10px 0 0;
}

.notes {
  align-self: stretch;
  text-align: left;
  white-space: pre-wrap;
  font-family: inherit;
  font-size: 12px;
  color: var(--text-dim);
  background: var(--panel);
  border-radius: var(--r-row);
  padding: 10px 12px;
  margin: 12px 0 0;
  max-height: 140px;
  overflow: auto;
}

.buttons {
  display: flex;
  gap: 8px;
  margin-top: 16px;
  flex-wrap: wrap;
  justify-content: center;
}

.btn.big {
  height: 36px;
  padding: 0 16px;
  font-size: 13px;
}

.hint,
.fine {
  font-size: 11px;
  color: var(--text-faint);
  margin: 12px 0 0;
}

.err {
  font-size: 12px;
  color: var(--danger-text);
  margin: 10px 0 0;
}
</style>
