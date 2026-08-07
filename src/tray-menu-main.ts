import { createApp } from "vue";
import TrayMenuApp from "./TrayMenuApp.vue";
import { i18n } from "./locales";
import { installTooltips } from "./utils/tooltips";
import "./styles/main.css";

document.addEventListener("contextmenu", (e) => e.preventDefault());
installTooltips();

const app = createApp(TrayMenuApp);
app.use(i18n);
app.mount("#app");
