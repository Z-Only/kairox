import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { router } from "./router";
import { i18n, bindLocaleToStore } from "./locales";
import "./assets/main.css";
import "highlight.js/styles/github-dark.css";

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.use(i18n);
bindLocaleToStore();
app.mount("#app");
