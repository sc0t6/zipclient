<script setup lang="ts">
import { computed, nextTick, onActivated, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import Dropdown from "../components/Dropdown.vue";
import KebabMenu, { type MenuItem } from "../components/KebabMenu.vue";
import Modal from "../components/Modal.vue";
import { api, errorText, rangeFill, type HostedServer, type SavedServer, type ServerStatus } from "../api";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";

const app = useApp();
const notify = useNotify();

const tab = ref<"saved" | "host">("saved");
const saved = ref<SavedServer[]>([]);
const status = reactive<Record<string, ServerStatus | null>>({});
const hosted = ref<HostedServer[]>([]);
const lan = ref("");
const logs = reactive<Record<string, string[]>>({});
const consoleFor = ref("");
const commandLine = ref("");
const consoleEl = ref<HTMLElement[]>([]);
const unlisten: UnlistenFn[] = [];

const addOpen = ref(false);
const addForm = reactive({ id: "", name: "", address: "" });
const addError = ref("");

const hostOpen = ref(false);
const hostForm = reactive({ name: "", minecraft: "", memory: 2048, port: 25565, motd: "", eula: false, understood: false });
const hostError = ref("");
const creating = ref(false);

const instanceOptions = computed(() =>
  app.instances.filter((i) => i.loader !== "horizon").map((i) => ({ value: i.id, label: i.name, hint: `${i.loader === "vanilla" ? "" : i.loader + " "}${i.minecraft}` })),
);
const releaseOptions = computed(() => (app.versions?.versions ?? []).filter((v) => v.type === "release").map((v) => ({ value: v.id, label: v.id })));
const maxMemory = computed(() => 16384);
const canHost = computed(() => !!hostForm.name.trim() && !!hostForm.minecraft && hostForm.eula && hostForm.understood && !creating.value);
const playWith = reactive<Record<string, string>>({});

function defaultInstance(server: SavedServer) {
  if (server.instanceId && app.instances.some((i) => i.id === server.instanceId)) return server.instanceId;
  const quick = app.quick;
  return quick && quick.loader !== "horizon" ? quick.id : instanceOptions.value[0]?.value ?? "";
}

async function loadSaved() {
  saved.value = await api.savedServers().catch(() => []);
  for (const server of saved.value) {
    if (!playWith[server.id]) playWith[server.id] = defaultInstance(server);
  }
  pingAll();
}

watch(instanceOptions, (options) => {
  for (const server of saved.value) {
    if (!options.some((o) => o.value === playWith[server.id])) playWith[server.id] = defaultInstance(server);
  }
});

function pingAll() {
  for (const server of saved.value) ping(server);
}

async function ping(server: SavedServer) {
  status[server.id] = null;
  status[server.id] = await api.pingServer(server.address).catch((e) => ({ online: false, error: errorText(e) }) as ServerStatus);
}

function openAdd(server?: SavedServer) {
  addForm.id = server?.id ?? "";
  addForm.name = server?.name ?? "";
  addForm.address = server?.address ?? "";
  addError.value = "";
  addOpen.value = true;
}

async function submitAdd() {
  addError.value = "";
  try {
    const existing = saved.value.find((s) => s.id === addForm.id);
    const server = await api.saveServer({ id: addForm.id, name: addForm.name, address: addForm.address, instanceId: existing?.instanceId ?? "" });
    addOpen.value = false;
    await loadSaved();
    ping(server);
  } catch (e) {
    addError.value = errorText(e);
  }
}

async function removeSaved(server: SavedServer) {
  await api.removeServer(server.id).catch((e) => notify.error("Could not remove server", errorText(e)));
  saved.value = saved.value.filter((s) => s.id !== server.id);
}

async function play(server: SavedServer) {
  const id = playWith[server.id];
  if (!id) {
    notify.warn("No instance to play with", "Create a Minecraft instance first.");
    return;
  }
  if (server.instanceId !== id) api.saveServer({ ...server, instanceId: id }).catch(() => {});
  await app.launch(id, server.address);
}

function copy(text: string) {
  navigator.clipboard.writeText(text).then(() => notify.success("Copied", text)).catch(() => {});
}

function savedMenu(server: SavedServer): MenuItem[] {
  return [
    { label: "Refresh", run: () => ping(server) },
    { label: "Edit", run: () => openAdd(server) },
    { label: "Copy address", run: () => copy(server.address) },
    { label: "Remove", run: () => removeSaved(server), danger: true },
  ];
}

async function loadHosted() {
  hosted.value = await api.hostedServers().catch(() => []);
  if (!lan.value) lan.value = (await api.lanAddress().catch(() => null)) ?? "";
}

function openHost() {
  if (!app.versions) app.loadVersions();
  const used = new Set(hosted.value.map((s) => s.port));
  let port = 25565;
  while (used.has(port)) port++;
  Object.assign(hostForm, { name: "", minecraft: app.versions?.latestRelease ?? "", memory: 2048, port, motd: "", eula: false, understood: false });
  hostError.value = "";
  hostOpen.value = true;
}

async function submitHost() {
  if (!canHost.value) return;
  creating.value = true;
  hostError.value = "";
  try {
    const server = await api.createHosted({
      name: hostForm.name,
      minecraft: hostForm.minecraft || app.versions?.latestRelease || "",
      memory: hostForm.memory,
      port: hostForm.port,
      motd: hostForm.motd,
      acceptEula: hostForm.eula,
    });
    hosted.value = [...hosted.value, server];
    hostOpen.value = false;
    notify.success("Server created", `${server.name} downloads when you press Start.`);
  } catch (e) {
    hostError.value = errorText(e);
  } finally {
    creating.value = false;
  }
}

function setHostedState(id: string, state: HostedServer["state"]) {
  const server = hosted.value.find((s) => s.id === id);
  if (server) server.state = state;
}

async function start(server: HostedServer) {
  setHostedState(server.id, "preparing");
  logs[server.id] = [];
  try {
    await api.startHosted(server.id);
  } catch (e) {
    setHostedState(server.id, "stopped");
    notify.error(`Could not start ${server.name}`, errorText(e));
  }
}

async function stop(server: HostedServer) {
  try {
    await api.stopHosted(server.id);
  } catch (e) {
    notify.error(`Could not stop ${server.name}`, errorText(e));
  }
}

async function toggleConsole(server: HostedServer) {
  if (consoleFor.value === server.id) {
    consoleFor.value = "";
    return;
  }
  consoleFor.value = server.id;
  logs[server.id] = await api.hostedLogs(server.id).catch(() => []);
  scrollConsole();
}

async function sendCommand(server: HostedServer) {
  const line = commandLine.value.trim();
  if (!line) return;
  commandLine.value = "";
  try {
    await api.hostedCommand(server.id, line);
  } catch (e) {
    notify.error("Command failed", errorText(e));
  }
}

function scrollConsole() {
  nextTick(() => {
    const el = consoleEl.value[0];
    if (el) el.scrollTop = el.scrollHeight;
  });
}

async function addHostedToSaved(server: HostedServer) {
  try {
    await api.saveServer({ name: server.name, address: `localhost:${server.port}` });
    await loadSaved();
    tab.value = "saved";
  } catch (e) {
    notify.error("Could not add server", errorText(e));
  }
}

async function removeHosted(server: HostedServer) {
  try {
    await api.deleteHosted(server.id);
    hosted.value = hosted.value.filter((s) => s.id !== server.id);
    notify.info("Server deleted", `${server.name} and its world were removed.`);
  } catch (e) {
    notify.error("Could not delete server", errorText(e));
  }
}

const pendingDelete = ref("");

function hostedMenu(server: HostedServer): MenuItem[] {
  return [
    { label: "Add to my servers", run: () => addHostedToSaved(server) },
    { label: "Copy local address", run: () => copy(`localhost:${server.port}`) },
    ...(lan.value ? [{ label: "Copy LAN address", run: () => copy(`${lan.value}:${server.port}`) }] : []),
    { label: "Open folder", run: () => api.openHostedFolder(server.id) },
    {
      label: pendingDelete.value === server.id ? "Click again to delete" : "Delete server",
      danger: true,
      disabled: server.state !== "stopped",
      run: () => {
        if (pendingDelete.value === server.id) {
          pendingDelete.value = "";
          removeHosted(server);
        } else {
          pendingDelete.value = server.id;
          window.setTimeout(() => pendingDelete.value === server.id && (pendingDelete.value = ""), 4000);
        }
      },
    },
  ];
}

const stateLabels: Record<HostedServer["state"], string> = {
  stopped: "Stopped",
  preparing: "Downloading",
  starting: "Starting",
  running: "Online",
  stopping: "Stopping",
};

const gb = (mb: number) => `${(mb / 1024).toFixed(mb % 1024 ? 1 : 0)} GB`;

onMounted(async () => {
  unlisten.push(
    await listen<{ id: string; state: HostedServer["state"]; message: string | null }>("server-state", ({ payload }) => {
      setHostedState(payload.id, payload.state);
      const name = hosted.value.find((s) => s.id === payload.id)?.name ?? "Server";
      if (payload.state === "running") notify.success(`${name} is online`, `Join with localhost:${hosted.value.find((s) => s.id === payload.id)?.port ?? 25565}`);
      if (payload.state === "stopped" && payload.message) notify.error(`${name} stopped`, payload.message);
    }),
    await listen<{ id: string; lines: string[] }>("server-log", ({ payload }) => {
      const list = (logs[payload.id] ??= []);
      list.push(...payload.lines);
      if (list.length > 2000) list.splice(0, list.length - 2000);
      if (consoleFor.value === payload.id) scrollConsole();
    }),
  );
  loadSaved();
  loadHosted();
});

onActivated(() => {
  if (saved.value.length) pingAll();
});

onUnmounted(() => unlisten.forEach((u) => u()));
</script>

<template>
  <div class="servers">
    <div class="head">
      <div class="seg">
        <button :class="{ on: tab === 'saved' }" @click="tab = 'saved'">My servers</button>
        <button :class="{ on: tab === 'host' }" @click="tab = 'host'">Host your own</button>
      </div>
      <span class="grow" />
      <template v-if="tab === 'saved'">
        <button class="btn" :disabled="!saved.length" @click="pingAll">Refresh</button>
        <button class="btn primary" @click="openAdd()">+ Add server</button>
      </template>
      <button v-else class="btn primary" @click="openHost">+ Add your own server</button>
    </div>

    <div v-if="tab === 'saved'" class="list scroll">
      <div v-for="server in saved" :key="server.id" class="row panel">
        <div class="favicon">
          <img v-if="status[server.id]?.favicon" :src="status[server.id]!.favicon" class="pixel" alt="" />
          <span v-else>{{ server.name.charAt(0).toUpperCase() }}</span>
        </div>
        <div class="info">
          <div class="line">
            <span class="name ellipsis">{{ server.name }}</span>
            <span class="addr ellipsis selectable" :title="status[server.id]?.address && status[server.id]!.address !== server.address ? `Connects to ${status[server.id]!.address}` : ''">{{ server.address }}</span>
          </div>
          <div class="motd" :class="{ multi: status[server.id]?.online }">
            <template v-if="status[server.id] === null || status[server.id] === undefined"><span class="spinner" /> Pinging..</template>
            <span v-else-if="status[server.id]!.online" class="motd-text selectable">{{ status[server.id]!.motd || "A Minecraft server" }}</span>
            <span v-else class="off">Can't reach server · {{ status[server.id]!.error }}</span>
          </div>
        </div>
        <div v-if="status[server.id]?.online" class="stats">
          <span class="pill-stat">{{ status[server.id]!.playersOnline }}/{{ status[server.id]!.playersMax }} online</span>
          <span class="pill-stat" :class="{ slow: status[server.id]!.latency > 150 }">{{ status[server.id]!.latency }} ms</span>
          <span class="ver ellipsis" :title="status[server.id]!.version">{{ status[server.id]!.version }}</span>
        </div>
        <Dropdown v-model="playWith[server.id]" :options="instanceOptions" placeholder="Pick instance" class="pick" />
        <button class="btn primary play" :disabled="!playWith[server.id] || !!app.games[playWith[server.id]]" @click="play(server)">
          {{ app.games[playWith[server.id]] ? "Running" : "Play" }}
        </button>
        <KebabMenu :items="savedMenu(server)" />
      </div>
      <div v-if="!saved.length" class="blank">
        <b>No servers yet</b>
        <span>Add a server address to see who is online and join in one click.</span>
        <span class="hint">Saved servers also appear in the multiplayer list of every instance.</span>
        <button class="btn primary" @click="openAdd()">+ Add server</button>
      </div>
    </div>

    <div v-else class="list scroll">
      <div class="warning">
        <b>This is not a free VPS.</b>
        <span>
          Servers here run on <b>your own computer</b>, using your CPU, RAM and internet. They are only online while this PC and Zip are on. Friends on your Wi-Fi can join with your LAN
          address; anyone else needs port forwarding on your router, which exposes your PC to the internet. Only share your address with people you trust.
        </span>
      </div>
      <div v-for="server in hosted" :key="server.id" class="host panel" :class="server.state">
        <div class="row-inner">
          <span class="state-dot" />
          <div class="info">
            <div class="line">
              <span class="name ellipsis">{{ server.name }}</span>
              <span class="addr">Minecraft {{ server.minecraft }} · {{ gb(server.memory) }} · port {{ server.port }}</span>
            </div>
            <div class="motd">
              {{ stateLabels[server.state] }}
              <template v-if="server.state === 'preparing' && app.progress[`server:${server.id}`]">
                · {{ app.progress[`server:${server.id}`].phase }} {{ Math.round(app.progress[`server:${server.id}`].fraction * 100) }}%
              </template>
              <template v-if="server.state === 'running'">
                · <span class="selectable">localhost:{{ server.port }}</span><template v-if="lan"> · LAN <span class="selectable">{{ lan }}:{{ server.port }}</span></template>
              </template>
            </div>
          </div>
          <button class="btn" :class="{ on: consoleFor === server.id }" @click="toggleConsole(server)">Console</button>
          <button v-if="server.state === 'stopped'" class="btn primary" @click="start(server)">Start</button>
          <button v-else class="btn danger" :disabled="server.state === 'preparing' || server.state === 'stopping'" @click="stop(server)">
            <span v-if="server.state !== 'running'" class="spinner" />
            {{ server.state === "stopping" ? "Stopping" : "Stop" }}
          </button>
          <KebabMenu :items="hostedMenu(server)" />
        </div>
        <div v-if="consoleFor === server.id" class="console">
          <div ref="consoleEl" class="console-log scroll selectable">
            <div v-for="(line, i) in logs[server.id] ?? []" :key="i">{{ line }}</div>
            <div v-if="!(logs[server.id] ?? []).length" class="muted">No output yet. Press Start to boot the server.</div>
          </div>
          <form class="console-input" @submit.prevent="sendCommand(server)">
            <input v-model="commandLine" class="field" :disabled="server.state !== 'running'" placeholder="Type a command, like op YourName or whitelist add Friend" />
            <button class="btn" :disabled="server.state !== 'running' || !commandLine.trim()">Send</button>
          </form>
        </div>
      </div>
      <div v-if="!hosted.length" class="blank">
        <b>Host a world for your friends</b>
        <span>Zip downloads the official Minecraft server from Mojang and runs it on this PC.</span>
        <button class="btn primary" @click="openHost">+ Add your own server</button>
      </div>
    </div>

    <Modal v-if="addOpen" :title="addForm.id ? 'Edit server' : 'Add server'" :width="440" @close="addOpen = false">
      <form class="form" @submit.prevent="submitAdd">
        <label class="label">Name</label>
        <input v-model="addForm.name" class="field" maxlength="48" placeholder="My server" />
        <label class="label">Address</label>
        <input v-model="addForm.address" class="field" maxlength="255" placeholder="play.example.com or 127.0.0.1:25565" />
        <small class="note">Saved servers also appear in the multiplayer list of every instance.</small>
        <div v-if="addError" class="err">{{ addError }}</div>
        <div class="footer">
          <button type="button" class="btn" @click="addOpen = false">Cancel</button>
          <button class="btn primary" :disabled="!addForm.address.trim()">{{ addForm.id ? "Save" : "Add server" }}</button>
        </div>
      </form>
    </Modal>

    <Modal v-if="hostOpen" title="Add your own server" :width="520" :dismissable="!creating" @close="!creating && (hostOpen = false)">
      <form class="form" @submit.prevent="submitHost">
        <div class="warning small">
          <b>Runs on your PC, not in the cloud.</b>
          <span>This is not free hosting or a VPS. It stops when you close Zip or turn off your computer.</span>
        </div>
        <label class="label">Name</label>
        <input v-model="hostForm.name" class="field" maxlength="40" placeholder="Survival with friends" />
        <div class="two">
          <div>
            <label class="label">Minecraft version</label>
            <Dropdown v-model="hostForm.minecraft" :options="releaseOptions" searchable placeholder="Loading versions.." />
          </div>
          <div>
            <label class="label">Port</label>
            <input v-model.number="hostForm.port" class="field" type="number" min="1024" max="65535" />
          </div>
        </div>
        <label class="label">Memory · {{ gb(hostForm.memory) }}</label>
        <input v-model.number="hostForm.memory" type="range" min="1024" :max="maxMemory" step="512" :style="rangeFill(hostForm.memory, 1024, maxMemory)" />
        <label class="label">Message of the day (optional)</label>
        <input v-model="hostForm.motd" class="field" maxlength="59" placeholder="A Zip server" />
        <label class="check"><input v-model="hostForm.eula" type="checkbox" /><span>I accept the <a href="#" @click.prevent="openUrl('https://aka.ms/MinecraftEULA')">Minecraft EULA</a>.</span></label>
        <label class="check"><input v-model="hostForm.understood" type="checkbox" /><span>I understand this server runs on my own hardware and is not a free VPS.</span></label>
        <div v-if="hostError" class="err">{{ hostError }}</div>
        <div class="footer">
          <button type="button" class="btn" :disabled="creating" @click="hostOpen = false">Cancel</button>
          <button class="btn primary" :disabled="!canHost">
            <span v-if="creating" class="spinner" />
            Create server
          </button>
        </div>
      </form>
    </Modal>
  </div>
</template>

<style scoped>
.servers {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 12px 14px;
}

.head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.seg {
  display: flex;
  gap: 3px;
  padding: 3px;
  border-radius: var(--r-row);
  background: var(--panel);
}

.seg button {
  height: 28px;
  padding: 0 14px;
  border-radius: var(--r-chip);
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  transition: background var(--t-med), color var(--t-fast);
}

.seg button.on {
  background: var(--chip-active);
  color: var(--text);
  font-weight: 600;
}

.list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.row,
.row-inner {
  display: flex;
  align-items: center;
  gap: 10px;
}

.row {
  padding: 10px 12px;
  flex: none;
}

.favicon {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  overflow: hidden;
  flex: none;
  display: grid;
  place-items: center;
  background: var(--field);
  border: 1px solid var(--border);
  color: var(--text-faint);
  font-weight: 700;
  font-size: calc(20px * var(--text-scale, 1));
}

.favicon img {
  width: 100%;
  height: 100%;
  image-rendering: pixelated;
}

.info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.line {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.name {
  font-size: calc(14px * var(--text-scale, 1));
  font-weight: 600;
  flex: none;
  max-width: 55%;
}

.addr {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
  font-style: italic;
  min-width: 0;
}

.motd {
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.motd.multi {
  align-items: flex-start;
}

.motd-text {
  white-space: pre-line;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.35;
}

.blank .hint {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
}

.off {
  color: var(--text-faint);
}

.stats {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
  flex: none;
  max-width: 130px;
}

.pill-stat {
  font-size: calc(10px * var(--text-scale, 1));
  padding: 2px 7px;
  border-radius: 10px;
  background: var(--accent-bg);
  color: var(--accent);
}

.pill-stat.slow {
  background: var(--panel-3);
  color: var(--warn);
}

.ver {
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-faint);
  max-width: 130px;
}

.pick {
  width: 170px;
  flex: none;
}

.play {
  min-width: 80px;
}

.blank {
  margin: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  text-align: center;
  color: var(--text-dim);
  font-size: calc(12px * var(--text-scale, 1));
  max-width: 360px;
  padding: 30px 0;
}

.blank b {
  font-size: calc(15px * var(--text-scale, 1));
  color: var(--text);
}

.warning {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 11px 13px;
  border-radius: var(--r-row);
  border: 1px solid color-mix(in srgb, var(--warn) 40%, transparent);
  background: color-mix(in srgb, var(--warn) 9%, var(--panel));
  font-size: calc(12px * var(--text-scale, 1));
  line-height: 1.45;
  color: var(--text-dim);
  flex: none;
}

.warning > b {
  color: var(--warn);
  font-size: calc(13px * var(--text-scale, 1));
}

.warning.small {
  margin-bottom: 6px;
}

.host {
  padding: 10px 12px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.state-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--text-ghost);
  flex: none;
}

.host.running .state-dot {
  background: var(--running);
  box-shadow: 0 0 8px var(--accent-glow);
}

.host.preparing .state-dot,
.host.starting .state-dot,
.host.stopping .state-dot {
  background: var(--warn);
}

.btn.on {
  background: var(--chip-active);
}

.console {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.console-log {
  height: 220px;
  padding: 8px 10px;
  border-radius: var(--r-row);
  background: var(--panel-4);
  font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  font-size: calc(11px * var(--text-scale, 1));
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  scroll-behavior: auto;
}

.console-input {
  display: flex;
  gap: 6px;
}

.console-input .field {
  flex: 1;
}

.muted {
  color: var(--text-faint);
  font-style: italic;
}

.form {
  padding: 4px 20px 18px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form input[type="range"] {
  width: 100%;
}

.two {
  display: grid;
  grid-template-columns: 1fr 120px;
  gap: 10px;
}

.two .label {
  display: block;
  margin-bottom: 6px;
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: calc(12px * var(--text-scale, 1));
  color: var(--text-dim);
  margin-top: 4px;
}

.check a {
  color: var(--link);
}

.note {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
}

.err {
  color: var(--danger-text);
  font-size: calc(12px * var(--text-scale, 1));
}

.footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 10px;
}
</style>
