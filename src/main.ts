import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./styles/main.css";

// Disable WebView native context menu (Inspect / Reload, etc.)
document.addEventListener("contextmenu", (e) => e.preventDefault());

const app = createApp(App);
app.use(createPinia());
app.mount("#app");
