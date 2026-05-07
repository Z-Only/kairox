import js from "@eslint/js";
import globals from "globals";
import pluginVue from "eslint-plugin-vue";
import eslintConfigPrettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const autoImportGlobals = (() => {
  const path = resolve(
    __dirname,
    "apps/agent-gui/.eslintrc-auto-import.json"
  );
  if (!existsSync(path)) return {};
  try {
    return JSON.parse(readFileSync(path, "utf8")).globals ?? {};
  } catch {
    return {};
  }
})();

export default [
  {
    ignores: [
      "**/node_modules/**",
      "**/dist/**",
      "**/coverage/**",
      "target/**",
      "apps/agent-gui/src-tauri/gen/**",
      "apps/agent-gui/src/generated/**",
      "crates/agent-mcp/tests/fixtures/**"
    ]
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  {
    files: ["apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: {
        ...globals.browser,
        ...globals.node,
        ...autoImportGlobals
      },
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: [".vue"]
      }
    },
    rules: {
      "vue/multi-word-component-names": "off"
    }
  },
  {
    files: ["scripts/**/*.cjs"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "commonjs",
      globals: {
        ...globals.node
      }
    },
    rules: {
      "@typescript-eslint/no-require-imports": "off"
    }
  },
  // E2E test files: relax rules for Playwright specs and browser-side mock
  {
    files: ["apps/agent-gui/e2e/**/*.{ts,js}"],
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": "off",
      "@typescript-eslint/ban-ts-comment": "off",
      "no-redeclare": "off",
      "no-useless-assignment": "off"
    }
  },
  eslintConfigPrettier
];
