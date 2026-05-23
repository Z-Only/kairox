import { afterEach, beforeEach, vi } from "vitest";
import { enableAutoUnmount } from "@vue/test-utils";
import SessionsSidebar from "./SessionsSidebar.vue";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";

// NOTE: callers must register the matching `vi.mock(...)` declarations at
// the top of their own test file because `vi.mock` is hoisted per-file and
// will not be applied to a test file's transitive imports if it only runs
// here. See SessionsSidebar.*.test.ts for the canonical block.
import { invoke } from "@tauri-apps/api/core";
export const mockedInvoke = vi.mocked(invoke);

type InvokeResponses = Record<string, unknown>;

export function mockInvokeCommandResponses(responses: InvokeResponses = {}) {
  mockedInvoke.mockImplementation((command) => {
    if (command in responses) {
      return Promise.resolve(responses[command]);
    }

    if (command === "list_projects" || command === "list_project_sessions") {
      return Promise.resolve([]);
    }

    if (command === "list_archived_sessions") {
      return Promise.resolve([]);
    }

    if (command === "switch_session") {
      return Promise.resolve({
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false,
        last_context_usage: null,
        model_limits: null,
        compaction: { type: "Idle" }
      });
    }

    if (command === "get_profile_info" || command === "list_profiles" || command === "get_trace") {
      return Promise.resolve([]);
    }

    return Promise.resolve(null);
  });
}

// `mountWithPlugins({ initialRoute })` wires Pinia + i18n + the production
// router so the Sidebar's dependencies resolve cleanly.
export async function mountSidebar() {
  const hostElement = document.createElement("div");
  document.body.appendChild(hostElement);
  const { wrapper, router } = mountWithPlugins(SessionsSidebar, {
    initialRoute: "/workbench",
    mount: {
      attachTo: hostElement,
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: vi.fn().mockResolvedValue(true) }
        },
        stubs: {
          Teleport: true
        }
      }
    }
  });
  await router.isReady();
  return { wrapper, router };
}

// Registers the file-wide hooks that mirror the original SessionsSidebar
// test bootstrap: auto-unmount, body cleanup, and per-test mock reset.
export function installSidebarTestEnv() {
  enableAutoUnmount(afterEach);

  afterEach(() => {
    document.body.innerHTML = "";
  });

  beforeEach(() => {
    // `mountWithPlugins` activates a fresh Pinia internally; we just reset
    // mocks here so per-test invoke / store state stays isolated.
    vi.clearAllMocks();
    mockInvokeCommandResponses();
  });
}
