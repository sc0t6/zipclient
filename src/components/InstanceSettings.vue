<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Dropdown from "./Dropdown.vue";
import Toggle from "./Toggle.vue";
import VersionPicker from "./VersionPicker.vue";
import { api, errorText, rangeFill, type Instance, type InstancePatch, type JavaInstall, type Loader, type ZipClientStatus } from "../api";
import { useApp } from "../stores/app";
import { useNotify } from "../stores/notify";
import { useSettings } from "../stores/settings";

const props = defineProps<{ instance: Instance }>();
const app = useApp();
const notify = useNotify();
const settings = useSettings();

const form = reactive({
  name: props.instance.name,
  group: props.instance.group,
  minecraft: props.instance.minecraft,
  loader: props.instance.loader as Loader,
  loaderVersion: props.instance.loader_version,
  javaPath: props.instance.java_path,
  overrideMemory: props.instance.memory_max > 0,
  memoryMin: props.instance.memory_min || settings.s?.minecraft.memory_min || 512,
  memoryMax: props.instance.memory_max || settings.s?.minecraft.memory_max || 4096,
  width: props.instance.width,
  height: props.instance.height,
  jvmArgs: props.instance.jvm_args,
  gameDir: props.instance.game_dir,
  accountId: props.instance.account_id,
  zipClient: props.instance.zip_client,
  memoryWarning: props.instance.memory_warning,
  fastLaunch: props.instance.fast_launch,
});
const java = ref<JavaInstall[]>([]);
const zip = ref<ZipClientStatus | null>(null);
const saving = ref(false);
const error = ref("");
let timer = 0;
let lastSent = "";

const running = computed(() => !!app.games[props.instance.id]);
const javaOptions = computed(() => [
  { value: "", label: "Default (launcher setting)" },
  ...java.value.map((j) => ({ value: j.path, label: `Java ${j.major} · ${j.vendor}`, hint: j.source })),
  ...(form.javaPath && !java.value.some((j) => j.path === form.javaPath) ? [{ value: form.javaPath, label: form.javaPath, hint: "custom" }] : []),
]);
const accountOptions = computed(() => [
  { value: "", label: "Active account" },
  ...app.accounts.map((a) => ({ value: a.id, label: a.name, hint: a.kind === "microsoft" ? "Microsoft" : "offline" })),
]);
const maxRam = 32768;

function patch(): InstancePatch {
  return {
    name: form.name,
    group: form.group,
    minecraft: form.minecraft,
    loader: form.loader,
    loaderVersion: form.loader === "vanilla" ? "" : form.loaderVersion,
    javaPath: form.javaPath,
    memoryMin: form.overrideMemory ? form.memoryMin : 0,
    memoryMax: form.overrideMemory ? form.memoryMax : 0,
    width: Number(form.width) || 0,
    height: Number(form.height) || 0,
    jvmArgs: form.jvmArgs,
    gameDir: form.gameDir,
    accountId: form.accountId,
    zipClient: form.zipClient,
    memoryWarning: form.memoryWarning,
    fastLaunch: form.fastLaunch,
  };
}

async function save() {
  if (!form.name.trim() || (form.loader !== "vanilla" && !form.loaderVersion)) return;
  const body = patch();
  const key = JSON.stringify(body);
  if (key === lastSent) return;
  saving.value = true;
  error.value = "";
  try {
    const updated = await api.updateInstance(props.instance.id, body);
    lastSent = key;
    app.replaceInstance(updated);
  } catch (e) {
    error.value = errorText(e);
  } finally {
    saving.value = false;
  }
}

const optimizing = ref(false);

async function applyOptimized() {
  optimizing.value = true;
  try {
    const preset = await api.optimizePreset(form.minecraft, form.loader);
    form.overrideMemory = true;
    form.memoryMax = preset.memoryMax;
    form.memoryMin = preset.memoryMin;
    form.jvmArgs = preset.jvmArgs;
    notify.success("Optimized Mode applied", `${preset.memoryMax} MB RAM and tuned JVM arguments for Java ${preset.javaMajor}.`);
  } catch (e) {
    notify.error("Could not optimize", errorText(e));
  } finally {
    optimizing.value = false;
  }
}

async function pickJava() {
  const path = await open({ multiple: false, filters: [{ name: "Java", extensions: ["exe"] }] });
  if (typeof path !== "string") return;
  try {
    const info = await api.javaCheck(path);
    notify.info("Java selected", `Java ${info.major} (${info.vendor})`);
  } catch (e) {
    notify.warn("Unrecognised Java", errorText(e));
  }
  form.javaPath = path;
}

async function pickDir() {
  const path = await open({ directory: true, multiple: false });
  if (typeof path === "string") form.gameDir = path;
}

watch(
  () => ({ ...form }),
  () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(save, 500);
  },
  { deep: true },
);

watch(
  () => [form.minecraft, form.loader],
  async () => {
    zip.value = await api.zipClientStatus(form.minecraft, form.loader).catch(() => null);
    if (!zip.value?.available) form.zipClient = false;
  },
  { immediate: true },
);

watch(
  () => [form.memoryMin, form.memoryMax],
  () => {
    if (form.memoryMin > form.memoryMax) form.memoryMin = form.memoryMax;
  },
);

onMounted(async () => {
  lastSent = JSON.stringify(patch());
  java.value = await api.javaDetect().catch(() => []);
});
</script>

<template>
  <div class="settings scroll">
    <div v-if="running" class="banner">Changes apply the next time this instance launches.</div>
    <div v-if="error" class="banner err">{{ error }}</div>
    <section>
      <h4>General</h4>
      <div class="grid2">
        <label><span class="label">Name</span><input v-model="form.name" class="field" maxlength="64" /></label>
        <label><span class="label">Group</span><input v-model="form.group" class="field" maxlength="40" placeholder="Ungrouped" /></label>
      </div>
    </section>
    <section>
      <h4>Version</h4>
      <VersionPicker v-model:minecraft="form.minecraft" v-model:loader="form.loader" v-model:loader-version="form.loaderVersion" />
      <p class="note">Changing the version or loader installs the new files on next launch. Existing mods are kept but may be incompatible.</p>
    </section>
    <template v-if="form.loader !== 'horizon'">
    <section>
      <div class="row between">
        <div>
          <h4>Optimized Mode</h4>
          <div class="note">Pick memory and JVM arguments for this version, loader and your RAM.</div>
        </div>
        <button class="btn primary" :disabled="optimizing" @click="applyOptimized"><span v-if="optimizing" class="spinner" />Apply</button>
      </div>
      <div class="row between">
        <div>
          <h4>Low memory warning</h4>
          <div class="note">Notify me when Minecraft is running out of memory.</div>
        </div>
        <Toggle v-model="form.memoryWarning" label="Low memory warning" />
      </div>
      <div class="row between">
        <div>
          <h4>Improve loading speeds</h4>
          <div class="note">Skip file checks on launch. The next launch still checks once, then checks turn off. Use Repair if files break.</div>
        </div>
        <Toggle v-model="form.fastLaunch" label="Improve loading speeds" />
      </div>
    </section>
    <section>
      <h4>Java</h4>
      <div class="row">
        <Dropdown v-model="form.javaPath" :options="javaOptions" class="grow" />
        <button class="btn" @click="pickJava">Browse</button>
      </div>
      <p class="note">By default Zip uses the Java runtime Mojang specifies for each Minecraft version.</p>
    </section>
    <section>
      <div class="row between">
        <h4>Memory</h4>
        <label class="row inline"><span class="label">Override</span><Toggle v-model="form.overrideMemory" label="Override memory" /></label>
      </div>
      <div class="sliders" :class="{ off: !form.overrideMemory }">
        <label>
          <span class="label">Maximum · {{ form.memoryMax }} MB</span>
          <input v-model.number="form.memoryMax" type="range" min="512" :max="maxRam" step="256" :disabled="!form.overrideMemory" :style="rangeFill(form.memoryMax, 512, maxRam)" />
        </label>
        <label>
          <span class="label">Minimum · {{ form.memoryMin }} MB</span>
          <input v-model.number="form.memoryMin" type="range" min="256" :max="form.memoryMax" step="256" :disabled="!form.overrideMemory" :style="rangeFill(form.memoryMin, 256, form.memoryMax)" />
        </label>
      </div>
    </section>
    <section>
      <h4>Window</h4>
      <div class="grid2">
        <label><span class="label">Width (0 = default)</span><input v-model.number="form.width" class="field" type="number" min="0" max="16384" /></label>
        <label><span class="label">Height (0 = default)</span><input v-model.number="form.height" class="field" type="number" min="0" max="16384" /></label>
      </div>
    </section>
    <section>
      <h4>JVM arguments</h4>
      <textarea v-model="form.jvmArgs" class="field area" rows="2" placeholder="-XX:+UseG1GC -Dexample=true" spellcheck="false" />
    </section>
    </template>
    <section>
      <h4>Game directory</h4>
      <div class="row">
        <input v-model="form.gameDir" class="field grow" placeholder="Instance folder (default)" />
        <button class="btn" @click="pickDir">Browse</button>
        <button v-if="form.gameDir" class="btn ghost" @click="form.gameDir = ''">Reset</button>
      </div>
    </section>
    <section>
      <h4>Account</h4>
      <Dropdown v-model="form.accountId" :options="accountOptions" />
    </section>
    <section>
      <div class="row between">
        <div>
          <h4>Use Zip Client</h4>
          <div class="note">{{ zip?.label ?? "Zip Client — Coming Soon" }}</div>
        </div>
        <Toggle v-model="form.zipClient" :disabled="!zip?.available" label="Use Zip Client" />
      </div>
    </section>
    <div class="saving" :class="{ show: saving }">Saving..</div>
  </div>
</template>

<style scoped>
.settings {
  padding: 12px 16px 24px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  position: relative;
}

section {
  background: var(--panel-2);
  border: 1px solid var(--border-soft);
  border-radius: var(--r-row);
  padding: 10px 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: none;
  box-shadow: inset 0 1px 0 var(--hairline);
}

h4 {
  margin: 0 0 2px;
  font-size: calc(12px * var(--text-scale, 1));
  font-weight: 600;
}

.grid2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

label {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

label.inline {
  flex-direction: row;
  align-items: center;
  gap: 8px;
}

.between {
  justify-content: space-between;
}

.note {
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
  margin: 2px 0 0;
}

.sliders {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
  transition: opacity var(--t-med);
}

.sliders.off {
  opacity: 0.45;
}

input[type="range"] {
  width: 100%;
}

.area {
  height: auto;
  padding: 8px 12px;
  resize: vertical;
  font-family: ui-monospace, Consolas, monospace;
  font-size: calc(11px * var(--text-scale, 1));
}

.banner {
  font-size: calc(11px * var(--text-scale, 1));
  padding: 7px 10px;
  border-radius: var(--r-small);
  background: var(--accent-bg);
  border: 1px solid var(--accent-border);
  color: var(--text-dim);
}

.banner.err {
  background: var(--danger-bg);
  border-color: color-mix(in srgb, var(--danger-text) 35%, transparent);
  color: var(--danger-text);
}

.saving {
  position: sticky;
  bottom: 0;
  align-self: flex-end;
  font-size: calc(11px * var(--text-scale, 1));
  color: var(--text-faint);
  opacity: 0;
  transition: opacity var(--t-med);
}

.saving.show {
  opacity: 1;
}
</style>
