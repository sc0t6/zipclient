import { defineStore } from "pinia";
import { computed, ref, toRaw, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, type Settings } from "../api";
import { setLanguage } from "../i18n";
import { applyUi, readUi, type UiLayout } from "../ui";

function contrastText(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return "#0d1f12";
  const n = parseInt(m[1], 16);
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  };
  const lum = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
  return lum > 0.45 ? "#0d1f12" : "#ffffff";
}

export const useSettings = defineStore("settings", () => {
  const s = ref<Settings | null>(null);
  const ui = computed(() => readUi(s.value?.appearance.ui));
  const backgroundImage = ref("");
  let timer = 0;
  let suppress = false;
  let lastScale = 0;

  function editUi(change: (draft: UiLayout) => void) {
    if (!s.value) return;
    const draft = structuredClone(toRaw(ui.value));
    change(draft);
    s.value.appearance.ui = draft as unknown as Record<string, unknown>;
  }

  async function loadBackground() {
    backgroundImage.value = (await api.uiBackground().catch(() => null)) ?? "";
  }

  function apply() {
    if (!s.value) return;
    const root = document.documentElement;
    const a = s.value.appearance;
    root.dataset.theme = a.theme;
    root.dataset.motion = a.animations;
    root.dataset.followMotion = String(a.follow_system_motion);
    root.style.setProperty("--accent", a.accent);
    root.style.setProperty("--on-accent", contrastText(a.accent));
    root.style.setProperty("--bg-dim", String(a.background_dim));
    applyUi(root, ui.value);
    setLanguage(s.value.general.language);
    if (a.scale !== lastScale) {
      lastScale = a.scale;
      getCurrentWebview().setZoom(a.scale).catch(() => {});
    }
  }

  async function flush() {
    if (!s.value) return;
    window.clearTimeout(timer);
    timer = 0;
    const sent = structuredClone(toRaw(s.value));
    const saved = await api.saveSettings(sent);
    if (JSON.stringify(saved) !== JSON.stringify(sent) && JSON.stringify(toRaw(s.value)) === JSON.stringify(sent)) {
      suppress = true;
      s.value = saved;
      suppress = false;
      apply();
    }
  }

  async function load() {
    suppress = true;
    s.value = await api.settings();
    suppress = false;
    apply();
    loadBackground();
  }

  function replace(next: Settings) {
    suppress = true;
    s.value = next;
    suppress = false;
    apply();
  }

  watch(
    s,
    () => {
      apply();
      if (suppress) return;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => flush().catch(() => {}), 400);
    },
    { deep: true },
  );

  return { s, ui, backgroundImage, editUi, loadBackground, load, flush, replace, apply };
});
