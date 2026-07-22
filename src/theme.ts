import { definePreset } from "@primeuix/themes";
import Aura from "@primeuix/themes/aura";

// White surface, 1px hairline borders, medium body weight — matches the
// reference screenshots (flat panels, thin dividers, no heavy shadows).
export const CernePreset = definePreset(Aura, {
  semantic: {
    primary: {
      50: "{neutral.50}",
      100: "{neutral.100}",
      200: "{neutral.200}",
      300: "{neutral.300}",
      400: "#8a8f98",
      500: "#3f3f46",
      600: "#27272a",
      700: "#18181b",
      800: "#111113",
      900: "#0a0a0b",
      950: "#050505",
    },
    colorScheme: {
      light: {
        surface: {
          0: "#ffffff",
          50: "#fafafa",
          100: "#f4f4f5",
          200: "#e4e4e7",
          300: "#d4d4d8",
          400: "#a1a1aa",
          500: "#71717a",
          600: "#52525b",
          700: "#3f3f46",
          800: "#27272a",
          900: "#18181b",
          950: "#09090b",
        },
        formField: {
          background: "#ffffff",
          borderColor: "#e4e4e7",
          hoverBorderColor: "#d4d4d8",
          focusBorderColor: "#27272a",
          color: "#18181b",
        },
        content: {
          background: "#ffffff",
          borderColor: "#e4e4e7",
        },
      },
    },
  },
  components: {
    button: {
      root: {
        borderRadius: "8px",
      },
    },
  },
});

export const cerneThemeOptions = {
  preset: CernePreset,
  options: {
    darkModeSelector: ".cerne-dark",
    cssLayer: {
      name: "primevue",
      order: "reset, primevue",
    },
  },
};
