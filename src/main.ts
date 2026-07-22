import { createApp } from "vue";
import { createPinia } from "pinia";
import PrimeVue from "primevue/config";
import Tooltip from "primevue/tooltip";
import App from "./App.vue";
import { cerneThemeOptions } from "./theme";
import "./style.css";

const app = createApp(App);
app.use(createPinia());
app.use(PrimeVue, { theme: cerneThemeOptions });
app.directive("tooltip", Tooltip);
app.mount("#app");
