<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import Dropdown, { type Option } from "./Dropdown.vue";
import { api, errorText, loaderNames, type Loader, type LoaderVersion } from "../api";
import { useApp } from "../stores/app";

const props = defineProps<{ minecraft: string; loader: Loader; loaderVersion: string }>();
const emit = defineEmits<{ "update:minecraft": [v: string]; "update:loader": [v: Loader]; "update:loaderVersion": [v: string] }>();

const app = useApp();
const kinds = ref<Record<string, boolean>>({ release: true, snapshot: false, old_beta: false, old_alpha: false });
const loaderList = ref<LoaderVersion[]>([]);
const supported = ref<Record<string, Set<string>>>({});
const loadingLoader = ref(false);
const loaderError = ref("");
const loaders: Loader[] = ["vanilla", "fabric", "forge", "neoforge", "quilt", "horizon"];

const versionOptions = computed<Option[]>(() => {
  const list = app.versions?.versions ?? [];
  const allowed = props.loader !== "vanilla" ? supported.value[props.loader] : null;
  return list
    .filter((v) => kinds.value[v.type] || v.id === props.minecraft)
    .map((v) => ({
      value: v.id,
      label: v.id,
      hint: v.type === "release" ? v.releaseTime.slice(0, 10) : v.type.replace("old_", ""),
      disabled: allowed ? !allowed.has(v.id) : false,
    }));
});

const loaderOptions = computed<Option[]>(() =>
  loaderList.value.map((v) => ({ value: v.version, label: v.version, hint: v.recommended ? "recommended" : v.stable ? "" : "beta" })),
);

async function loadSupported(loader: Loader, adjust = false) {
  if (loader === "vanilla") return;
  if (!supported.value[loader]) {
    try {
      supported.value = { ...supported.value, [loader]: new Set(await api.loaderSupported(loader)) };
    } catch {
      return;
    }
  }
  const allowed = supported.value[loader];
  if (adjust && allowed && !allowed.has(props.minecraft)) {
    const first = (app.versions?.versions ?? []).find((v) => allowed.has(v.id));
    if (first) emit("update:minecraft", first.id);
  }
}

async function loadLoaderVersions() {
  loaderError.value = "";
  loaderList.value = [];
  if (props.loader === "vanilla" || !props.minecraft) return;
  loadingLoader.value = true;
  const request = `${props.loader}|${props.minecraft}`;
  try {
    const list = await api.loaderVersions(props.loader, props.minecraft);
    if (request !== `${props.loader}|${props.minecraft}`) return;
    loaderList.value = list;
    if (!list.some((v) => v.version === props.loaderVersion)) {
      const pick = list.find((v) => v.recommended) ?? list[0];
      emit("update:loaderVersion", pick?.version ?? "");
    }
    if (!list.length) loaderError.value = `${loaderNames[props.loader]} is not available for Minecraft ${props.minecraft}.`;
  } catch (e) {
    loaderError.value = errorText(e);
  } finally {
    loadingLoader.value = false;
  }
}

watch(() => [props.loader, props.minecraft], loadLoaderVersions);
watch(() => props.loader, (l) => loadSupported(l, true));

onMounted(() => {
  if (!app.versions) app.loadVersions();
  loadSupported(props.loader);
  loadLoaderVersions();
});
</script>

<template>
  <div class="picker-grid">
    <label class="label">Minecraft version</label>
    <div class="kinds">
      <button v-for="(label, key) in { release: 'Releases', snapshot: 'Snapshots', old_beta: 'Beta', old_alpha: 'Alpha' }" :key="key" class="kind" :class="{ on: kinds[key] }" @click="kinds[key] = !kinds[key]">
        {{ label }}
      </button>
    </div>
    <Dropdown :model-value="minecraft" :options="versionOptions" searchable placeholder="Choose a version" @update:model-value="emit('update:minecraft', $event)" />
    <label class="label">Mod loader</label>
    <div class="segments">
      <button v-for="l in loaders" :key="l" class="seg" :class="{ on: loader === l, rust: l === 'horizon' }" @click="emit('update:loader', l)">{{ l === "horizon" ? "Horizon (Rust)" : loaderNames[l] }}</button>
    </div>
    <p v-if="loader === 'horizon'" class="note">Horizon is Minecraft {{ minecraft }} rewritten in Rust (unofficial, open source). It runs natively without Java and uses your Minecraft files. Java mods do not run on it; Horizon mods do.</p>
    <template v-if="loader !== 'vanilla'">
      <label class="label">{{ loaderNames[loader] }} version</label>
      <div class="row">
        <Dropdown
          class="grow"
          :model-value="loaderVersion"
          :options="loaderOptions"
          searchable
          :disabled="loadingLoader || !loaderList.length"
          :placeholder="loadingLoader ? 'Loading..' : 'Not available'"
          @update:model-value="emit('update:loaderVersion', $event)"
        />
        <span v-if="loadingLoader" class="spinner" />
      </div>
      <div v-if="loaderError" class="err">{{ loaderError }}</div>
    </template>
  </div>
</template>

<style scoped>
.picker-grid {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.label {
  margin-top: 6px;
}

.kinds,
.segments {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.kind {
  font-size: 11px;
  padding: 3px 9px;
  border-radius: 12px;
  border: 1px solid var(--border);
  color: var(--text-dim);
  transition: background var(--t-fast), color var(--t-fast), border-color var(--t-fast);
}

.kind.on {
  background: var(--accent-bg-hover);
  color: var(--text);
  border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
}

.segments {
  background: var(--panel);
  padding: 3px;
  border-radius: var(--r-row);
  flex-wrap: nowrap;
}

.seg {
  flex: 1;
  height: 26px;
  border-radius: var(--r-chip);
  font-size: 12px;
  color: var(--text-dim);
  transition: background var(--t-med) var(--ease), color var(--t-fast);
}

.seg:hover {
  background: var(--chip-hover);
}

.seg.on {
  background: var(--chip-active);
  color: var(--text);
  font-weight: 600;
}

.seg.rust.on {
  background: color-mix(in srgb, #b7410e 35%, var(--chip-active));
}

.note {
  font-size: 11px;
  color: var(--text-dim);
  margin: 2px 0 0;
  line-height: 1.4;
}

.err {
  font-size: 11px;
  color: var(--warn);
}
</style>
