<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref } from "vue";
import TitleBar from "./components/TitleBar.vue";
import SideNav from "./components/SideNav.vue";
import Toasts from "./components/Toasts.vue";
import ConfirmExit from "./components/ConfirmExit.vue";
import HomePage from "./pages/Home.vue";
import { useApp } from "./stores/app";
import { useSettings } from "./stores/settings";
import { api, errorText } from "./api";
import legal from "../legal/legal.json";

const InstancesPage = defineAsyncComponent(() => import("./pages/Instances.vue"));
const SkinsPage = defineAsyncComponent(() => import("./pages/Skins.vue"));
const ServersPage = defineAsyncComponent(() => import("./pages/Servers.vue"));
const SettingsPage = defineAsyncComponent(() => import("./pages/Settings.vue"));
const SetupWizard = defineAsyncComponent(() => import("./pages/Setup.vue"));
const LoginDialog = defineAsyncComponent(() => import("./components/LoginDialog.vue"));
const CreateInstance = defineAsyncComponent(() => import("./components/CreateInstance.vue"));
const UpdateDialog = defineAsyncComponent(() => import("./components/UpdateDialog.vue"));
const LegalDialog = defineAsyncComponent(() => import("./components/LegalDialog.vue"));
const LegalGate = defineAsyncComponent(() => import("./components/LegalGate.vue"));

const app = useApp();
const settings = useSettings();
const ready = ref(false);
const fatal = ref("");

const pages = { home: HomePage, instances: InstancesPage, servers: ServersPage, skins: SkinsPage, settings: SettingsPage };
const current = computed(() => pages[app.page]);
const bg = computed(() => settings.ui.background);
const bgStyle = computed(() => ({
  backgroundColor: bg.value.color,
  backgroundImage: bg.value.mode === "image" && settings.backgroundImage ? `url(${settings.backgroundImage})` : "none",
  filter: bg.value.blur ? `blur(${bg.value.blur}px)` : "none",
  inset: `${-bg.value.blur * 2}px`,
}));

onMounted(async () => {
  try {
    await settings.load();
    await app.init();
    ready.value = true;
  } catch (e) {
    fatal.value = errorText(e);
    api.logError(`Startup: ${fatal.value}`).catch(() => {});
  }
});
</script>

<template>
  <div class="window">
    <div v-if="settings.s && bg.mode !== 'theme'" class="app-bg">
      <div class="app-bg-image" :style="bgStyle" />
      <div class="app-bg-dim" :style="{ opacity: bg.dim }" />
    </div>
    <TitleBar />
    <div v-if="ready" class="stage">
      <SideNav v-if="settings.s?.appearance.nav === 'side' || settings.s?.appearance.nav === 'icons'" :compact="settings.s?.appearance.nav === 'icons'" />
      <main class="content">
        <Transition name="page" mode="out-in">
          <KeepAlive :max="5">
            <component :is="current" :key="app.page" />
          </KeepAlive>
        </Transition>
      </main>
    </div>
    <div v-else-if="fatal" class="fatal">
      <strong>Zip could not start</strong>
      <span>{{ fatal }}</span>
    </div>
    <Toasts />
    <Transition name="fade">
      <LoginDialog v-if="app.loginOpen" @close="app.loginOpen = false" />
    </Transition>
    <Transition name="fade">
      <CreateInstance v-if="app.createOpen" @close="app.createOpen = false" />
    </Transition>
    <Transition name="fade">
      <ConfirmExit v-if="app.exitOpen" @close="app.exitOpen = false" />
    </Transition>
    <Transition name="fade">
      <SetupWizard v-if="ready && settings.s && !settings.s.setup.completed" />
    </Transition>
    <LegalGate v-if="ready && settings.s?.setup.completed && settings.s.setup.legal !== legal.version" />
    <Transition name="fade">
      <UpdateDialog v-if="app.updateOpen && settings.s?.setup.completed && settings.s.setup.legal === legal.version" @close="app.updateOpen = false" />
    </Transition>
    <Transition name="fade">
      <LegalDialog v-if="app.legalDoc" :doc="app.legalDoc" @close="app.legalDoc = ''" />
    </Transition>
  </div>
</template>

<style scoped>
.window {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg);
  isolation: isolate;
}

.app-bg {
  position: absolute;
  inset: 0;
  z-index: -1;
  overflow: hidden;
  pointer-events: none;
}

.app-bg-image {
  position: absolute;
  background-size: cover;
  background-position: center;
  transition: background-color var(--t-slow);
}

.app-bg-dim {
  position: absolute;
  inset: 0;
  background: #000;
  transition: opacity var(--t-med);
}

.stage {
  flex: 1;
  min-height: 0;
  display: flex;
}

.content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  position: relative;
}

.fatal {
  margin: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: center;
  color: var(--text-dim);
  max-width: 420px;
  text-align: center;
}
</style>
