import vue from "@vitejs/plugin-vue";
import AutoImport from "unplugin-auto-import/vite";
import Components from "unplugin-vue-components/vite";

const vueUseImports = [
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
];

export function createKairoxVitePlugins() {
  return [
    vue(),
    AutoImport({
      imports: [
        "vue",
        "vue-router",
        "pinia",
        "vue-i18n",
        {
          "@vueuse/core": vueUseImports
        }
      ],
      dts: "src/auto-imports.d.ts",
      dirs: [],
      vueTemplate: true
    }),
    Components({
      dirs: ["src/components"],
      extensions: ["vue"],
      deep: true,
      dts: "src/components.d.ts"
    })
  ];
}
