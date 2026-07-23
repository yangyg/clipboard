import { createApp } from "vue";
import TrayMenuApp from "./TrayMenuApp.vue";
import "./styles/main.css";

document.addEventListener("contextmenu", (e) => e.preventDefault());

createApp(TrayMenuApp).mount("#app");
