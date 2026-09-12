import { createApp } from "vue";
import { createPinia } from "pinia";
import PrimeVue from "primevue/config";
import Tooltip from "primevue/tooltip";
import App from "./App.vue";
import { cerneThemeOptions } from "./theme";
import { i18n } from "./i18n";
import { applyFontSettings } from "./fontSettings";
// Fontes embutidas (variáveis, empacotadas pelo Vite — funciona offline).
// `wght.css` das serifadas: só o eixo de peso; subconjuntos não-latinos têm
// unicode-range, então o navegador nunca baixa grego/cirílico para PT-BR.
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@fontsource-variable/literata/wght.css";
import "@fontsource-variable/source-serif-4/wght.css";
import "./style.css";

document.documentElement.setAttribute("lang", i18n.global.locale.value);
applyFontSettings();

const app = createApp(App);
app.use(createPinia());
app.use(PrimeVue, { theme: cerneThemeOptions });
app.use(i18n);
app.directive("tooltip", Tooltip);
app.mount("#app");
