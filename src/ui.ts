export interface UiLayout {
  font: string;
  customFont: string;
  textScale: number;
  radius: number;
  background: { mode: "theme" | "color" | "image"; color: string; dim: number; blur: number; panels: number };
  home: { accounts: boolean; news: boolean; swap: boolean; hero: "small" | "normal" | "large" };
  instances: { side: "left" | "right"; width: "narrow" | "normal" | "wide" };
  mods: { view: "list" | "grid"; descriptions: boolean; compact: boolean };
  skins: { preview: "left" | "right"; size: "small" | "medium" | "large" };
  sources: { modrinth: boolean; curseforge: boolean };
}

export const fonts = [
  { id: "inter", label: "Inter", stack: '"Inter Variable", "Inter", "Segoe UI", system-ui, sans-serif' },
  { id: "system", label: "System", stack: 'system-ui, "Segoe UI", Ubuntu, "Noto Sans", sans-serif' },
  { id: "wide", label: "Wide", stack: 'Verdana, "DejaVu Sans", "Bitstream Vera Sans", sans-serif' },
  { id: "serif", label: "Serif", stack: 'Georgia, Cambria, "Times New Roman", serif' },
  { id: "mono", label: "Mono", stack: '"Cascadia Code", Consolas, "DejaVu Sans Mono", monospace' },
  { id: "hand", label: "Handwritten", stack: '"Caveat", "Segoe Print", cursive' },
  { id: "custom", label: "Custom", stack: "" },
];

export const defaultUi: UiLayout = {
  font: "inter",
  customFont: "",
  textScale: 1,
  radius: 1,
  background: { mode: "theme", color: "#1f3a2b", dim: 0.2, blur: 0, panels: 0.78 },
  home: { accounts: true, news: true, swap: false, hero: "normal" },
  instances: { side: "left", width: "normal" },
  mods: { view: "list", descriptions: true, compact: false },
  skins: { preview: "left", size: "medium" },
  sources: { modrinth: true, curseforge: true },
};

const clampNum = (v: unknown, min: number, max: number, fallback: number) => (typeof v === "number" && Number.isFinite(v) ? Math.min(max, Math.max(min, v)) : fallback);
const pick = <T extends string>(v: unknown, options: readonly T[], fallback: T): T => (options.includes(v as T) ? (v as T) : fallback);
const bool = (v: unknown, fallback: boolean) => (typeof v === "boolean" ? v : fallback);
const hex = (v: unknown, fallback: string) => (typeof v === "string" && /^#[0-9a-f]{6}$/i.test(v) ? v : fallback);
const obj = (v: unknown): Record<string, unknown> => (v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : {});

export function readUi(raw: unknown): UiLayout {
  const r = obj(raw);
  const d = defaultUi;
  const bg = obj(r.background);
  const home = obj(r.home);
  const inst = obj(r.instances);
  const mods = obj(r.mods);
  const skins = obj(r.skins);
  const sources = obj(r.sources);
  return {
    font: pick(r.font, fonts.map((f) => f.id), d.font),
    customFont: typeof r.customFont === "string" ? r.customFont.replace(/[;{}<>"\\]/g, "").slice(0, 60) : "",
    textScale: clampNum(r.textScale, 0.85, 1.3, d.textScale),
    radius: clampNum(r.radius, 0, 2, d.radius),
    background: {
      mode: pick(bg.mode, ["theme", "color", "image"] as const, d.background.mode),
      color: hex(bg.color, d.background.color),
      dim: clampNum(bg.dim, 0, 0.9, d.background.dim),
      blur: clampNum(bg.blur, 0, 24, d.background.blur),
      panels: clampNum(bg.panels, 0.35, 1, d.background.panels),
    },
    home: {
      accounts: bool(home.accounts, d.home.accounts),
      news: bool(home.news, d.home.news),
      swap: bool(home.swap, d.home.swap),
      hero: pick(home.hero, ["small", "normal", "large"] as const, d.home.hero),
    },
    instances: {
      side: pick(inst.side, ["left", "right"] as const, d.instances.side),
      width: pick(inst.width, ["narrow", "normal", "wide"] as const, d.instances.width),
    },
    mods: {
      view: pick(mods.view, ["list", "grid"] as const, d.mods.view),
      descriptions: bool(mods.descriptions, d.mods.descriptions),
      compact: bool(mods.compact, d.mods.compact),
    },
    skins: {
      preview: pick(skins.preview, ["left", "right"] as const, d.skins.preview),
      size: pick(skins.size, ["small", "medium", "large"] as const, d.skins.size),
    },
    sources: {
      modrinth: bool(sources.modrinth, d.sources.modrinth),
      curseforge: bool(sources.curseforge, d.sources.curseforge),
    },
  };
}

const radii: [string, number][] = [
  ["--r-window", 12],
  ["--r-panel", 12],
  ["--r-row", 9],
  ["--r-chip", 7],
  ["--r-small", 5],
];
const surfaces = ["--panel", "--panel-2", "--panel-3", "--panel-4", "--field", "--chip", "--chip-hover", "--chip-active"];

export function applyUi(root: HTMLElement, ui: UiLayout) {
  const font = ui.font === "custom" ? (ui.customFont ? `"${ui.customFont}", ${fonts[0].stack}` : fonts[0].stack) : fonts.find((f) => f.id === ui.font)?.stack ?? fonts[0].stack;
  root.style.setProperty("--font", font);
  root.style.setProperty("--text-scale", String(ui.textScale));
  for (const [name, base] of radii) root.style.setProperty(name, `${Math.round(base * ui.radius * 10) / 10}px`);
  for (const name of [...surfaces, "--bg"]) root.style.removeProperty(name);
  const custom = ui.background.mode !== "theme";
  root.dataset.customBg = String(custom);
  if (!custom) return;
  const computed = getComputedStyle(root);
  const alpha = Math.round(ui.background.panels * 100);
  for (const name of surfaces) {
    const base = computed.getPropertyValue(name).trim();
    if (base) root.style.setProperty(name, `color-mix(in srgb, ${base} ${alpha}%, transparent)`);
  }
  root.style.setProperty("--bg", "transparent");
}
