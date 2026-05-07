import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import AutoImport from "unplugin-auto-import/vite";
import Components from "unplugin-vue-components/vite";
import { NaiveUiResolver } from "unplugin-vue-components/resolvers";

// Vitest must run through the same `unplugin-auto-import` /
// `unplugin-vue-components` pipeline as Vite. Otherwise the bulk import
// cleanup done in Task 9 (where `defineStore`, `ref`, `computed`,
// `useStorage`, `useI18n`, NaiveUI components, etc. are no longer
// explicitly imported) would resolve in `pnpm dev` / `pnpm build` but
// fail at runtime under vitest's jsdom environment with
// "ReferenceError: defineStore is not defined".
//
// Plugin config MUST stay in sync with `vite.config.ts`. Whitelisted
// `@vueuse/core` hooks here mirror the production list verbatim — any
// addition there should be mirrored here in the same commit.
export default defineConfig({
  plugins: [
    vue(),
    AutoImport({
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
      eslintrc: {
        enabled: true,
        filepath: "./.eslintrc-auto-import.json",
        globalsPropValue: true
      },
      dirs: [],
      vueTemplate: true
    }),
    Components({
      resolvers: [NaiveUiResolver()],
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
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,vue}"],
      exclude: ["src/generated/**", "src/env.d.ts"]
    }
  }
});
