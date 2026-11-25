import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "..");
const legal = JSON.parse(fs.readFileSync(path.join(root, "legal", "legal.json"), "utf8"));
const version = JSON.parse(fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8")).version;

const escape = (text) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const pages = { terms: "terms.html", privacy: "privacy.html", disclaimer: "disclaimer.html" };

const header = `<a class="skip" href="#main">Skip to content</a>
    <header class="site scrolled" id="top">
      <div class="wrap bar">
        <a class="brand" href="index.html"><img src="assets/icon.png" alt="" width="34" height="34" />Zip Client</a>
        <nav class="links" id="nav" aria-label="Main">
          <a href="index.html#features">Features</a>
          <a href="index.html#horizon">Horizon</a>
          <a href="index.html#privacy">Privacy</a>
          <a href="index.html#faq">FAQ</a>
        </nav>
        <a class="btn primary small" href="https://github.com/sc0t6/zipclient/releases/tag/Release" rel="noopener">Download</a>
        <button class="menu-toggle" type="button" aria-controls="nav" aria-expanded="false" aria-label="Open menu">
          <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><path d="M2 4h14M2 9h14M2 14h14" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg>
        </button>
      </div>
    </header>`;

const footer = fs.readFileSync(path.join(here, "index.html"), "utf8").match(/<footer class="site">[\s\S]*?<\/footer>/)[0].replace(/href="#(features|download)"/g, 'href="index.html#$1"').replace('href="#top"', 'href="index.html"');

for (const [key, file] of Object.entries(pages)) {
  const doc = legal.documents[key];
  const nav = Object.entries(pages)
    .map(([k, f]) => `<a href="${f}"${k === key ? ' aria-current="page"' : ""}>${escape(legal.documents[k].title)}</a>`)
    .join("\n            ");
  const sections = doc.sections
    .map((s) => `<h2>${escape(s.heading)}</h2>\n          ${s.body.map((p) => `<p>${escape(p)}</p>`).join("\n          ")}`)
    .join("\n          ");
  const html = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; font-src 'self'; base-uri 'self'; form-action 'none'" />
    <meta name="referrer" content="strict-origin-when-cross-origin" />
    <title>${escape(doc.title)} · Zip Client</title>
    <meta name="description" content="${escape(doc.title)} for Zip Client, produced by ${escape(legal.provider)}." />
    <meta name="theme-color" content="#141414" />
    <link rel="icon" type="image/png" href="assets/favicon.png" />
    <link rel="stylesheet" href="assets/style.css" />
  </head>
  <body>
    ${header}
    <main id="main">
      <div class="wrap">
        <article class="doc">
          <nav class="doc-nav" aria-label="Legal documents">
            ${nav}
          </nav>
          <h1>${escape(doc.title)}</h1>
          <p class="meta">Effective ${escape(legal.effective)} · ${escape(legal.provider)} · <a href="${legal.providerUrl}" rel="noopener">krizuvo.xyz</a></p>
          <p class="intro">${escape(doc.intro)}</p>
          ${sections}
          <p class="meta">Contact: <a href="mailto:${legal.contactEmail}">${legal.contactEmail}</a> · <a href="${legal.providerUrl}" rel="noopener">krizuvo.xyz</a> · <a href="${legal.contactUrl}" rel="noopener">Zip issue tracker</a></p>
        </article>
      </div>
    </main>

    ${footer}
    <script src="assets/site.js"></script>
  </body>
</html>
`;
  fs.writeFileSync(path.join(here, file), html);
  console.log(`wrote ${file}`);
}

const index = path.join(here, "index.html");
fs.writeFileSync(index, fs.readFileSync(index, "utf8").replace(/<span data-version>[^<]*<\/span>/g, `<span data-version>${version}</span>`));
console.log(`version ${version}`);
