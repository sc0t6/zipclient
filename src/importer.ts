import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText, type ImportResult } from "./api";
import { useApp } from "./stores/app";
import { useNotify } from "./stores/notify";

export type ImportKind = "curseforge" | "prism" | "modrinth" | "any";

const filters: Record<ImportKind, { name: string; extensions: string[] }[]> = {
  curseforge: [{ name: "CurseForge modpack", extensions: ["zip"] }],
  prism: [{ name: "Prism Launcher instance", extensions: ["zip"] }],
  modrinth: [{ name: "Modrinth modpack", extensions: ["mrpack"] }],
  any: [{ name: "Modpacks and instances", extensions: ["zip", "mrpack"] }],
};

export const importing = ref(false);

function report(result: ImportResult) {
  const app = useApp();
  const notify = useNotify();
  app.replaceInstance(result.instance);
  if (result.warnings.length) {
    notify.push("warn", `Imported ${result.instance.name} with warnings`, result.warnings.slice(0, 4).join("\n") + (result.warnings.length > 4 ? `\n+${result.warnings.length - 4} more` : ""), {
      sticky: true,
    });
  } else {
    notify.success("Import complete", `${result.instance.name} is ready.`);
  }
}

export async function importFile(kind: ImportKind): Promise<ImportResult | null> {
  const path = await open({ multiple: false, directory: false, filters: filters[kind] });
  if (!path || typeof path !== "string") return null;
  return importPath(path);
}

export async function importPath(path: string): Promise<ImportResult | null> {
  if (importing.value) return null;
  importing.value = true;
  try {
    const result = await api.importInstance(path);
    report(result);
    return result;
  } catch (e) {
    useNotify().error("Import failed", errorText(e));
    return null;
  } finally {
    importing.value = false;
  }
}

export async function installModpack(projectId: string, versionId: string | null = null): Promise<ImportResult | null> {
  importing.value = true;
  try {
    const result = await api.installModpack(projectId, versionId);
    report(result);
    return result;
  } catch (e) {
    useNotify().error("Modpack install failed", errorText(e));
    return null;
  } finally {
    importing.value = false;
  }
}
