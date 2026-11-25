import grass from "./assets/instance-icons/grass.svg";
import pickaxe from "./assets/instance-icons/pickaxe.svg";
import sword from "./assets/instance-icons/sword.svg";
import gem from "./assets/instance-icons/gem.svg";
import tree from "./assets/instance-icons/tree.svg";
import flame from "./assets/instance-icons/flame.svg";
import star from "./assets/instance-icons/star.svg";
import potion from "./assets/instance-icons/potion.svg";

export const presetIcons: { id: string; url: string }[] = [
  { id: "grass", url: grass },
  { id: "pickaxe", url: pickaxe },
  { id: "sword", url: sword },
  { id: "gem", url: gem },
  { id: "tree", url: tree },
  { id: "flame", url: flame },
  { id: "star", url: star },
  { id: "potion", url: potion },
];

export function presetUrl(icon: string): string | null {
  const id = icon.startsWith("preset:") ? icon.slice(7) : "";
  return presetIcons.find((p) => p.id === id)?.url ?? null;
}

export function randomPreset(): string {
  return `preset:${presetIcons[Math.floor(Math.random() * presetIcons.length)].id}`;
}
