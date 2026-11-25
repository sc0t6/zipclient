import { createApp } from "vue";
import { createPinia } from "pinia";
import "@fontsource-variable/inter";
import "@fontsource/caveat/400.css";
import "./styles/base.css";
import App from "./App.vue";
import { api } from "./api";
import { startI18n } from "./i18n";

const app = createApp(App);
app.use(createPinia());
app.config.errorHandler = (err) => {
  console.error(err);
  api.logError(String((err as Error)?.stack ?? err)).catch(() => {});
};
window.addEventListener("unhandledrejection", (event) => {
  api.logError(`Unhandled: ${String(event.reason)}`).catch(() => {});
});
document.addEventListener("contextmenu", (event) => {
  const target = event.target as HTMLElement;
  if (!target.closest("input, textarea, .selectable")) event.preventDefault();
});
startI18n();
app.mount("#app");
