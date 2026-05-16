// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). This composable is plain `.ts`, so the
// `@vueuse/core` lifecycle helper must be imported explicitly.
import { tryOnScopeDispose } from "@vueuse/core";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent } from "@/types";
import { useSessionStore } from "@/stores/session";
import { applyTraceEvent } from "@/composables/useTraceStore";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { useUiStore } from "@/stores/ui";
import { useMcpStore } from "@/stores/mcp";
import { useAgentsStore } from "@/stores/agents";
import { useCatalogStore } from "@/stores/catalog";

export function useTauriEvents() {
  const session = useSessionStore();
  const taskGraph = useTaskGraphStore();
  const ui = useUiStore();
  const mcp = useMcpStore();
  const agents = useAgentsStore();
  const catalog = useCatalogStore();

  // Capture the unlisten promise synchronously so tryOnScopeDispose
  // can register cleanup before any await boundary — calling it after
  // await would lose the effect scope and silently no-op.
  const unlistenPromise = listen<DomainEvent>("session-event", (tauriEvent) => {
    // Only process session-scoped events for the current session.
    const domainEvent = tauriEvent.payload;
    const sessionId: string | undefined = domainEvent.session_id;
    if (sessionId && session.currentSessionId && sessionId === session.currentSessionId) {
      session.applyEvent(domainEvent);
      applyTraceEvent(domainEvent);

      // Delegate task-graph mutations to the owning store. Mirrors the
      // existing `agents.applyAgentEvent` / `mcp.handleMcpEvent` pattern.
      taskGraph.applyTaskEvent(domainEvent.payload);

      // Surface task-failure errors as user-facing notifications.
      if (domainEvent.payload.type === "AgentTaskFailed" && domainEvent.payload.error) {
        ui.pushNotification("error", domainEvent.payload.error);
      }

      // Route agent lifecycle events to the agents store.
      agents.applyAgentEvent(domainEvent.payload);
    }

    // MCP and catalog source events are global, not session-scoped.
    const payload = domainEvent.payload;
    switch (payload.type) {
      case "McpServerStarting":
      case "McpServerReady":
      case "McpServerStopped":
      case "McpServerFailed":
      case "McpToolCallStarted":
      case "McpToolCallCompleted":
      case "McpTrustGranted":
      case "McpTrustRevoked":
        mcp.handleMcpEvent(payload);
        break;
      case "CatalogSourceAdded":
        void catalog.fetchSources();
        break;
      case "CatalogSourceFailed":
        catalog.handleSourceFailed(payload.source, payload.error);
        break;
      case "CatalogSourceResultsArrived":
        catalog.mergeSourceResults(payload.source, payload.entries);
        break;
    }
  });

  // SYNCHRONOUS: must be called before any await so the current effect scope
  // (captured by getCurrentScope()) is the one from setup().
  tryOnScopeDispose(() => {
    void unlistenPromise.then((u) => u()).catch(() => {});
    session.setConnected(false);
  });

  void unlistenPromise
    .then(() => session.setConnected(true))
    .catch((err) => {
      ui.pushNotification("error", `Failed to subscribe to session events: ${err}`);
    });
}
