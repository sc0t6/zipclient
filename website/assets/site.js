const header = document.querySelector("header.site");
const onScroll = () => header.classList.toggle("scrolled", window.scrollY > 8);
window.addEventListener("scroll", onScroll, { passive: true });
onScroll();

const toggle = document.querySelector(".menu-toggle");
const nav = document.getElementById("nav");
if (toggle && nav) {
  toggle.addEventListener("click", () => {
    const open = nav.classList.toggle("open");
    toggle.setAttribute("aria-expanded", String(open));
  });
  nav.addEventListener("click", (event) => {
    if (event.target.closest("a")) {
      nav.classList.remove("open");
      toggle.setAttribute("aria-expanded", "false");
    }
  });
}

const captions = {
  home: ["Zip home screen", "Your latest version, recent instance, accounts and news at a glance."],
  mods: ["Zip mod browser", "Search Modrinth and CurseForge and install with dependencies in one click."],
  create: ["Creating a new instance in Zip", "Pick any version and loader, with Optimized Mode and low-memory warnings."],
  skins: ["Zip skin changer", "A 3D preview, a skin library and one-click uploads to your profile."],
  themes: ["Zip appearance settings", "Six themes, any accent colour and UI scaling."],
};
const image = document.getElementById("gallery-img");
const caption = document.getElementById("gallery-cap");
const tabs = [...document.querySelectorAll("[data-shot]")];
const select = (tab) => {
  const key = tab.dataset.shot;
  for (const t of tabs) t.setAttribute("aria-selected", String(t === tab));
  image.src = `assets/screens/${key}.png`;
  image.alt = captions[key][0];
  caption.textContent = captions[key][1];
};
for (const tab of tabs) {
  tab.addEventListener("click", () => select(tab));
  tab.addEventListener("keydown", (event) => {
    const i = tabs.indexOf(tab);
    const next = event.key === "ArrowRight" ? tabs[(i + 1) % tabs.length] : event.key === "ArrowLeft" ? tabs[(i - 1 + tabs.length) % tabs.length] : null;
    if (next) {
      next.focus();
      select(next);
    }
  });
}
