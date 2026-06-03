import { fileURLToPath, URL } from "node:url";
import { createLogger, defineConfig } from "vite";
import { createKairoxVitePlugins } from "./build/vitePlugins";
import { DEFAULT_DEV_PORT, parsePort, shouldUseStrictPort } from "./scripts/dev-port.mjs";

const logger = createLogger();
const defaultWarn = logger.warn;
const devServerPort = parsePort(process.env.KAIROX_DEV_PORT ?? process.env.PORT, DEFAULT_DEV_PORT);
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
  server: { port: devServerPort, host: "0.0.0.0", strictPort: shouldUseStrictPort() }
});
