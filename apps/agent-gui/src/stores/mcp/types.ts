import type { McpServerStatusResponse } from "@/generated/commands";

export interface McpServerEntry extends McpServerStatusResponse {
  error?: string;
}

export type RefreshInstalledOptions = { forceTools?: boolean };
