import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import { createKairoxVitePlugins } from "./build/vitePlugins";

export default defineConfig({
  plugins: createKairoxVitePlugins(),
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url))
    }
  },
  build: {
    sourcemap: false
  },
  clearScreen: false,
  server: { port: 1420, host: "0.0.0.0", strictPort: true }
});
