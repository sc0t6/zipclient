<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import Modal from "./Modal.vue";
import { api, errorText, type DeviceCodeInfo } from "../api";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";

const emit = defineEmits<{ close: []; added: [] }>();
const app = useApp();
const notify = useNotify();

const code = ref<DeviceCodeInfo | null>(null);
const status = ref("");
const error = ref("");
const loading = ref(false);
const offlineName = ref("");
const offlineError = ref("");
const copied = ref(false);
let unlisten: UnlistenFn | null = null;

const configured = computed(() => app.info?.msConfigured ?? false);
const linkLabel = computed(() => (code.value?.verificationUri ?? "https://www.microsoft.com/link").replace(/^https?:\/\/(www\.)?/, ""));
const validOffline = computed(() => /^[A-Za-z0-9_]{3,16}$/.test(offlineName.value.trim()));

async function start() {
  if (!configured.value) return;
  error.value = "";
  status.value = "";
  loading.value = true;
  try {
    code.value = await api.beginMicrosoft();
    status.value = "Waiting for you to sign in..";
  } catch (e) {
    error.value = errorText(e);
  } finally {
    loading.value = false;
  }
}

async function openLink() {
  if (!code.value) return;
  await copyCode();
  openUrl(code.value.verificationUri);
}

async function copyCode() {
  if (!code.value) return;
  await navigator.clipboard.writeText(code.value.userCode).catch(() => {});
  copied.value = true;
  window.setTimeout(() => (copied.value = false), 1500);
}

async function addOffline() {
  offlineError.value = "";
  if (!validOffline.value) {
    offlineError.value = "Use 3-16 letters, numbers or underscores.";
    return;
  }
  try {
    const account = await api.addOffline(offlineName.value.trim());
    await app.refreshAccounts();
    notify.success("Offline profile added", `${account.name} can only join offline-mode servers and singleplayer.`);
    emit("added");
    emit("close");
  } catch (e) {
    offlineError.value = errorText(e);
  }
}

function close() {
  api.cancelMicrosoft().catch(() => {});
  emit("close");
}

onMounted(async () => {
  unlisten = await listen<{ status: string; message: string }>("auth-event", async ({ payload }) => {
    if (payload.status === "progress") status.value = payload.message;
    if (payload.status === "error") {
      error.value = payload.message;
      status.value = "";
    }
    if (payload.status === "success") {
      await app.refreshAccounts();
      notify.success("Account connected", `Signed in as ${payload.message}.`);
      emit("added");
      emit("close");
    }
  });
  start();
});

onUnmounted(() => {
  unlisten?.();
  api.cancelMicrosoft().catch(() => {});
});
</script>

<template>
  <Modal title="Login" :width="597" @close="close">
    <div class="login">
      <template v-if="configured">
        <div class="lead">Your login code is:</div>
        <button class="code selectable" :class="{ loading: loading || !code }" title="Click to copy" @click="copyCode">
          <span v-if="code">{{ code.userCode }}</span>
          <span v-else-if="loading" class="spinner big" />
          <span v-else class="placeholder">- - - - - - - -</span>
        </button>
        <div class="copied" :class="{ show: copied }">Copied to clipboard</div>
        <div class="where">Enter the code at:</div>
        <button class="link" :disabled="!code" @click="openLink">{{ linkLabel }}</button>
        <div class="where">to continue with servers.</div>
        <div class="state">
          <template v-if="error">
            <span class="err">{{ error }}</span>
            <button class="btn small" @click="start">Try again</button>
          </template>
          <template v-else-if="status">
            <span class="spinner" />
            <span>{{ status }}</span>
          </template>
        </div>
      </template>
      <div v-else class="unconfigured">
        <div class="lead">Microsoft sign-in is not set up</div>
        <p>
          Zip needs an Azure application client ID approved for Minecraft services before it can sign in with Microsoft. Add one in
          <b>Settings › Advanced</b>. See the README for setup steps.
        </p>
      </div>
      <div class="offline">
        <div class="hint">or you could just use a offline account.</div>
        <div class="row">
          <input v-model="offlineName" class="field pill name" maxlength="16" placeholder="Enter the name.." @keydown.enter="addOffline" />
          <button class="btn primary add" :disabled="!offlineName" @click="addOffline">+ Add account</button>
        </div>
        <div v-if="offlineError" class="err small">{{ offlineError }}</div>
      </div>
    </div>
  </Modal>
</template>

<style scoped>
.login {
  padding: 4px 27px 30px;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.lead {
  font-size: calc(24px * var(--text-scale, 1));
  margin-top: 6px;
}

.code {
  font-size: calc(64px * var(--text-scale, 1));
  margin: 14px 0 0;
  letter-spacing: 0.12em;
  min-height: 80px;
  display: grid;
  place-items: center;
  border-radius: var(--r-row);
  padding: 0 14px;
  transition: background var(--t-fast);
}

.code:hover {
  background: var(--panel);
}

.placeholder {
  color: var(--text-ghost);
  font-size: calc(40px * var(--text-scale, 1));
}

.spinner.big {
  width: 36px;
  height: 36px;
  border-width: 3px;
}

.copied {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
  height: 14px;
  opacity: 0;
  transition: opacity var(--t-med);
}

.copied.show {
  opacity: 1;
}

.where {
  font-size: calc(16px * var(--text-scale, 1));
}

.link {
  font-size: calc(18px * var(--text-scale, 1));
  font-style: italic;
  color: var(--link);
  margin: 2px 0;
  transition: color var(--t-fast);
}

.link:hover:not(:disabled) {
  color: color-mix(in srgb, var(--link) 60%, var(--text));
  text-decoration: underline;
}

.state {
  min-height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin-top: 8px;
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
}

.err {
  color: var(--danger-text);
  font-size: calc(12px * var(--text-scale, 1));
}

.err.small {
  margin-top: 6px;
  align-self: flex-start;
}

.unconfigured {
  margin: 12px 0 24px;
  max-width: 440px;
}

.unconfigured p {
  color: var(--text-dim);
  font-size: calc(13px * var(--text-scale, 1));
  line-height: 1.5;
}

.offline {
  align-self: stretch;
  text-align: left;
  margin-top: 16px;
}

.hint {
  font-size: calc(12px * var(--text-scale, 1));
  margin-bottom: 7px;
}

.name {
  width: 325px;
  max-width: 60%;
  height: 33px;
}

.add {
  height: 30px;
}
</style>
