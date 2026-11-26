<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import AccountHead from "../components/AccountHead.vue";
import Modal from "../components/Modal.vue";
import { icons } from "../icons";
import { api, errorText, formatPlaytime, loaderNames, type Account, type NewsItem } from "../api";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";
import { useSettings } from "../stores/settings";

defineOptions({ name: "HomePage" });

const app = useApp();
const notify = useNotify();
const settings = useSettings();

const now = ref(new Date());
const news = ref<NewsItem[]>([]);
const newsState = ref<"loading" | "ready" | "error">("loading");
const newsSource = ref<"zip" | "minecraft">("minecraft");
const pickerOpen = ref(false);
const stopOpen = ref(false);
let clockTimer = 0;

const instance = computed(() => app.quick);
const game = computed(() => (instance.value ? app.games[instance.value.id] : undefined));
const progress = computed(() => (instance.value ? app.progress[instance.value.id] : undefined));
const account = computed(() => app.activeAccount);

const time = computed(() => now.value.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" }));
const date = computed(() => {
  const d = now.value;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}/${pad(d.getDate())}/${d.getFullYear()}`;
});
const weekday = computed(() => now.value.toLocaleDateString("en-US", { weekday: "long" }));

const play = computed(() => {
  if (!instance.value) return { label: "Create", sub: "", busy: false, fill: 0 };
  const state = game.value?.state;
  const p = progress.value;
  if (state === "running") return { label: "Running", sub: "", busy: false, fill: 0 };
  if (state === "launching") return { label: "Launching", sub: "", busy: true, fill: 1 };
  if (state === "downloading" || state === "preparing") {
    const fraction = p?.fraction ?? 0;
    const label = p?.phase?.startsWith("Downloading") ? "Downloading" : p?.phase?.startsWith("Running") || p?.phase?.startsWith("Installing") ? "Installing" : "Preparing";
    return { label, sub: p?.phase ?? "", busy: true, fill: fraction, percent: p && p.totalFiles > 0 ? Math.round(fraction * 100) : null };
  }
  return { label: "Play", sub: "", busy: false, fill: 0 };
});

const versionLabel = computed(() => {
  const i = instance.value;
  if (!i) return "—";
  return i.loader === "vanilla" ? i.minecraft : `${loaderNames[i.loader]} ${i.minecraft}`;
});

const updateText = computed(() => {
  const u = app.update;
  if (app.updateChecking) return "Checking for updates..";
  if (!u) return `Zip ${app.info?.version ?? ""}`;
  if (u.error) return `Zip ${u.current}`;
  if (!u.configured) return `Zip ${u.current} · updates not configured`;
  if (u.available) return `Zip ${u.version} is available!`;
  return "Zip is up to date.";
});

function pickNews(source: "zip" | "minecraft") {
  if (newsSource.value === source) return;
  newsSource.value = source;
  news.value = [];
  loadNews();
}

function tick() {
  now.value = new Date();
  const msToNextMinute = 60000 - (Date.now() % 60000) + 20;
  clockTimer = window.setTimeout(tick, msToNextMinute);
}

async function loadNews() {
  newsState.value = "loading";
  const source = newsSource.value;
  try {
    const items = await api.news(source);
    if (source !== newsSource.value) return;
    news.value = items;
    newsState.value = "ready";
  } catch {
    newsState.value = "error";
  }
}

function onPlay() {
  const i = instance.value;
  if (!i) {
    app.createOpen = true;
    return;
  }
  const state = game.value?.state;
  if (state === "running") {
    stopOpen.value = true;
    return;
  }
  if (state) return;
  if (!app.accounts.length) {
    notify.info("Add an account first", "Sign in with Microsoft or add an offline profile.");
    app.loginOpen = true;
    return;
  }
  app.launch(i.id);
}

async function useAccount(a: Account) {
  if (a.needsRelogin) {
    app.loginOpen = true;
    return;
  }
  try {
    await api.setActiveAccount(a.id);
    await app.refreshAccounts();
  } catch (e) {
    notify.error("Could not switch account", errorText(e));
  }
}

function choose(id: string) {
  app.quickInstance = id;
  pickerOpen.value = false;
}

function closePicker(event: MouseEvent) {
  if (!(event.target as HTMLElement).closest(".picker, .instance-name")) pickerOpen.value = false;
}

onMounted(() => {
  tick();
  loadNews();
  window.addEventListener("mousedown", closePicker);
});

onUnmounted(() => {
  window.clearTimeout(clockTimer);
  window.removeEventListener("mousedown", closePicker);
});
</script>

<template>
  <div
    class="home"
    :class="[
      `hero-${settings.ui.home.hero}`,
      {
        swap: settings.ui.home.swap && settings.ui.home.accounts && settings.ui.home.news,
        'one-bottom': settings.ui.home.accounts !== settings.ui.home.news,
        'no-bottom': !settings.ui.home.accounts && !settings.ui.home.news,
      },
    ]"
  >
    <section class="hero panel" :class="{ plain: settings.s?.appearance.background === 'none' }">
      <div class="bg" :style="{ backgroundImage: `url(${icons.background})` }" />
      <div class="hero-left">
        <h1><span class="h1-label">Latest Client Version:</span> <span class="h1-value">{{ app.versions?.latestRelease ?? "…" }}</span></h1>
        <div class="line ellipsis"><span class="k">Recent Instance:</span> <span class="v">{{ instance?.name ?? "none yet" }}</span></div>
        <div class="line ellipsis"><span class="k">Instance Version:</span> <span class="v">{{ versionLabel }}</span></div>
        <div class="line ellipsis">
          <span class="k">Playtime:</span> <span class="v">{{ formatPlaytime(instance?.playtime ?? 0) }}</span>
          <span class="total">· total {{ formatPlaytime(app.totalPlaytime) }}</span>
        </div>
        <div class="play-area">
          <button class="instance-name ellipsis" :disabled="!app.instances.length" @click="pickerOpen = !pickerOpen">
            {{ instance?.name ?? "No instance" }}
            <svg v-if="app.instances.length > 1" viewBox="0 0 10 6"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" /></svg>
          </button>
          <Transition name="pop">
            <div v-if="pickerOpen" class="picker scroll">
              <button v-for="i in app.instances" :key="i.id" class="pick" :class="{ on: i.id === instance?.id }" @click="choose(i.id)">
                <span class="ellipsis">{{ i.name }}</span>
                <span class="pick-ver">{{ i.loader === "vanilla" ? i.minecraft : `${loaderNames[i.loader]} ${i.minecraft}` }}</span>
              </button>
            </div>
          </Transition>
          <div class="play-row">
            <button class="play" :class="{ busy: play.busy, running: play.label === 'Running' }" :title="play.sub" @click="onPlay">
              <span class="fill" :style="{ transform: `scaleX(${play.fill})` }" />
              <span class="shine" />
              <span class="play-label">
                <span v-if="play.busy" class="spinner" />
                <span class="text">{{ play.label }}</span>
                <span v-if="play.percent != null" class="pct">{{ play.percent }}%</span>
              </span>
              <span v-if="play.label === 'Running'" class="stop-hint">Stop</span>
            </button>
            <button class="shortcut" title="Instance settings" :disabled="!instance" @click="instance && app.openInstance(instance.id, 'settings')">
              <img :src="icons.instanceSettings" class="icon" alt="" />
            </button>
          </div>
          <div class="phase ellipsis">{{ play.sub }}</div>
        </div>
      </div>
      <div class="hero-right">
        <div class="tagline">Zip, your next client to try out.</div>
        <div class="identity">
          <button class="avatar" title="Skins" @click="app.go('skins')">
            <AccountHead :url="account?.skinUrl" :variant="account?.skinVariant" :data="account && account.kind === 'offline' ? app.localSkins[account.id]?.dataUrl : null" :size="92" :radius="10" />
            <span class="avatar-name ellipsis">{{ account?.name ?? "No account" }}</span>
          </button>
          <div class="clock">
            <div class="time">{{ time }}</div>
            <div class="date">
              {{ date }} <span class="sep" /> {{ weekday }}
            </div>
            <button class="update" @click="app.update?.available ? app.go('settings') : app.checkUpdate(true)">{{ updateText }}</button>
          </div>
        </div>
      </div>
    </section>

    <section v-if="settings.ui.home.accounts" class="accounts panel">
      <div class="title">Accounts</div>
      <div class="acc-list scroll">
        <div v-for="a in app.accounts" :key="a.id" class="acc" :class="{ active: a.active && !a.needsRelogin }">
          <AccountHead :url="a.skinUrl" :variant="a.skinVariant" :data="a.kind === 'offline' ? app.localSkins[a.id]?.dataUrl : null" :size="32" />
          <div class="acc-name">
            <div class="row nowrap">
              <span class="ellipsis">{{ a.name }}</span>
              <img :src="a.kind === 'microsoft' ? icons.verified : icons.cracked" class="icon badge" :title="a.kind === 'microsoft' ? 'Microsoft account' : 'Offline profile'" alt="" />
            </div>
            <div v-if="a.kind === 'offline'" class="acc-note">(can't play on normal servers.)</div>
            <div v-else-if="a.needsRelogin" class="acc-note warn">(session expired)</div>
          </div>
          <button class="acc-action" :class="{ using: a.active && !a.needsRelogin }" @click="useAccount(a)">
            {{ a.needsRelogin ? "+ Login" : a.active ? "Using." : "Use" }}
          </button>
        </div>
        <div v-if="!app.accounts.length" class="empty">No accounts yet.</div>
      </div>
      <button class="add-account" @click="app.loginOpen = true">+ Add an account</button>
    </section>

    <section v-if="settings.ui.home.news" class="news panel">
      <div class="news-side">
        <div class="news-title"><img :src="icons.news" class="icon" alt="" /> News</div>
        <div class="news-tabs">
          <button :class="{ on: newsSource === 'zip' }" @click="pickNews('zip')">Zip News</button>
          <button :class="{ on: newsSource === 'minecraft' }" @click="pickNews('minecraft')">Minecraft News</button>
        </div>
      </div>
      <div class="news-list scroll">
        <button v-for="item in news" :key="item.url" class="news-item" :title="item.text" @click="openUrl(item.url)">
          <img :src="icons.info" class="icon info" alt="" />
          <div class="grow">
            <div class="news-head ellipsis">{{ item.title }}</div>
            <div class="news-sub ellipsis">{{ item.text }}</div>
          </div>
          <span class="news-date">{{ item.date }}</span>
        </button>
        <div v-if="newsState === 'loading'" class="end"><span class="spinner" /></div>
        <div v-else-if="newsState === 'error'" class="end">
          News is unavailable right now. <button class="retry" @click="loadNews">Retry</button>
        </div>
        <div v-else-if="!news.length" class="end">{{ newsSource === "zip" ? "No Zip news yet." : "No news right now." }}</div>
        <div v-else class="end">No more news to be found.</div>
      </div>
    </section>

    <Modal v-if="stopOpen" title="Stop Minecraft?" :width="380" @close="stopOpen = false">
      <div class="stop-body">
        <p>Force stopping can lose unsaved progress. Prefer quitting from the game menu.</p>
        <div class="row end-row">
          <button class="btn" @click="stopOpen = false">Cancel</button>
          <button
            class="btn danger"
            @click="
              instance && app.kill(instance.id);
              stopOpen = false;
            "
          >
            Force stop
          </button>
        </div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>
.home {
  height: 100%;
  display: grid;
  grid-template-columns: minmax(250px, 0.9fr) minmax(320px, 1.4fr);
  grid-template-rows: minmax(225px, 1.15fr) minmax(160px, 1fr);
  gap: 12px;
  padding: 10px 12px 12px;
}

.home.hero-small {
  grid-template-rows: minmax(190px, 0.75fr) minmax(160px, 1fr);
}

.home.hero-large {
  grid-template-rows: minmax(260px, 1.8fr) minmax(150px, 1fr);
}

.home.swap {
  grid-template-columns: minmax(320px, 1.4fr) minmax(250px, 0.9fr);
}

.home.swap .accounts {
  order: 2;
}

.home.one-bottom .accounts,
.home.one-bottom .news {
  grid-column: 1 / -1;
}

.home.no-bottom {
  grid-template-rows: 1fr;
}

.hero {
  grid-column: 1 / -1;
  position: relative;
  overflow: hidden;
  display: flex;
  justify-content: space-between;
  padding: 13px 18px 16px 7px;
  min-height: 0;
}

.bg {
  position: absolute;
  inset: 0;
  background-size: cover;
  background-position: center;
  opacity: calc(1 - var(--bg-dim, 0.78));
  transition: opacity var(--t-slow);
}

.hero::after {
  content: "";
  position: absolute;
  inset: 0;
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--panel) 78%, transparent), transparent 62%),
    linear-gradient(0deg, color-mix(in srgb, var(--panel) 60%, transparent), transparent 48%),
    radial-gradient(120% 90% at 88% 0%, color-mix(in srgb, var(--accent) 8%, transparent), transparent 55%);
  pointer-events: none;
}

.hero.plain .bg {
  display: none;
}

.hero-left,
.hero-right {
  position: relative;
  z-index: 1;
  min-width: 0;
}

.hero-left {
  display: flex;
  flex-direction: column;
  flex: 1;
}

h1 {
  font-size: calc(24px * var(--text-scale, 1));
  font-weight: 600;
  margin: 0 0 4px;
  white-space: nowrap;
  letter-spacing: -0.01em;
  text-shadow: 0 1px 8px rgba(0, 0, 0, 0.5);
}

.h1-label {
  color: var(--text-dim);
  font-weight: 500;
}

.h1-value {
  color: var(--accent);
  font-weight: 700;
}

.line {
  font-size: calc(19px * var(--text-scale, 1));
  line-height: 25px;
  text-shadow: 0 1px 6px rgba(0, 0, 0, 0.45);
}

.line .k {
  color: var(--text-dim);
  font-weight: 500;
}

.line .v {
  color: var(--text);
  font-weight: 600;
}

.total {
  font-size: calc(13px * var(--text-scale, 1));
  color: var(--text-faint);
}

.play-area {
  margin-top: auto;
  padding-left: 11px;
  position: relative;
  width: 210px;
}

.instance-name {
  display: flex;
  align-items: center;
  gap: 5px;
  width: 142px;
  justify-content: center;
  font-size: calc(12px * var(--text-scale, 1));
  font-style: italic;
  margin-bottom: 5px;
  padding: 3px 6px;
  border-radius: var(--r-chip);
  color: var(--text-dim);
  border: 1px solid transparent;
  transition: background var(--t-fast), color var(--t-fast), border-color var(--t-fast);
}

.instance-name:hover:not(:disabled) {
  background: color-mix(in srgb, var(--panel) 75%, transparent);
  color: var(--text);
  border-color: var(--panel-border);
}

.instance-name svg {
  width: 8px;
  height: 5px;
  flex: none;
}

.picker {
  position: absolute;
  bottom: 78px;
  left: 11px;
  width: 240px;
  max-height: 180px;
  background: var(--panel);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  padding: 4px;
  z-index: 5;
  box-shadow: var(--shadow-pop), inset 0 1px 0 var(--hairline);
}

.pick {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: var(--r-small);
  font-size: calc(12px * var(--text-scale, 1));
  text-align: left;
  transition: background var(--t-fast);
}

.pick:hover {
  background: var(--chip-hover);
}

.pick.on {
  background: var(--accent-bg-hover);
  color: var(--accent);
  font-weight: 600;
}

.pick-ver {
  margin-left: auto;
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-faint);
  flex: none;
}

.play-row {
  display: flex;
  align-items: flex-end;
  gap: 6px;
}

.play {
  position: relative;
  width: 142px;
  height: 49px;
  border-radius: var(--r-panel);
  background: var(--accent-grad);
  color: var(--on-accent);
  border: 1px solid color-mix(in srgb, var(--accent) 70%, #000);
  overflow: hidden;
  font-size: calc(20px * var(--text-scale, 1));
  font-weight: 700;
  letter-spacing: 0.01em;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.3), 0 4px 18px var(--accent-glow);
  transition: background var(--t-fast), transform var(--t-fast) var(--ease), box-shadow var(--t-fast) var(--ease), filter var(--t-fast), color var(--t-fast), border-color var(--t-fast);
}

.play:hover {
  filter: brightness(1.06);
  transform: translateY(-1px);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.32), 0 6px 24px var(--accent-glow);
}

.play:active {
  transform: scale(0.97);
}

.play .fill {
  position: absolute;
  inset: 0;
  background: rgba(255, 255, 255, 0.26);
  transform-origin: left;
  transition: transform var(--t-slow) var(--ease);
}

.play .shine {
  position: absolute;
  inset: 0;
  background: linear-gradient(105deg, transparent 42%, rgba(255, 255, 255, 0.38) 50%, transparent 58%);
  transform: translateX(-130%);
  pointer-events: none;
}

.play:hover .shine {
  transform: translateX(130%);
  transition: transform calc(620ms * var(--motion)) var(--ease);
}

.play-label {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  transition: opacity var(--t-fast), transform var(--t-fast);
}

.play.busy .text {
  font-size: calc(15px * var(--text-scale, 1));
}

.play .pct {
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
  color: inherit;
  opacity: 0.72;
}

.play .spinner {
  border-color: color-mix(in srgb, var(--on-accent) 30%, transparent);
  border-top-color: var(--on-accent);
}

.stop-hint {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  opacity: 0;
  transform: translateY(6px);
  transition: opacity var(--t-fast), transform var(--t-fast);
}

.play.running {
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.3), 0 0 22px var(--accent-glow);
}

.play.running:hover {
  background: linear-gradient(180deg, color-mix(in srgb, var(--danger) 88%, #ffffff), var(--danger-text));
  border-color: color-mix(in srgb, var(--danger-text) 70%, #000);
  color: #ffffff;
  filter: none;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.25), 0 4px 18px var(--danger-glow);
}

.play.running:hover .play-label {
  opacity: 0;
  transform: translateY(-6px);
}

.play.running:hover .stop-hint {
  opacity: 1;
  transform: none;
}

.shortcut {
  width: 30px;
  height: 30px;
  border-radius: var(--r-chip);
  display: grid;
  place-items: center;
  background: var(--chip-active);
  border: 1px solid var(--panel-border);
  transition: background var(--t-fast), transform var(--t-fast), border-color var(--t-fast);
}

.shortcut:hover:not(:disabled) {
  background: var(--chip-hover);
  border-color: var(--border-strong);
}

.shortcut:active:not(:disabled) {
  transform: scale(0.93);
}

.shortcut .icon {
  width: 18px;
  height: 18px;
}

.phase {
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-dim);
  height: 13px;
  margin-top: 4px;
  width: 200px;
}

.hero-right {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  flex: none;
}

.tagline {
  font-size: calc(20px * var(--text-scale, 1));
  font-style: italic;
  margin-top: 20px;
  color: var(--text-dim);
  text-shadow: 0 1px 6px rgba(0, 0, 0, 0.4);
}

.identity {
  display: flex;
  gap: 14px;
  margin-top: 8px;
}

.avatar {
  width: 92px;
  border-radius: 10px;
  overflow: hidden;
  background: var(--panel);
  border: 1px solid var(--panel-border);
  display: flex;
  flex-direction: column;
  box-shadow: var(--shadow);
  transition: transform var(--t-fast) var(--ease), box-shadow var(--t-fast) var(--ease), border-color var(--t-fast);
}

.avatar:hover {
  transform: translateY(-2px);
  border-color: var(--accent-border);
  box-shadow: var(--shadow), 0 0 16px color-mix(in srgb, var(--accent) 20%, transparent);
}

.avatar :deep(.head) {
  border-radius: 10px 10px 0 0 !important;
  padding: 20px;
  background: var(--panel);
  height: 92px !important;
}

.avatar-name {
  height: 26px;
  line-height: 26px;
  background: var(--panel-3);
  font-weight: 700;
  font-size: calc(14px * var(--text-scale, 1));
  padding: 0 6px;
  border-radius: 0 0 8px 8px;
}

.clock {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.time {
  font-family: var(--font-clock);
  font-size: calc(52px * var(--text-scale, 1));
  line-height: 52px;
  margin-top: -2px;
  font-style: italic;
  text-shadow: 0 2px 14px rgba(0, 0, 0, 0.5);
}

.date {
  font-size: calc(19px * var(--text-scale, 1));
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 4px;
  font-variant-numeric: tabular-nums;
  color: var(--text-dim);
}

.sep {
  width: 1.5px;
  height: 22px;
  background: var(--text-faint);
}

.update {
  font-size: calc(12px * var(--text-scale, 1));
  font-style: italic;
  margin-top: 12px;
  padding: 3px 9px;
  border-radius: 20px;
  color: var(--text-dim);
  border: 1px solid transparent;
  transition: background var(--t-fast), color var(--t-fast), border-color var(--t-fast);
}

.update:hover {
  background: color-mix(in srgb, var(--panel) 75%, transparent);
  color: var(--text);
  border-color: var(--panel-border);
}

.accounts {
  border-radius: var(--r-row);
  padding: 6px 8px 8px;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.title {
  text-align: center;
  font-size: calc(11px * var(--text-scale, 1));
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.09em;
  color: var(--text-faint);
  margin-bottom: 6px;
}

.acc-list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.acc {
  display: flex;
  align-items: center;
  gap: 7px;
  min-height: 40px;
  padding: 4px 8px 4px 5px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  border: 1px solid var(--panel-border);
  flex: none;
  transition: border-color var(--t-fast), background var(--t-fast);
}

.acc.active {
  border-color: var(--accent-border);
  background: var(--accent-bg);
}

.acc-name {
  flex: 1;
  min-width: 0;
  font-size: calc(13px * var(--text-scale, 1));
}

.nowrap {
  gap: 4px;
  min-width: 0;
}

.badge {
  width: 10px;
  height: 10px;
}

.acc-note {
  font-size: calc(8px * var(--text-scale, 1));
  color: var(--text-faint);
  margin-top: 1px;
}

.acc-note.warn {
  color: var(--warn);
}

.acc-action {
  width: 81px;
  height: 22px;
  border-radius: var(--r-chip);
  background: var(--field);
  border: 1px solid var(--border-soft);
  font-size: calc(12px * var(--text-scale, 1));
  font-style: italic;
  color: var(--text);
  transition: background var(--t-fast), color var(--t-fast), border-color var(--t-fast);
  flex: none;
}

.acc-action.using {
  color: var(--accent);
  border-color: var(--accent-border);
  background: var(--accent-bg);
  cursor: default;
  font-weight: 600;
}

.acc-action:not(.using):hover {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
}

.add-account {
  margin-top: 8px;
  height: 32px;
  border-radius: var(--r-chip);
  background: transparent;
  border: 1px dashed var(--border-strong);
  font-size: calc(12px * var(--text-scale, 1));
  font-style: italic;
  color: var(--text-dim);
  flex: none;
  transition: background var(--t-fast), color var(--t-fast), border-color var(--t-fast);
}

.add-account:hover {
  background: var(--accent-bg);
  border-color: var(--accent);
  color: var(--accent);
}

.news {
  border-radius: var(--r-row);
  display: flex;
  overflow: hidden;
  min-height: 0;
}

.news-side {
  width: 128px;
  flex: none;
  background: var(--panel-4);
  padding: 8px 8px 8px 12px;
  border-right: 1px solid var(--border-soft);
}

.news-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: calc(19px * var(--text-scale, 1));
  font-weight: 600;
}

.news-tabs {
  display: flex;
  flex-direction: column;
  gap: 5px;
  margin-top: 10px;
}

.news-tabs button {
  text-align: left;
  font-size: calc(12px * var(--text-scale, 1));
  padding: 6px 9px;
  border-radius: var(--r-chip);
  color: var(--text-dim);
  border-left: 2px solid transparent;
  transition: background var(--t-fast), color var(--t-fast), border-color var(--t-fast);
}

.news-tabs button:hover {
  background: var(--chip-hover);
  color: var(--text);
}

.news-tabs button.on {
  background: var(--accent-bg-hover);
  color: var(--accent);
  font-weight: 600;
  border-left-color: var(--accent);
}

.news-list {
  flex: 1;
  min-width: 0;
  padding: 9px 9px 6px 7px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.news-item {
  display: flex;
  align-items: center;
  gap: 9px;
  min-height: 44px;
  padding: 0 11px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  border: 1px solid var(--panel-border);
  text-align: left;
  flex: none;
  transition: background var(--t-fast), border-color var(--t-fast), transform var(--t-fast) var(--ease);
}

.news-item:hover {
  background: var(--chip-hover);
  border-color: var(--accent-border);
  transform: translateX(2px);
}

.info {
  width: 20px;
  height: 20px;
}

.news-head {
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
}

.news-sub {
  font-size: calc(10px * var(--text-scale, 1));
  font-style: italic;
  color: var(--text-dim);
}

.news-date {
  font-size: calc(9px * var(--text-scale, 1));
  color: var(--text-faint);
  flex: none;
  font-variant-numeric: tabular-nums;
}

.end {
  text-align: center;
  font-size: calc(10px * var(--text-scale, 1));
  font-style: italic;
  color: var(--text-ghost);
  padding: 4px;
  display: flex;
  justify-content: center;
  gap: 6px;
}

.retry {
  text-decoration: underline;
  font-size: calc(10px * var(--text-scale, 1));
  color: var(--text-dim);
}

.stop-body {
  padding: 14px 20px 18px;
}

.stop-body p {
  color: var(--text-dim);
  margin: 0 0 14px;
  line-height: 1.5;
}

.end-row {
  justify-content: flex-end;
}
</style>
