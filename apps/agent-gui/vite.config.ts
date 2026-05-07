import { fileURLToPath, URL } from "node:url";
import vue from "@vitejs/plugin-vue";
import AutoImport from "unplugin-auto-import/vite";
import Components from "unplugin-vue-components/vite";
import { NaiveUiResolver } from "unplugin-vue-components/resolvers";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [
    vue(),
    AutoImport({
      // Whitelist only — no business stores, per spec §3 Q7.
      imports: [
        "vue",
        "vue-router",
        "pinia",
        "vue-i18n",
        {
          "@vueuse/core": [
            "useDark",
            "useColorMode",
            "useStorage",
            "useEventListener",
            "tryOnScopeDispose",
            "useDebounceFn",
            "useThrottleFn",
            "useIntervalFn",
            "useTimeoutFn",
            "useClipboard",
            "useFocus"
          ]
        }
      ],
      dts: "src/auto-imports.d.ts",
      dirs: [],
      vueTemplate: true
    }),
    Components({
      // Naive UI components are auto-imported on use.
      resolvers: [NaiveUiResolver()],
      // Project SFCs under src/components are also auto-registered for templates.
      dirs: ["src/components"],
      extensions: ["vue"],
      deep: true,
      dts: "src/components.d.ts"
    })
  ],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url))
    }
  },
  clearScreen: false,
  server: { port: 1420, host: "0.0.0.0" }
});
