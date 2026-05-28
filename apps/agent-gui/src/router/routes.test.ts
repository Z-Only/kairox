import { describe, it, expect, vi, beforeEach } from "vitest";
import { createRouter, createMemoryHistory, type Router } from "vue-router";
import { routes } from "./routes";

// Stub Tauri APIs that lazily-loaded components may pull in.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

let router: Router;

beforeEach(async () => {
  router = createRouter({ history: createMemoryHistory(), routes });
});

/** Navigate and return the resolved route. */
async function navigateTo(path: string) {
  await router.push(path);
  await router.isReady();
  return router.currentRoute.value;
}

describe("routes", () => {
  // --- Top-level redirects ---

  it("redirects / to the workbench route", async () => {
    const route = await navigateTo("/");
    expect(route.name).toBe("workbench");
    expect(route.path).toBe("/workbench");
  });

  it("redirects unknown paths to workbench (catch-all)", async () => {
    const route = await navigateTo("/does-not-exist");
    expect(route.name).toBe("workbench");
  });

  it("redirects deeply nested unknown paths to workbench", async () => {
    const route = await navigateTo("/foo/bar/baz");
    expect(route.name).toBe("workbench");
  });

  // --- Legacy redirect ---

  it("redirects /marketplace to /settings", async () => {
    const route = await navigateTo("/marketplace");
    // /settings itself redirects to settings-general
    expect(route.name).toBe("settings-general");
    expect(route.path).toBe("/settings/general");
  });

  // --- Workbench ---

  it("resolves /workbench to the workbench route", async () => {
    const route = await navigateTo("/workbench");
    expect(route.name).toBe("workbench");
    expect(route.matched.length).toBeGreaterThan(0);
  });

  it("resolves /workbench/:sessionId with a session param", async () => {
    const route = await navigateTo("/workbench/abc-123");
    expect(route.name).toBe("workbench");
    expect(route.params.sessionId).toBe("abc-123");
  });

  it("treats sessionId as optional — /workbench works without it", async () => {
    const route = await navigateTo("/workbench");
    expect(route.name).toBe("workbench");
    // Optional param is undefined when not provided
    expect(route.params.sessionId).toBeUndefined();
  });

  it("marks the workbench route with props: true", () => {
    const workbenchRoute = routes.find((r) => r.name === "workbench");
    expect(workbenchRoute).toBeDefined();
    expect(workbenchRoute!.props).toBe(true);
  });

  // --- Workbench lazy loading ---

  it("lazy-loads WorkbenchView component", () => {
    const workbenchRoute = routes.find((r) => r.name === "workbench");
    expect(workbenchRoute).toBeDefined();
    // Lazy-loaded components are functions (dynamic imports)
    expect(typeof workbenchRoute!.component).toBe("function");
  });

  // --- Settings ---

  it("redirects /settings to settings-general", async () => {
    const route = await navigateTo("/settings");
    expect(route.name).toBe("settings-general");
    expect(route.path).toBe("/settings/general");
  });

  const SETTINGS_CHILDREN = [
    { path: "/settings/general", name: "settings-general" },
    { path: "/settings/mcp", name: "settings-mcp" },
    { path: "/settings/skills", name: "settings-skills" },
    { path: "/settings/plugins", name: "settings-plugins" },
    { path: "/settings/agents", name: "settings-agents" },
    { path: "/settings/models", name: "settings-models" },
    { path: "/settings/instructions", name: "settings-instructions" },
    { path: "/settings/hooks", name: "settings-hooks" },
    { path: "/settings/archive", name: "settings-archive" }
  ] as const;

  it.each(SETTINGS_CHILDREN)("resolves $path to route named $name", async ({ path, name }) => {
    const route = await navigateTo(path);
    expect(route.name).toBe(name);
  });

  // --- Lazy-loaded settings children ---

  it("eagerly imports GeneralSettings (not lazy)", () => {
    const settingsRoute = routes.find((r) => r.path === "/settings");
    const general = settingsRoute?.children?.find((c) => c.name === "settings-general");
    expect(general).toBeDefined();
    // Eagerly imported components are objects, not functions
    expect(typeof general!.component).not.toBe("function");
  });

  const LAZY_SETTINGS = [
    "settings-mcp",
    "settings-skills",
    "settings-plugins",
    "settings-agents",
    "settings-models",
    "settings-instructions",
    "settings-hooks",
    "settings-archive"
  ] as const;

  it.each(LAZY_SETTINGS)("lazy-loads the %s settings pane", (name) => {
    const settingsRoute = routes.find((r) => r.path === "/settings");
    const child = settingsRoute?.children?.find((c) => c.name === name);
    expect(child).toBeDefined();
    expect(typeof child!.component).toBe("function");
  });

  // --- Route count ---

  it("defines the expected number of settings children", () => {
    const settingsRoute = routes.find((r) => r.path === "/settings");
    expect(settingsRoute?.children).toHaveLength(SETTINGS_CHILDREN.length + 1); // +1 for the "" redirect
  });
});
