import { fileURLToPath, URL } from "node:url";
import { createLogger, defineConfig } from "vite";
import { createKairoxVitePlugins } from "./build/vitePlugins";

const logger = createLogger();
const defaultWarn = logger.warn;
logger.warn = (message, options) => {
  if (
    message.includes("[INVALID_ANNOTATION]") &&
    message.includes("@vueuse/core/dist/index.js") &&
    message.includes("#__PURE__")
  ) {
    return;
  }
  defaultWarn.call(logger, message, options);
};

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
  customLogger: logger,
  clearScreen: false,
  server: { port: 1420, host: "0.0.0.0", strictPort: true }
});
