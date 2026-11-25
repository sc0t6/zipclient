import { rows } from "./locales";

export const languages = [
  { value: "en", label: "English" },
  { value: "es", label: "Español" },
  { value: "pt-BR", label: "Português (Brasil)" },
  { value: "de", label: "Deutsch" },
  { value: "fr", label: "Français" },
  { value: "pl", label: "Polski" },
  { value: "ru", label: "Русский" },
];

const columns: Record<string, number> = { es: 1, "pt-BR": 2, de: 3, fr: 4, pl: 5, ru: 6 };
const ATTRS = ["placeholder", "title", "aria-label"];
const SKIP = "script,style,textarea,[translate=no]";

let dict: Map<string, string> | null = null;
let current = "en";
const source = new WeakMap<Node, string>();
const written = new WeakMap<Node, string>();
const attrSource = new WeakMap<Element, Record<string, string>>();
const attrWritten = new WeakMap<Element, Record<string, string>>();

function lookup(text: string): string {
  if (!dict) return text;
  const trimmed = text.trim();
  const hit = trimmed && dict.get(trimmed.replace(/\s+/g, " "));
  return hit ? text.replace(trimmed, hit) : text;
}

function refreshText(node: Text) {
  if (node.parentElement?.closest(SKIP)) return;
  const original = written.get(node) === node.data ? source.get(node) ?? node.data : node.data;
  source.set(node, original);
  const next = lookup(original);
  written.set(node, next);
  if (node.data !== next) node.data = next;
}

function refreshAttrs(el: Element) {
  for (const name of ATTRS) {
    const value = el.getAttribute(name);
    if (value === null) continue;
    const store = attrSource.get(el) ?? {};
    const done = attrWritten.get(el) ?? {};
    const original = done[name] === value ? store[name] ?? value : value;
    store[name] = original;
    const next = lookup(original);
    done[name] = next;
    attrSource.set(el, store);
    attrWritten.set(el, done);
    if (next !== value) el.setAttribute(name, next);
  }
}

function walk(root: Node) {
  if (root.nodeType === Node.TEXT_NODE) return refreshText(root as Text);
  if (root.nodeType !== Node.ELEMENT_NODE || (root as Element).closest(SKIP)) return;
  refreshAttrs(root as Element);
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => (n.nodeType === Node.ELEMENT_NODE && (n as Element).matches(SKIP) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (n.nodeType === Node.TEXT_NODE) refreshText(n as Text);
    else refreshAttrs(n as Element);
  }
}

const observer = new MutationObserver((records) => {
  for (const r of records) {
    if (r.type === "characterData") refreshText(r.target as Text);
    else if (r.type === "attributes") refreshAttrs(r.target as Element);
    else r.addedNodes.forEach(walk);
  }
});

export function startI18n() {
  observer.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
}

export function setLanguage(lang: string) {
  if (lang === current) return;
  current = lang;
  const col = columns[lang];
  dict = col ? new Map(rows.map((r) => [r[0], r[col]])) : null;
  document.documentElement.lang = lang;
  walk(document.body);
}

export function t(text: string): string {
  return dict?.get(text) ?? text;
}
