import { createApp } from "vue";
import { createPinia } from "pinia";
import PrimeVue from "primevue/config";
import Tooltip from "primevue/tooltip";
import App from "./App.vue";
import { cerneThemeOptions } from "./theme";
import { i18n } from "./i18n";
import "./style.css";

document.documentElement.setAttribute("lang", i18n.global.locale.value);

const app = createApp(App);
app.use(createPinia());
app.use(PrimeVue, { theme: cerneThemeOptions });
app.use(i18n);
app.directive("tooltip", Tooltip);
app.mount("#app");
