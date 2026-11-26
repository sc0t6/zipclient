import { api, type ProjectDetails } from "./api";
import { parse, plainText, type Block } from "./markdown";

export interface Project extends ProjectDetails {
  blocks: Block[];
  text: string;
}

const cache = new Map<string, Promise<Project>>();

export function project(source: string, id: string): Promise<Project> {
  const key = `${source}:${id}`;
  let pending = cache.get(key);
  if (!pending) {
    pending = api.contentProject(source, id).then((p) => {
      const blocks = parse(p.body || p.summary);
      return { ...p, blocks, text: plainText(blocks) };
    });
    pending.catch(() => cache.delete(key));
    cache.set(key, pending);
  }
  return pending;
}
