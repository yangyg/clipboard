import App from "./App.vue";
import { mountApp } from "./bootstrap";
import "./styles/settings.css";

mountApp(App, { pinia: true });
