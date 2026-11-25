import { icons } from "./icons";
import type { Page } from "./stores/app";

export const navTabs: { id: Page; label: string; icon: string }[] = [
  { id: "home", label: "Home", icon: icons.home },
  { id: "instances", label: "Instances", icon: icons.instances },
  { id: "servers", label: "Servers", icon: icons.servers },
  { id: "skins", label: "Skins", icon: icons.skins },
  { id: "settings", label: "Settings", icon: icons.settings },
];
