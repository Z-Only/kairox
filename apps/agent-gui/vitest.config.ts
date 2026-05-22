import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";
import { createKairoxVitePlugins } from "./build/vitePlugins";

export default defineConfig({
  plugins: createKairoxVitePlugins(),
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
      reporter: ["text", "lcov"],
      reportsDirectory: "coverage",
      all: true,
      include: ["src/**/*.{ts,vue}"],
      exclude: [
        "src/**/*.d.ts",
        "src/**/*.{test,spec}.{ts,tsx}",
        "src/App.vue",
        "src/main.ts",
        "src/generated/**",
        "src/layouts/**",
        "src/locales/**",
        "src/router/**",
        "src/test-utils/**",
        "src/types/**",
        "src/env.d.ts"
      ],
      thresholds: {
        statements: 79,
        branches: 72,
        functions: 76,
        lines: 80,
        "src/components/**/*.{ts,vue}": {
          statements: 78,
          branches: 72,
          functions: 74,
          lines: 78
        },
        "src/composables/**/*.ts": {
          statements: 68,
          branches: 54,
          functions: 76,
          lines: 70
        },
        "src/stores/**/*.ts": {
          statements: 78,
          branches: 64,
          functions: 78,
          lines: 80
        },
        "src/utils/**/*.ts": {
          statements: 92,
          branches: 90,
          functions: 95,
          lines: 92
        },
        "src/views/**/*.vue": {
          statements: 90,
          branches: 82,
          functions: 78,
          lines: 90
        }
      }
    }
  }
});
