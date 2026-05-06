import type { GlobalThemeOverrides } from "naive-ui";

/**
 * NaiveUI theme overrides derived from the palette currently shipped in
 * `apps/agent-gui/src/assets/main.css` + `App.vue` fallbacks:
 *   accent      = #334455 (App.vue --accent fallback "#345")
 *   border      = #cccccc (App.vue --border fallback "#ccc")
 *   surface-alt = #f7f7f7 (App.vue --surface-alt fallback)
 *   body fg/bg  = #333 / #fff (main.css)
 *
 * Hover/pressed tones are 12% lighter / 12% darker than primary, matching
 * NaiveUI's default contrast convention.
 */
export const lightThemeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#334455",
    primaryColorHover: "#4d6273",
    primaryColorPressed: "#1f2c38",
    primaryColorSuppl: "#334455",
    borderColor: "#cccccc",
    dividerColor: "#d7d7d7",
    bodyColor: "#ffffff",
    cardColor: "#ffffff",
    modalColor: "#ffffff",
    popoverColor: "#ffffff",
    tableColor: "#ffffff",
    hoverColor: "#f7f7f7",
    textColorBase: "#333333",
    textColor1: "#333333",
    textColor2: "#555555",
    textColor3: "#888888"
  },
  Card: { paddingSmall: "12px", paddingMedium: "16px" },
  Button: { borderRadiusMedium: "4px" },
  Menu: { itemHeight: "32px" }
};

/**
 * Dark palette mirrors the light one with inverted lightness:
 *   accent      = #6688aa (lightened brand)
 *   border/divider = #3a3f47 (matches existing dark surface tokens elsewhere)
 *   body bg     = #1a1d22 (sits below cardColor for contrast)
 *   card bg     = #22262c
 *   text        = #e6e8eb (matches WCAG AA against #1a1d22)
 */
export const darkThemeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#6688aa",
    primaryColorHover: "#809fbe",
    primaryColorPressed: "#4d6f91",
    primaryColorSuppl: "#6688aa",
    borderColor: "#3a3f47",
    dividerColor: "#3a3f47",
    bodyColor: "#1a1d22",
    cardColor: "#22262c",
    modalColor: "#22262c",
    popoverColor: "#22262c",
    tableColor: "#22262c",
    hoverColor: "#2a2f36",
    textColorBase: "#e6e8eb",
    textColor1: "#e6e8eb",
    textColor2: "#c0c4c9",
    textColor3: "#8b9098"
  },
  Card: { paddingSmall: "12px", paddingMedium: "16px" },
  Button: { borderRadiusMedium: "4px" },
  Menu: { itemHeight: "32px" }
};
