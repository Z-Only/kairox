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
      // Baseline: 2026-06-01 — floor(actual - 1), only raised.
      thresholds: {
        statements: 89,
        branches: 82,
        functions: 88,
        lines: 90,
        // T3 — components aggregate: stmts 85.89, br 80.22, fn 87.02, ln 86.37
        "src/components/**/*.{ts,vue}": {
          statements: 84,
          branches: 79,
          functions: 86,
          lines: 85
        },
        // T3 — views aggregate: stmts 95.23, br 84.00, fn 93.75, ln 95.00
        "src/views/**/*.vue": {
          statements: 90,
          branches: 83,
          functions: 78,
          lines: 90
        },
        // T2 — stores aggregate: stmts 95.06, br 85.39, fn 95.18, ln 96.44
        "src/stores/**/*.ts": {
          statements: 94,
          branches: 84,
          functions: 94,
          lines: 95
        },
        // T2 — composables aggregate: stmts 92.40, br 86.45, fn 89.26, ln 94.11
        "src/composables/**/*.ts": {
          statements: 91,
          branches: 85,
          functions: 88,
          lines: 93
        },
        // T1 — utils aggregate: stmts 95.83, br 100, fn 100, ln 95.83
        "src/utils/**/*.ts": {
          statements: 94,
          branches: 99,
          functions: 99,
          lines: 94
        }
      }
    }
  }
});
