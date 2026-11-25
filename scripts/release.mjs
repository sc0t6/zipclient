import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..");
const conf = JSON.parse(fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const version = conf.version;
const product = conf.productName;
const out = path.join(root, "release");
const target = path.join(root, "src-tauri", "target", "release");
const linux = process.platform === "linux";

const buildEnv = path.join(root, "config", "build.env");
if (fs.existsSync(buildEnv)) {
  for (const line of fs.readFileSync(buildEnv, "utf8").split(/\r?\n/)) {
    const at = line.indexOf("=");
    if (at > 0 && !process.env[line.slice(0, at)]) process.env[line.slice(0, at)] = line.slice(at + 1);
  }
}

const keyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH || path.join(os.homedir(), ".tauri", "zip-updater.key");
const env = { ...process.env };
if (!env.TAURI_SIGNING_PRIVATE_KEY && fs.existsSync(keyPath)) {
  env.TAURI_SIGNING_PRIVATE_KEY = fs.readFileSync(keyPath, "utf8");
  env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= "";
}
const signed = Boolean(env.TAURI_SIGNING_PRIVATE_KEY);

const extra = signed ? "" : ` --config "{\\"bundle\\":{\\"createUpdaterArtifacts\\":false}}"`;
console.log(signed ? "Signing updater artifacts." : "No signing key found: building without updater signatures.");
execSync(`npx tauri build --bundles ${linux ? "deb,appimage" : "msi,nsis"}${extra}`, { cwd: root, env, stdio: "inherit" });
fs.mkdirSync(out, { recursive: true });

const artifacts = linux
  ? [
      [path.join(target, "bundle", "appimage", `${product}_${version}_amd64.AppImage`), "Zip-Launcher-x86_64.AppImage"],
      [path.join(target, "bundle", "deb", `${product}_${version}_amd64.deb`), "Zip-Launcher-amd64.deb"],
    ]
  : [
      [path.join(target, "zip-launcher.exe"), "zip-portable-x64.exe"],
      [path.join(target, "bundle", "msi", `${product}_${version}_x64_en-US.msi`), "zip-setup-x64.msi"],
      [path.join(target, "bundle", "nsis", `${product}_${version}_x64-setup.exe`), "zip-setup-x64.exe"],
    ];

for (const [from, name] of artifacts) {
  if (!fs.existsSync(from)) throw new Error(`Missing build artifact: ${from}`);
  const dest = path.join(out, name);
  fs.copyFileSync(from, dest);
  fs.rmSync(`${dest}.sig`, { force: true });
  if (fs.existsSync(`${from}.sig`)) fs.copyFileSync(`${from}.sig`, `${dest}.sig`);
}

if (linux) {
  const stage = path.join(root, "src-tauri", "target", "portable-linux", "zip-launcher");
  fs.rmSync(path.dirname(stage), { recursive: true, force: true });
  fs.mkdirSync(path.join(stage, "ZipData"), { recursive: true });
  fs.copyFileSync(path.join(target, "zip-launcher"), path.join(stage, "zip-launcher"));
  fs.chmodSync(path.join(stage, "zip-launcher"), 0o755);
  fs.copyFileSync(path.join(root, "src-tauri", "icons", "128x128.png"), path.join(stage, "zip-launcher.png"));
  fs.writeFileSync(
    path.join(stage, "README.txt"),
    `Zip Launcher ${version} portable for Linux x86_64

Run ./zip-launcher. Everything Zip saves stays in the ZipData folder next to it.
Delete the ZipData folder to use ~/.local/share/Zip instead.

Needs WebKitGTK 4.1 and GTK 3 from your distribution:
  Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0
  Fedora:        sudo dnf install webkit2gtk4.1 gtk3
  Arch:          sudo pacman -S webkit2gtk-4.1 gtk3

Minecraft 1.12.2 and older also need xrandr (x11-xserver-utils on Debian/Ubuntu).
`,
  );
  execSync(`tar -czf "${path.join(out, "Zip-Launcher-Portable-linux-x64.tar.gz")}" -C "${path.dirname(stage)}" zip-launcher`, { stdio: "inherit" });
}

const skip = new Set(["SHA256SUMS.txt", "latest.json"]);
const sums = fs
  .readdirSync(out)
  .filter((f) => !skip.has(f) && !f.endsWith(".sig"))
  .sort()
  .map((f) => {
    const file = path.join(out, f);
    console.log(`${f}  ${(fs.statSync(file).size / 1048576).toFixed(1)} MB`);
    return `${createHash("sha256").update(fs.readFileSync(file)).digest("hex")}  ${f}`;
  });
fs.writeFileSync(path.join(out, "SHA256SUMS.txt"), sums.join("\n") + "\n");

const base = process.env.ZIP_RELEASE_BASE_URL?.replace(/\/$/, "");
const asset = (file) => {
  const sig = path.join(out, `${file}.sig`);
  return fs.existsSync(sig) ? { signature: fs.readFileSync(sig, "utf8").trim(), url: `${base}/${file}` } : null;
};
if (base) {
  const latestPath = path.join(out, "latest.json");
  const previous = fs.existsSync(latestPath) ? JSON.parse(fs.readFileSync(latestPath, "utf8")) : null;
  const platforms = previous?.version === version ? { ...previous.platforms } : {};
  const add = (keys, file) => {
    const entry = asset(file);
    if (entry) for (const key of keys) platforms[key] = entry;
  };
  if (linux) {
    add(["linux-x86_64", "linux-x86_64-appimage"], "Zip-Launcher-x86_64.AppImage");
    add(["linux-x86_64-deb"], "Zip-Launcher-amd64.deb");
  } else {
    add(["windows-x86_64", "windows-x86_64-nsis"], "zip-setup-x64.exe");
    add(["windows-x86_64-msi"], "zip-setup-x64.msi");
  }
  const manifest = { version, notes: process.env.ZIP_RELEASE_NOTES || `Zip ${version}`, pub_date: new Date().toISOString(), platforms };
  fs.writeFileSync(latestPath, JSON.stringify(manifest, null, 2));
  console.log(`latest.json written (${Object.keys(platforms).join(", ")})`);
}
console.log(`Release files are in ${out}`);
