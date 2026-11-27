import { defineStore } from "pinia";
import { ref } from "vue";
import { useSettings } from "./settings";

export type Level = "info" | "success" | "warn" | "error";

export interface Toast {
  id: number;
  title: string;
  message: string;
  level: Level;
  action?: { label: string; run: () => void };
}

export interface Entry {
  id: number;
  title: string;
  message: string;
  level: Level;
  time: number;
}

let nextId = 1;
const recent = new Map<string, number>();

export const useNotify = defineStore("notify", () => {
  const toasts = ref<Toast[]>([]);
  const history = ref<Entry[]>([]);
  const unread = ref(0);
  const timers = new Map<number, number>();

  function dismiss(id: number) {
    toasts.value = toasts.value.filter((t) => t.id !== id);
    const timer = timers.get(id);
    if (timer) window.clearTimeout(timer);
    timers.delete(id);
  }

  function push(level: Level, title: string, message = "", options: { action?: Toast["action"]; sticky?: boolean; force?: boolean } = {}) {
    const settings = useSettings();
    const key = `${level}|${title}|${message}`;
    const now = Date.now();
    if ((recent.get(key) ?? 0) > now - 4000) return;
    recent.set(key, now);
    const id = nextId++;
    history.value = [{ id, title, message, level, time: now }, ...history.value].slice(0, 100);
    unread.value++;
    if (!options.force && settings.s && !settings.s.general.notifications && level !== "error") return;
    const toast: Toast = { id, title, message, level, action: options.action };
    toasts.value = [...toasts.value.slice(-3), toast];
    if (!options.sticky) {
      const duration = level === "error" ? 9000 : level === "warn" ? 7000 : 4500;
      timers.set(toast.id, window.setTimeout(() => dismiss(toast.id), duration));
    }
  }

  return {
    toasts,
    history,
    unread,
    markRead: () => (unread.value = 0),
    clearHistory: () => {
      history.value = [];
      unread.value = 0;
    },
    dismiss,
    push,
    info: (title: string, message = "") => push("info", title, message),
    success: (title: string, message = "") => push("success", title, message),
    warn: (title: string, message = "") => push("warn", title, message),
    error: (title: string, message = "") => push("error", title, message),
  };
});
