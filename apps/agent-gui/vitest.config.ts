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
      // GitHub CI baseline 2026-06-11 (run 27352470387): all files stmts
      // 91.53, br 85.09, fn 91.60, ln 92.76; global floors remain tight.
      thresholds: {
        statements: 91,
        branches: 85,
        functions: 91,
        lines: 92,
        // T3 — components recursive aggregate: stmts 89.70, br 84.87,
        // fn 90.18, ln 90.78.
        "src/components/**/*.{ts,vue}": {
          statements: 89,
          branches: 84,
          functions: 88,
          lines: 90
        },
        // T3 — views recursive aggregate: stmts 95.83, br 90.53,
        // fn 92.31, ln 95.58; raise branches 83 → 89.
        "src/views/**/*.vue": {
          statements: 94,
          branches: 89,
          functions: 92,
          lines: 94
        },
        // T2 — stores recursive aggregate: stmts 94.20, br 84.73,
        // fn 95.76, ln 95.68.
        "src/stores/**/*.ts": {
          statements: 94,
          branches: 84,
          functions: 94,
          lines: 95
        },
        // T2 — composables recursive aggregate: stmts 94.18, br 86.52,
        // fn 95.72, ln 95.68; tighten floors where current headroom is real.
        "src/composables/**/*.ts": {
          statements: 92,
          branches: 85,
          functions: 94,
          lines: 94
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
