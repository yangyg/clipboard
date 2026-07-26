import { createApp } from "vue";
import TrayMenuApp from "./TrayMenuApp.vue";
import { i18n } from "./locales";
import "./styles/main.css";

document.addEventListener("contextmenu", (e) => e.preventDefault());

const app = createApp(TrayMenuApp);
app.use(i18n);
app.mount("#app");
