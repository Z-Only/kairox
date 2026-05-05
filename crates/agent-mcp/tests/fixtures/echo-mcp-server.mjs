#!/usr/bin/env node

/**
 * Minimal MCP stdio server for integration tests.
 *
 * Implements:
 * - initialize  → { name: "echo-test-server", version: "1.0.0" } with tools/resources/prompts caps
 * - tools/list  → echo + env tools
 * - tools/call  → echo returns message, env returns an env var
 * - resources/list → test://echo resource
 * - resources/read → content for test://echo
 * - prompts/list  → test-prompt with topic argument
 * - prompts/get   → message about the topic
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const server = new McpServer({
  name: "echo-test-server",
  version: "1.0.0",
});

// -- Tools -----------------------------------------------------------------

server.tool("echo", "Echoes back the input", { message: z.string() }, async ({ message }) => {
  return {
    content: [{ type: "text", text: message }],
    isError: false,
  };
});

server.tool("env", "Returns an environment variable", { name: z.string() }, async ({ name }) => {
  const value = process.env[name] ?? "";
  return {
    content: [{ type: "text", text: value }],
    isError: false,
  };
});

// -- Resources -------------------------------------------------------------

server.resource("Echo Resource", "test://echo", { description: "A test resource", mimeType: "text/plain" }, async (uri) => {
  return {
    contents: [
      {
        uri: uri.href,
        mimeType: "text/plain",
        text: "Content of test://echo",
      },
    ],
  };
});

// -- Prompts ---------------------------------------------------------------

server.prompt("test-prompt", "A test prompt", { topic: z.string().describe("The topic") }, async ({ topic }) => {
  return {
    messages: [
      {
        role: "user",
        content: { type: "text", text: `Tell me about ${topic}` },
      },
    ],
  };
});

// -- Start -----------------------------------------------------------------

const transport = new StdioServerTransport();
await server.connect(transport);
