import js from "@eslint/js";
import globals from "globals";
import pluginVue from "eslint-plugin-vue";
import eslintConfigPrettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";

export default [
  {
    ignores: [
      "**/node_modules/**",
      "**/dist/**",
      "**/coverage/**",
      "target/**",
      "apps/agent-gui/src-tauri/gen/**",
      "apps/agent-gui/src/generated/**"
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
        ...globals.node
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
