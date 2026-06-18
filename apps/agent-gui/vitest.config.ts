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
      // GitHub CI baseline 2026-06-18 (run 27731357539): all files stmts
      // 92.21, br 85.59, fn 92.13, ln 93.50; raise floors to prevent
      // coverage regression.
      thresholds: {
        statements: 92,
        branches: 85,
        functions: 92,
        lines: 93,
        // T3 — components recursive aggregate.
        "src/components/**/*.{ts,vue}": {
          // CI baseline 2026-06-18: stmts 90.75, br 85.43, fn 90.83, ln 91.94.
          statements: 90,
          branches: 85,
          functions: 90,
          lines: 91
        },
        // T3 — views recursive aggregate. CI baseline 2026-06-18:
        // stmts 95.83, br 90.53, fn 92.31, ln 95.58.
        "src/views/**/*.vue": {
          statements: 95,
          branches: 90,
          functions: 92,
          lines: 95
        },
        // T2 — stores recursive aggregate.
        "src/stores/**/*.ts": {
          statements: 94,
          // CI baseline 2026-06-18: stmts 94.47, br 85.28, fn 96.01, ln 95.91.
          branches: 85,
          functions: 95,
          lines: 95
        },
        // T2 — composables recursive aggregate.
        "src/composables/**/*.ts": {
          // CI baseline 2026-06-18: stmts 94.20, br 86.19, fn 95.77, ln 95.65.
          statements: 94,
          branches: 86,
          functions: 95,
          lines: 95
        },
        // T1 — utils aggregate: stmts 95.83, br 100, fn 100, ln 95.83
        "src/utils/**/*.ts": {
          statements: 95,
          branches: 100,
          functions: 100,
          lines: 95
        }
      }
    }
  }
});
