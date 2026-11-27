<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Dropdown from "./Dropdown.vue";
import Modal from "./Modal.vue";
import { api, errorText, type Datapack, type Instance, type World } from "../api";
import { useNotify } from "../stores/notify";

const props = defineProps<{ instance: Instance }>();
const world = defineModel<string>("world", { default: "" });
const notify = useNotify();
const worlds = ref<World[]>([]);
const packs = ref<Datapack[]>([]);
const loaded = ref(false);
const removing = ref<Datapack | null>(null);

const options = computed(() => worlds.value.map((w) => ({ value: w.folder, label: w.name })));

async function loadPacks() {
  if (!world.value) return;
  try {
    packs.value = await api.datapacks(props.instance.id, world.value);
  } catch (e) {
    notify.error("Could not read datapacks", errorText(e));
  }
}

async function importPacks() {
  const paths = await open({ multiple: true, filters: [{ name: "Datapack", extensions: ["zip"] }] });
  if (!paths) return;
  try {
    const added = await api.importDatapacks(props.instance.id, world.value, Array.isArray(paths) ? paths : [paths]);
    notify.success(`${added} datapack${added === 1 ? "" : "s"} added`, "They load the next time you open the world.");
  } catch (e) {
    notify.error("Could not add datapack", errorText(e));
  }
  loadPacks();
}

async function remove() {
  const p = removing.value;
  removing.value = null;
  if (!p) return;
  try {
    await api.removeDatapack(props.instance.id, world.value, p.name);
  } catch (e) {
    notify.error("Could not remove datapack", errorText(e));
  }
  loadPacks();
}

function size(p: Datapack) {
  if (p.folder) return "Folder";
  return p.size > 1048576 ? `${(p.size / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(p.size / 1024))} KB`;
}

watch(world, loadPacks);

onMounted(async () => {
  try {
    worlds.value = await api.worlds(props.instance.id);
  } catch (e) {
    notify.error("Could not read worlds", errorText(e));
  }
  if (!worlds.value.some((w) => w.folder === world.value)) world.value = worlds.value[0]?.folder ?? "";
  await loadPacks();
  loaded.value = true;
});
</script>

<template>
  <div class="pane">
    <template v-if="worlds.length">
      <div class="toolbar">
        <Dropdown v-model="world" :options="options" class="pick" />
        <span class="muted grow">{{ packs.length }} datapack{{ packs.length === 1 ? "" : "s" }}</span>
        <button class="btn" @click="api.openInstanceFolder(instance.id, `saves/${world}/datapacks`)">Open folder</button>
        <button class="btn primary" @click="importPacks">+ Add datapack</button>
      </div>
      <div v-if="packs.length" class="scroll list">
        <div v-for="p in packs" :key="p.name" class="pack">
          <span class="kind">{{ p.folder ? "DIR" : "ZIP" }}</span>
          <span class="grow ellipsis name">{{ p.name.replace(/\.zip$/i, "") }}</span>
          <span class="muted">{{ size(p) }}</span>
          <button class="btn ghost danger-text" @click="removing = p">Remove</button>
        </div>
      </div>
      <div v-else-if="loaded" class="empty">This world has no datapacks. Add a .zip datapack to get started.</div>
    </template>
    <div v-else-if="loaded" class="empty">Datapacks live inside worlds. Create a world in game first.</div>

    <Modal v-if="removing" title="Remove datapack?" :width="420" @close="removing = null">
      <div class="confirm">
        <p><b>{{ removing.name }}</b> will be deleted from this world. Anything it added stays in the world, but its recipes and functions stop working.</p>
        <div class="row end">
          <button class="btn" @click="removing = null">Cancel</button>
          <button class="btn danger" @click="remove">Remove</button>
        </div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>
.pane {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: calc(12px * var(--text-scale, 1));
}

.pick {
  width: 240px;
}

.list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.pack {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 8px 6px 10px;
  border-radius: var(--r-small);
  background: var(--panel-2);
  border: 1px solid transparent;
  font-size: calc(12px * var(--text-scale, 1));
}

.pack:hover {
  border-color: var(--border-soft);
}

.kind {
  font-size: calc(9px * var(--text-scale, 1));
  font-weight: 700;
  letter-spacing: 0.06em;
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--panel-3);
  color: var(--text-faint);
}

.name {
  font-weight: 500;
}

.danger-text {
  color: var(--danger-text);
}

.confirm {
  padding: 0 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.empty {
  flex: 1;
  display: grid;
  place-items: center;
  color: var(--text-dim);
  font-size: calc(12px * var(--text-scale, 1));
}
</style>
