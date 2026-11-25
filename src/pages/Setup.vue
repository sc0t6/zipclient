<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import AccountHead from "../components/AccountHead.vue";
import Toggle from "../components/Toggle.vue";
import { icons } from "../icons";
import { api, loaderNames, rangeFill, type JavaInstall } from "../api";
import { importFile, importing } from "../importer";
import legal from "../../legal/legal.json";
import { useApp } from "../stores/app";
import { useSettings } from "../stores/settings";
import { themes } from "../themes";

const app = useApp();
const store = useSettings();
const s = computed(() => store.s!);
const step = computed({
  get: () => s.value.setup.step,
  set: (v: number) => (s.value.setup.step = Math.min(5, Math.max(1, v))),
});
const java = ref<JavaInstall[] | null>(null);
const accents = ["#7fcf8b", "#6fa8ff", "#c08cff", "#ff8fb1", "#ffb86b", "#5fd3c6"];
const titles = ["Welcome to Zip!", "Connect your Minecraft account", "Create a instance or import one", "Customize your Zip experience", "Your good to go!"];

const checks = computed(() => [
  { label: "Data folders prepared", ok: !!app.info, detail: app.info?.dataDir ?? "" },
  { label: "Settings validated", ok: !!store.s, detail: `schema v${s.value.schema}` },
  { label: "Mojang version service reachable", ok: !!app.versions, detail: app.versions ? `latest ${app.versions.latestRelease}` : "offline — retried when needed" },
  {
    label: "Java",
    ok: true,
    detail: java.value === null ? "checking.." : java.value.length ? `${java.value.length} installation(s) found; correct runtimes download automatically` : "none found; correct runtimes download automatically",
  },
]);

async function pickGameDir() {
  const path = await open({ directory: true, multiple: false });
  if (typeof path === "string") s.value.minecraft.game_dir = path;
}

async function finish() {
  s.value.setup.completed = true;
  s.value.setup.step = 5;
  await store.flush();
  app.go("home");
}

onMounted(async () => {
  if (!app.versions) app.loadVersions();
  java.value = await api.javaDetect().catch(() => []);
});
</script>

<template>
  <div class="setup">
    <div class="card panel">
      <div class="steps">
        <template v-for="n in 5" :key="n">
          <button class="dot" :class="{ done: n < step, on: n === step }" :disabled="n > step" :title="titles[n - 1]" @click="step = n">{{ n }}</button>
          <span v-if="n < 5" class="bar" :class="{ done: n < step }" />
        </template>
      </div>
      <Transition name="page" mode="out-in">
        <div :key="step" class="pane" :class="{ centered: step === 5 }">
          <template v-if="step === 1">
            <img :src="icons.logo" class="logo icon" alt="Zip" />
            <h1>Welcome to Zip!</h1>
            <p class="lead">Zip is a lightweight Minecraft launcher that runs every Minecraft version — releases, snapshots, betas and alphas — with Vanilla, Fabric, Forge, NeoForge or Quilt.</p>
            <div class="checks">
              <div v-for="c in checks" :key="c.label" class="check">
                <span class="mark" :class="{ ok: c.ok }">{{ c.ok ? "✓" : "!" }}</span>
                <span>{{ c.label }}</span>
                <span class="detail ellipsis">{{ c.detail }}</span>
              </div>
            </div>
            <label class="agree">
              <input type="checkbox" :checked="s.setup.legal === legal.version" @change="s.setup.legal = ($event.target as HTMLInputElement).checked ? legal.version : ''" />
              <span>
                I have read and agree to the
                <button class="inline" @click.prevent="app.legalDoc = 'terms'">Terms of Use</button> and the
                <button class="inline" @click.prevent="app.legalDoc = 'privacy'">Privacy Policy</button>, and I have read the
                <button class="inline" @click.prevent="app.legalDoc = 'disclaimer'">Legal Disclaimer</button>.
              </span>
            </label>
          </template>

          <template v-else-if="step === 2">
            <h1>Connect your Minecraft account</h1>
            <p class="lead">Sign in with Microsoft to play on servers. An offline profile works for singleplayer and offline-mode servers.</p>
            <div class="accounts">
              <div v-for="a in app.accounts" :key="a.id" class="acc">
                <AccountHead :url="a.skinUrl" :variant="a.skinVariant" :size="28" />
                <b class="grow ellipsis">{{ a.name }}</b>
                <span class="tag">{{ a.kind === "microsoft" ? "Microsoft" : "Offline" }}</span>
              </div>
              <div v-if="!app.accounts.length" class="none">No account connected yet.</div>
            </div>
            <div class="row center">
              <button class="btn primary big" @click="app.loginOpen = true">Sign in with Microsoft</button>
              <button class="btn big" @click="app.loginOpen = true">Add offline profile</button>
            </div>
          </template>

          <template v-else-if="step === 3">
            <h1>Create a instance or import one</h1>
            <p class="lead">An instance is an isolated Minecraft installation with its own version, mods and settings.</p>
            <div class="choices">
              <button class="choice" :disabled="importing" @click="app.createOpen = true"><b>Create new instance</b><span>Pick a version and loader</span></button>
              <button class="choice" :disabled="importing" @click="importFile('curseforge')"><b>Import CurseForge ZIP</b><span>.zip modpack</span></button>
              <button class="choice" :disabled="importing" @click="importFile('prism')"><b>Import Prism Launcher ZIP</b><span>.zip instance</span></button>
              <button class="choice" :disabled="importing" @click="importFile('modrinth')"><b>Import Modrinth MRPACK</b><span>.mrpack modpack</span></button>
            </div>
            <div v-if="importing" class="importing"><span class="spinner" /> {{ app.progress["import"]?.phase ?? "Importing.." }}</div>
            <div v-if="app.instances.length" class="made">
              <span class="label">Your instances</span>
              <div v-for="i in app.instances.slice(0, 4)" :key="i.id" class="inst">{{ i.name }} <span class="muted">{{ loaderNames[i.loader] }} {{ i.minecraft }}</span></div>
            </div>
          </template>

          <template v-else-if="step === 4">
            <h1>Customize your Zip experience</h1>
            <div class="custom">
              <div class="fields">
                <div class="f">
                  <span class="label">Appearance</span>
                  <div class="theme-row">
                    <button v-for="t in themes" :key="t.id" class="theme-dot" :class="{ on: s.appearance.theme === t.id }" :title="t.label" :style="{ background: t.bg, borderColor: t.panel }" @click="s.appearance.theme = t.id" />
                    <span class="label">{{ themes.find((t) => t.id === s.appearance.theme)?.label }}</span>
                  </div>
                </div>
                <div class="f">
                  <span class="label">Accent color</span>
                  <div class="swatches">
                    <button v-for="c in accents" :key="c" class="swatch" :class="{ on: s.appearance.accent === c }" :style="{ background: c }" @click="s.appearance.accent = c" />
                  </div>
                </div>
                <div class="f">
                  <span class="label">UI scaling · {{ Math.round(s.appearance.scale * 100) }}%</span>
                  <input v-model.number="s.appearance.scale" type="range" min="0.75" max="1.5" step="0.05" :style="rangeFill(s.appearance.scale, 0.75, 1.5)" />
                </div>
                <div class="f">
                  <span class="label">Animation intensity</span>
                  <div class="seg">
                    <button v-for="m in ['full', 'reduced', 'off']" :key="m" :class="{ on: s.appearance.animations === m }" @click="s.appearance.animations = m">{{ m[0].toUpperCase() + m.slice(1) }}</button>
                  </div>
                </div>
                <div class="f">
                  <span class="label">Minecraft directory</span>
                  <div class="row">
                    <input class="field grow" :value="s.minecraft.game_dir || 'Default'" readonly />
                    <button class="btn" @click="pickGameDir">Browse</button>
                  </div>
                </div>
                <div class="f">
                  <span class="label">Java</span>
                  <div class="seg">
                    <button :class="{ on: s.minecraft.java_mode === 'managed' }" @click="s.minecraft.java_mode = 'managed'">Automatic</button>
                    <button :class="{ on: s.minecraft.java_mode === 'system' }" @click="s.minecraft.java_mode = 'system'">Prefer installed</button>
                  </div>
                </div>
              </div>
              <div class="preview">
                <span class="label">Live preview</span>
                <div class="mock">
                  <div class="mock-bar"><span /><span class="on" /><span /><span /></div>
                  <div class="mock-hero">
                    <b>Latest Client Version</b>
                    <div class="mock-play">Play</div>
                  </div>
                  <div class="row mock-row"><Toggle :model-value="true" label="Preview toggle" /><span class="label">Toggle</span></div>
                </div>
              </div>
            </div>
          </template>

          <template v-else>
            <img :src="icons.logo" class="logo icon" alt="Zip" />
            <h1>Your good to go!</h1>
            <p class="lead">Press continue to get into the home menu.</p>
          </template>
        </div>
      </Transition>
      <div class="nav">
        <button class="btn" :disabled="step === 1" @click="step--">Back</button>
        <span class="count">Step {{ step }} of 5</span>
        <button v-if="step < 5" class="btn primary" :disabled="importing || (step === 1 && s.setup.legal !== legal.version)" @click="step++">
          {{ (step === 2 && !app.accounts.length) || (step === 3 && !app.instances.length) ? "Skip for now" : "Continue" }}
        </button>
        <button v-else class="btn primary" @click="finish">Continue</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setup {
  position: fixed;
  inset: 53px 0 0;
  z-index: 90;
  background: var(--bg);
  display: grid;
  place-items: center;
  padding: 14px;
}

.card {
  width: min(680px, 100%);
  height: min(520px, 100%);
  border-radius: 13px;
  padding: 18px 22px 16px;
  display: flex;
  flex-direction: column;
}

.steps {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}

.dot {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  font-size: 11px;
  font-weight: 600;
  background: var(--panel-3);
  color: var(--text-faint);
  transition: background var(--t-med), color var(--t-med), transform var(--t-med);
}

.dot.on {
  background: var(--accent-bg-hover);
  color: var(--text);
  box-shadow: 0 0 0 1px var(--accent);
  transform: scale(1.08);
}

.dot.done {
  background: color-mix(in srgb, var(--accent) 35%, var(--panel));
  color: var(--text);
}

.bar {
  width: 46px;
  height: 2px;
  border-radius: 2px;
  background: var(--panel-3);
  transition: background var(--t-slow);
}

.bar.done {
  background: color-mix(in srgb, var(--accent) 60%, var(--panel));
}

.pane {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 16px 4px 8px;
}

.pane.centered {
  justify-content: center;
  padding-bottom: 60px;
}

.logo {
  width: 72px;
  height: 72px;
  margin-top: 10px;
}

h1 {
  font-size: 24px;
  font-weight: 600;
  margin: 8px 0 6px;
}

.lead {
  color: var(--text-dim);
  max-width: 500px;
  line-height: 1.5;
  margin: 0 0 14px;
}

.agree {
  display: flex;
  gap: 8px;
  align-items: flex-start;
  max-width: 500px;
  margin-top: 14px;
  text-align: left;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-dim);
}

.agree input {
  accent-color: var(--accent);
  margin-top: 3px;
}

.inline {
  color: var(--link);
  text-decoration: underline;
  font-size: 12px;
}

.checks {
  width: 100%;
  max-width: 500px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: left;
}

.check {
  display: grid;
  grid-template-columns: 20px auto 1fr;
  gap: 8px;
  align-items: center;
  padding: 7px 10px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  font-size: 12px;
}

.mark {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  font-size: 10px;
  background: var(--danger-bg);
}

.mark.ok {
  background: color-mix(in srgb, var(--accent) 30%, var(--panel));
}

.detail {
  text-align: right;
  color: var(--text-faint);
  font-size: 11px;
}

.accounts {
  width: 100%;
  max-width: 420px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 14px;
}

.acc {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--r-row);
  background: var(--panel-2);
  text-align: left;
}

.tag {
  font-size: 10px;
  color: var(--text-faint);
}

.none {
  color: var(--text-ghost);
  font-style: italic;
  font-size: 12px;
}

.center {
  justify-content: center;
}

.btn.big {
  height: 38px;
  padding: 0 18px;
  font-size: 13px;
}

.choices {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  width: 100%;
  max-width: 500px;
}

.choice {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 14px;
  border-radius: var(--r-panel);
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  transition: background var(--t-fast), transform var(--t-fast) var(--ease), border-color var(--t-fast);
}

.choice:hover:not(:disabled) {
  background: var(--accent-bg-hover);
  border-color: var(--accent-border);
  transform: translateY(-1px);
}

.choice:active:not(:disabled) {
  transform: scale(0.98);
}

.choice span {
  font-size: 11px;
  color: var(--text-dim);
}

.importing {
  margin-top: 10px;
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 12px;
  color: var(--text-dim);
}

.made {
  margin-top: 14px;
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 12px;
}

.inst .muted {
  font-size: 11px;
}

.custom {
  display: grid;
  grid-template-columns: 1.3fr 1fr;
  gap: 16px;
  width: 100%;
  text-align: left;
}

.fields {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.f {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.f input[type="range"] {
  width: 100%;
}

.seg {
  display: flex;
  gap: 3px;
  padding: 3px;
  background: var(--panel-3);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
}

.seg button {
  flex: 1;
  padding: 5px 10px;
  border-radius: var(--r-chip);
  font-size: 12px;
  color: var(--text-dim);
  transition: background var(--t-med), color var(--t-fast);
}

.seg button.on {
  background: var(--accent-bg-hover);
  color: var(--accent);
  font-weight: 600;
  box-shadow: inset 0 0 0 1px var(--accent-border);
}

.theme-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.theme-dot {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  border: 3px solid;
  outline: 2px solid transparent;
  transition: outline-color var(--t-fast), transform var(--t-fast);
}

.theme-dot.on {
  outline-color: var(--accent);
}

.theme-dot:hover {
  transform: scale(1.08);
}

.swatches {
  display: flex;
  gap: 6px;
}

.swatch {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 2px solid transparent;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
  transition: transform var(--t-fast) var(--ease-spring), border-color var(--t-fast), box-shadow var(--t-fast);
}

.swatch:hover {
  transform: scale(1.12);
}

.swatch.on {
  border-color: var(--text);
  box-shadow: 0 0 0 2px var(--panel), 0 0 10px var(--accent-glow);
}

.preview {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.mock {
  background: var(--bg);
  border-radius: var(--r-panel);
  padding: 8px;
  border: 1px solid var(--border-soft);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.mock-bar {
  display: flex;
  gap: 4px;
  padding: 5px;
  background: var(--panel);
  border-radius: 10px;
  width: fit-content;
}

.mock-bar span {
  width: 18px;
  height: 18px;
  border-radius: 5px;
  background: var(--chip);
}

.mock-bar span.on {
  width: 42px;
  background: var(--chip-active);
}

.mock-hero {
  background: var(--panel);
  border-radius: 9px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  font-size: 13px;
}

.mock-play {
  width: 90px;
  height: 32px;
  border-radius: 8px;
  background: var(--accent-grad);
  color: var(--on-accent);
  box-shadow: 0 2px 10px var(--accent-glow);
  display: grid;
  place-items: center;
  font-weight: 700;
}

.mock-row {
  padding: 0 4px;
}

.nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid var(--border-soft);
  padding-top: 12px;
}

.count {
  font-size: 11px;
  color: var(--text-faint);
}
</style>
