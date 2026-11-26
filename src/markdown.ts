export type Inline = { t: "text" | "b" | "i" | "code"; v: string } | { t: "link"; v: string; href: string };
export type Block =
  | { t: "h"; level: number; inline: Inline[] }
  | { t: "p"; inline: Inline[] }
  | { t: "quote"; inline: Inline[] }
  | { t: "list"; ordered: boolean; items: Inline[][] }
  | { t: "img"; src: string; alt: string }
  | { t: "code"; text: string }
  | { t: "hr" };

const entities: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", mdash: "—", ndash: "–", hellip: "…", copy: "©", reg: "®", trade: "™", rsquo: "’", lsquo: "‘", rdquo: "”", ldquo: "“", bull: "•" };

function decode(text: string): string {
  return text.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (m, code: string) => {
    if (code[0] === "#") {
      const n = code[1] === "x" || code[1] === "X" ? parseInt(code.slice(2), 16) : parseInt(code.slice(1), 10);
      return Number.isFinite(n) && n > 0 && n < 0x110000 ? String.fromCodePoint(n) : m;
    }
    return entities[code.toLowerCase()] ?? m;
  });
}

function attr(tag: string, name: string): string {
  const m = new RegExp(`${name}\\s*=\\s*("([^"]*)"|'([^']*)'|([^\\s>]+))`, "i").exec(tag);
  return m ? decode(m[2] ?? m[3] ?? m[4] ?? "") : "";
}

export function htmlToMarkdown(html: string): string {
  return decode(
    html
      .replace(/<(script|style|iframe)[\s\S]*?<\/\1>/gi, "")
      .replace(/<!--[\s\S]*?-->/g, "")
      .replace(/<img\b[^>]*>/gi, (tag) => `\n\n![${attr(tag, "alt")}](${attr(tag, "src")})\n\n`)
      .replace(/<a\b[^>]*>([\s\S]*?)<\/a>/gi, (whole, inner: string) => {
        const href = attr(whole, "href");
        return /^\s*!\[/.test(inner) || !href ? inner : `[${inner}](${href})`;
      })
      .replace(/<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/gi, (_, level: string, inner: string) => `\n\n${"#".repeat(Math.min(3, Number(level)))} ${inner.replace(/\s+/g, " ")}\n\n`)
      .replace(/<br\s*\/?>/gi, "\n")
      .replace(/<li\b[^>]*>/gi, "\n- ")
      .replace(/<(strong|b)\b[^>]*>([\s\S]*?)<\/\1>/gi, "**$2**")
      .replace(/<(em|i)>([\s\S]*?)<\/\1>/gi, "*$2*")
      .replace(/<code\b[^>]*>([\s\S]*?)<\/code>/gi, "`$1`")
      .replace(/<hr\b[^>]*>/gi, "\n\n---\n\n")
      .replace(/<\/?(p|div|ul|ol|table|tr|blockquote|section|center|details|summary)\b[^>]*>/gi, "\n\n")
      .replace(/<[^>]+>/g, ""),
  );
}

function badge(src: string): boolean {
  return /shields\.io|badge|\/badges?\/|bstats\.org|img\.buymeacoffee|ko-fi\.com\/img/i.test(src);
}

function safeUrl(url: string): string {
  const u = url.trim().replace(/^<|>$/g, "");
  return /^https:\/\//i.test(u) ? u : "";
}

export function inline(text: string): Inline[] {
  const out: Inline[] = [];
  const re = /\*\*([^*]+)\*\*|__([^_]+)__|`([^`]+)`|\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|(?<![\w*])\*([^*\s][^*]*)\*(?!\w)|(?<![\w_])_([^_\s][^_]*)_(?!\w)/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index! > last) out.push({ t: "text", v: text.slice(last, m.index) });
    const styled = (inner: string, t: "b" | "i") => out.push(...inline(inner).map((p): Inline => (p.t === "text" ? { t, v: p.v } : p)));
    if (m[1] ?? m[2]) styled(m[1] ?? m[2], "b");
    else if (m[3]) out.push({ t: "code", v: m[3] });
    else if (m[5] !== undefined) {
      const href = safeUrl(m[5]);
      out.push(href ? { t: "link", v: m[4] || href, href } : { t: "text", v: m[4] });
    } else styled(m[6] ?? m[7], "i");
    last = m.index! + m[0].length;
  }
  if (last < text.length) out.push({ t: "text", v: text.slice(last) });
  return out.filter((p) => p.v);
}

export function parse(source: string): Block[] {
  const text = htmlToMarkdown(source.replace(/\r\n?/g, "\n")).slice(0, 80000);
  const blocks: Block[] = [];
  let para: string[] = [];
  let list: { ordered: boolean; items: Inline[][] } | null = null;
  const flushPara = () => {
    const joined = para.join(" ").replace(/\s+/g, " ").trim();
    if (joined) blocks.push({ t: "p", inline: inline(joined) });
    para = [];
  };
  const flushList = () => {
    if (list?.items.length) blocks.push({ t: "list", ...list });
    list = null;
  };
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i];
    const line = raw.trim();
    if (line.startsWith("```")) {
      flushPara();
      flushList();
      const code: string[] = [];
      while (++i < lines.length && !lines[i].trim().startsWith("```")) code.push(lines[i]);
      blocks.push({ t: "code", text: code.join("\n") });
      continue;
    }
    if (!line) {
      flushPara();
      flushList();
      continue;
    }
    const images = [...line.matchAll(/\[?!\[([^\]]*)\]\(([^)\s]+)[^)]*\)(?:\]\([^)]*\))?/g)];
    const rest = images.length ? line.replace(/\[?!\[([^\]]*)\]\(([^)\s]+)[^)]*\)(?:\]\([^)]*\))?/g, "").trim() : line;
    if (images.length) {
      flushPara();
      flushList();
      for (const m of images) {
        const src = safeUrl(m[2]);
        if (src && !badge(src)) blocks.push({ t: "img", src, alt: m[1] });
      }
      if (!rest) continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(rest);
    const bullet = /^([-*+]|\d+[.)])\s+(.*)$/.exec(rest);
    if (heading) {
      flushPara();
      flushList();
      const body = heading[2].replace(/#+$/, "").trim();
      if (body) blocks.push({ t: "h", level: Math.min(3, heading[1].length), inline: inline(body) });
    } else if (/^([-*_])(\s*\1){2,}$/.test(rest)) {
      flushPara();
      flushList();
      blocks.push({ t: "hr" });
    } else if (bullet) {
      flushPara();
      const ordered = /\d/.test(bullet[1]);
      if (!list || list.ordered !== ordered) {
        flushList();
        list = { ordered, items: [] };
      }
      list.items.push(inline(bullet[2]));
    } else if (rest.startsWith(">")) {
      flushPara();
      flushList();
      blocks.push({ t: "quote", inline: inline(rest.replace(/^>\s?/, "")) });
    } else if (/^\|?[\s:|-]+\|[\s:|-]*$/.test(rest)) {
      continue;
    } else if (list && /^\s{2,}/.test(raw)) {
      list.items[list.items.length - 1].push({ t: "text", v: ` ${rest}` });
    } else {
      flushList();
      para.push(rest.replace(/^\||\|$/g, "").replace(/\s*\|\s*/g, "  ·  "));
    }
  }
  flushPara();
  flushList();
  return blocks;
}

export function plainText(blocks: Block[]): string {
  const words = (parts: Inline[]) => parts.map((p) => p.v).join("");
  return blocks
    .map((b) => {
      if (b.t === "h" || b.t === "p" || b.t === "quote") return words(b.inline);
      if (b.t === "list") return b.items.map((item) => `• ${words(item)}`).join("\n");
      return "";
    })
    .filter(Boolean)
    .join("\n\n");
}
