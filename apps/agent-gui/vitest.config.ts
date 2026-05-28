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
      // Baseline: 2026-05-28 — floor(actual - 1), only raised.
      thresholds: {
        statements: 85,
        branches: 80,
        functions: 86,
        lines: 86,
        // T3 — components aggregate: stmts 87.71, br 82.84, fn 88.31, ln 88.59
        "src/components/**/*.{ts,vue}": {
          statements: 86,
          branches: 81,
          functions: 87,
          lines: 87
        },
        // T3 — views aggregate: stmts 91.46, br 86.20, fn 79.16, ln 90.78
        "src/views/**/*.vue": {
          statements: 90,
          branches: 85,
          functions: 78,
          lines: 90
        },
        // T2 — stores aggregate: stmts 85.17, br 76.32, fn 85.15, ln 86.66
        "src/stores/**/*.ts": {
          statements: 84,
          branches: 75,
          functions: 84,
          lines: 85
        },
        // T2 — composables aggregate: stmts 83.66, br 76.11, fn 85.45, ln 85.00
        "src/composables/**/*.ts": {
          statements: 82,
          branches: 75,
          functions: 84,
          lines: 84
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
