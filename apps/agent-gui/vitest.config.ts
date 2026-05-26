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
      // Thresholds are organised by risk tier:
      //   T1 (utils)        — pure functions, easy to test exhaustively
      //   T2 (stores, composables) — Pinia stores + reusable business logic;
      //                       user-action aggregation points
      //   T3 (components, views)   — UI presentation layer
      // Aggregate (no glob) is the workspace floor.
      thresholds: {
        statements: 80,
        branches: 72,
        functions: 76,
        lines: 80,
        // T3
        "src/components/**/*.{ts,vue}": {
          statements: 78,
          branches: 72,
          functions: 74,
          lines: 78
        },
        // T3
        "src/views/**/*.vue": {
          statements: 90,
          branches: 82,
          functions: 78,
          lines: 90
        },
        // T2 — stores branches lifted from the 67.08% pre-test baseline to
        // ~70% with new workspaceUi/agentSettings/modelProfiles/catalog tests.
        // Gate at 68 (~1.7pp buffer) because vitest V8 wobbles by ~0.3pp
        // between runs; plan target 70 stays the next follow-up.
        "src/stores/**/*.ts": {
          statements: 80,
          branches: 68,
          functions: 80,
          lines: 82
        },
        // T2
        "src/composables/**/*.ts": {
          statements: 74,
          branches: 60,
          functions: 78,
          lines: 74
        },
        // T1
        "src/utils/**/*.ts": {
          statements: 92,
          branches: 90,
          functions: 95,
          lines: 92
        }
      }
    }
  }
});
