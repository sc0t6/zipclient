import { defineStore } from "pinia";
import { computed, reactive, ref, shallowRef } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  errorText,
  type Account,
  type AppInfo,
  type GameState,
  type Instance,
  type ProgressEvent,
  type UpdateStatus,
  type VersionList,
} from "../api";
import { useNotify } from "./notify";
import { useSettings } from "./settings";

export type Page = "home" | "instances" | "servers" | "skins" | "settings";

export const useApp = defineStore("app", () => {
  const info = ref<AppInfo | null>(null);
  const page = ref<Page>("home");
  const instances = ref<Instance[]>([]);
  const accounts = ref<Account[]>([]);
  const versions = shallowRef<VersionList | null>(null);
  const games = reactive<Record<string, GameState>>({});
  const progress = reactive<Record<string, ProgressEvent>>({});
  const update = ref<UpdateStatus | null>(null);
  const updateChecking = ref(false);
  const loginOpen = ref(false);
  const exitOpen = ref(false);
  const createOpen = ref(false);
  const updateOpen = ref(false);
  const legalDoc = ref<"" | "terms" | "privacy" | "disclaimer">("");
  const selectedInstance = ref<string>("");
  const instanceTab = ref("overview");
  const quickInstance = ref<string>("");
  const icons = reactive<Record<string, string | null>>({});
  const localSkins = ref<Record<string, { dataUrl: string; variant: string }>>({});

  const activeAccount = computed(() => accounts.value.find((a) => a.active) ?? accounts.value[0] ?? null);
  const running = computed(() => Object.values(games).filter((g) => g.state === "running"));
  const busy = computed(() => Object.values(games).filter((g) => g.state !== "running"));
  const quick = computed(
    () => instances.value.find((i) => i.id === quickInstance.value) ?? [...instances.value].sort((a, b) => b.last_played - a.last_played)[0] ?? null,
  );
  const totalPlaytime = computed(() => instances.value.reduce((sum, i) => sum + i.playtime, 0));

  function go(next: Page) {
    page.value = next;
    const settings = useSettings();
    if (settings.s) settings.s.advanced.last_page = next;
  }

  function openInstance(id: string, tab = "overview") {
    selectedInstance.value = id;
    instanceTab.value = tab;
    go("instances");
  }

  async function refreshInstances() {
    instances.value = await api.instances();
    for (const i of instances.value) {
      if (i.icon.startsWith("custom") && icons[i.id + i.icon] === undefined) {
        icons[i.id + i.icon] = null;
        api.instanceIcon(i.id).then((data) => (icons[i.id + i.icon] = data));
      }
    }
  }

  function iconFor(instance: Instance) {
    return instance.icon.startsWith("custom") ? icons[instance.id + instance.icon] ?? null : null;
  }

  function replaceInstance(instance: Instance) {
    const index = instances.value.findIndex((i) => i.id === instance.id);
    if (index >= 0) instances.value.splice(index, 1, instance);
    else instances.value.unshift(instance);
    if (instance.icon.startsWith("custom") && icons[instance.id + instance.icon] === undefined) {
      icons[instance.id + instance.icon] = null;
      api.instanceIcon(instance.id).then((data) => (icons[instance.id + instance.icon] = data));
    }
  }

  async function refreshAccounts() {
    accounts.value = await api.accounts();
  }

  async function refreshLocalSkins() {
    const lib = await api.skins().catch(() => null);
    if (!lib) return;
    const next: Record<string, { dataUrl: string; variant: string }> = {};
    for (const [account, skinId] of Object.entries(lib.selected)) {
      const skin = lib.skins.find((s) => s.id === skinId);
      if (skin) next[account] = { dataUrl: skin.dataUrl, variant: skin.variant };
    }
    localSkins.value = next;
  }

  async function loadVersions(force = false) {
    try {
      versions.value = await api.versions(force);
    } catch (e) {
      if (force) useNotify().error("Could not load Minecraft versions", errorText(e));
    }
  }

  async function checkUpdate(manual = false) {
    updateChecking.value = true;
    try {
      update.value = await api.checkUpdate();
      const settings = useSettings();
      if (update.value.available && (!update.value.skipped || manual)) {
        updateOpen.value = true;
      } else if (manual && update.value.error) {
        useNotify().info("No update found", `You have Zip ${update.value.current}. ${update.value.error}`);
      } else if (manual && update.value.configured && !update.value.available) {
        useNotify().success("Zip is up to date", `Version ${update.value.current}`);
      }
    } finally {
      updateChecking.value = false;
    }
  }

  async function launch(id: string, server: string | null = null) {
    const notify = useNotify();
    quickInstance.value = id;
    try {
      await api.launch(id, server);
    } catch (e) {
      notify.error("Launch failed", errorText(e));
    }
  }

  async function kill(id: string) {
    try {
      await api.kill(id);
    } catch (e) {
      useNotify().warn("Could not stop the game", errorText(e));
    }
  }

  async function init() {
    info.value = await api.appInfo();
    const [, , states] = await Promise.all([refreshInstances(), refreshAccounts(), api.gameStates()]);
    refreshLocalSkins();
    for (const state of states) games[state.instanceId] = state;
    const settings = useSettings();
    if (settings.s?.general.startup_page === "last") {
      const last = settings.s.advanced.last_page as Page;
      if (["home", "instances", "servers", "skins", "settings"].includes(last)) page.value = last;
    }
    loadVersions();
    if (settings.s?.general.check_updates) checkUpdate();

    await Promise.all([
    listen<GameState>("game-state", ({ payload }) => {
      const notify = useNotify();
      const name = instances.value.find((i) => i.id === payload.instanceId)?.name ?? "Minecraft";
      if (payload.state === "idle" || payload.state === "stopped" || payload.state === "crashed") {
        delete games[payload.instanceId];
        delete progress[payload.instanceId];
        api.instance(payload.instanceId).then(replaceInstance).catch(() => {});
        if (payload.state === "crashed") {
          const report = payload.crashReport;
          notify.push("error", `${name} crashed`, payload.message ? `Exit code ${payload.exitCode ?? "unknown"}:\n${payload.message}` : `Exit code ${payload.exitCode ?? "unknown"}.`, {
            action: report || payload.logFile ? { label: report ? "Open crash report" : "Open log", run: () => api.openFile(report ?? payload.logFile!) } : undefined,
          });
        }
      } else {
        games[payload.instanceId] = payload;
        if (payload.state === "running") notify.success(`${name} is running`);
      }
    }),
    listen<ProgressEvent>("task-progress", ({ payload }) => {
      if (!payload.phase) delete progress[payload.key];
      else progress[payload.key] = payload;
    }),
    listen<{ instanceId: string; message: string }>("game-warning", ({ payload }) => {
      const name = instances.value.find((i) => i.id === payload.instanceId)?.name ?? "Minecraft";
      useNotify().push("warn", `${name} is low on memory`, payload.message, { action: { label: "Instance settings", run: () => openInstance(payload.instanceId, "settings") } });
    }),
    listen("close-requested", () => {
      exitOpen.value = true;
    }),
    ]);
  }

  return {
    info,
    page,
    instances,
    accounts,
    versions,
    games,
    progress,
    update,
    updateChecking,
    loginOpen,
    exitOpen,
    createOpen,
    updateOpen,
    legalDoc,
    selectedInstance,
    instanceTab,
    quickInstance,
    activeAccount,
    running,
    busy,
    quick,
    totalPlaytime,
    go,
    openInstance,
    refreshInstances,
    replaceInstance,
    iconFor,
    refreshAccounts,
    refreshLocalSkins,
    localSkins,
    loadVersions,
    checkUpdate,
    launch,
    kill,
    init,
  };
});
