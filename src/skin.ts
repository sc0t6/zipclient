import { api } from "./api";

const textures = new Map<string, Promise<string>>();
const faces = new Map<string, Promise<string>>();

export function texture(url: string | null | undefined, variant?: string | null): Promise<string> {
  const key = url || `default:${variant === "slim" ? "slim" : "classic"}`;
  let pending = textures.get(key);
  if (!pending) {
    pending = (url ? api.skinTexture(url) : api.defaultSkin(variant === "slim" ? "slim" : "classic")).catch((e) => {
      textures.delete(key);
      throw e;
    });
    textures.set(key, pending);
  }
  return pending;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("image"));
    img.src = src;
  });
}

const bodies = new Map<string, Promise<string>>();

export function bodyFront(dataUrl: string, slim: boolean): Promise<string> {
  const key = `${slim ? "s" : "c"}|${dataUrl}`;
  let pending = bodies.get(key);
  if (!pending) {
    pending = loadImage(dataUrl).then((img) => {
      const modern = img.height >= 64;
      const arm = slim ? 3 : 4;
      const canvas = document.createElement("canvas");
      canvas.width = 16;
      canvas.height = 32;
      const ctx = canvas.getContext("2d")!;
      ctx.imageSmoothingEnabled = false;
      const part = (sx: number, sy: number, w: number, h: number, dx: number, dy: number, mirror = false) => {
        if (mirror) {
          ctx.save();
          ctx.translate(dx + w, dy);
          ctx.scale(-1, 1);
          ctx.drawImage(img, sx, sy, w, h, 0, 0, w, h);
          ctx.restore();
        } else {
          ctx.drawImage(img, sx, sy, w, h, dx, dy, w, h);
        }
      };
      const armX = 4 - arm;
      part(8, 8, 8, 8, 4, 0);
      part(20, 20, 8, 12, 4, 8);
      part(44, 20, arm, 12, armX, 8);
      if (modern) part(36, 52, arm, 12, 12, 8);
      else part(44, 20, arm, 12, 12, 8, true);
      part(4, 20, 4, 12, 4, 20);
      if (modern) part(20, 52, 4, 12, 8, 20);
      else part(4, 20, 4, 12, 8, 20, true);
      if (modern) {
        part(40, 8, 8, 8, 4, 0);
        part(20, 36, 8, 12, 4, 8);
        part(44, 36, arm, 12, armX, 8);
        part(52, 52, arm, 12, 12, 8);
        part(4, 36, 4, 12, 4, 20);
        part(4, 52, 4, 12, 8, 20);
      } else {
        part(40, 8, 8, 8, 4, 0);
      }
      return canvas.toDataURL();
    });
    bodies.set(key, pending);
  }
  return pending;
}

export async function looksSlim(dataUrl: string): Promise<boolean> {
  const img = await loadImage(dataUrl);
  if (img.height < 64) return false;
  const canvas = document.createElement("canvas");
  canvas.width = 64;
  canvas.height = 64;
  const ctx = canvas.getContext("2d")!;
  ctx.drawImage(img, 0, 0);
  const pixel = ctx.getImageData(54, 20, 1, 1).data;
  return pixel[3] === 0;
}

export function face(dataUrl: string): Promise<string> {
  let pending = faces.get(dataUrl);
  if (!pending) {
    pending = loadImage(dataUrl).then((img) => {
      const canvas = document.createElement("canvas");
      canvas.width = 8;
      canvas.height = 8;
      const ctx = canvas.getContext("2d")!;
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 8, 8);
      if (img.height >= 64 || img.width >= 64) ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 8, 8);
      return canvas.toDataURL();
    });
    faces.set(dataUrl, pending);
  }
  return pending;
}
