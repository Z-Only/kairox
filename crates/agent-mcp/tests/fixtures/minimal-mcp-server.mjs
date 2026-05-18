#!/usr/bin/env node

/**
 * Dependency-free MCP stdio fixture for connectivity tests.
 *
 * The Rust stdio transport uses newline-delimited JSON-RPC, so this fixture
 * responds with one JSON object per stdout line.
 */

import readline from "node:readline";

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity
});

function send(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

for await (const line of rl) {
  if (!line.trim()) {
    continue;
  }

  const request = JSON.parse(line);

  if (request.method === "initialize") {
    send({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: request.params?.protocolVersion ?? "2025-06-18",
        capabilities: { tools: {} },
        serverInfo: {
          name: "minimal-mcp-server",
          version: "1.0.0"
        }
      }
    });
    continue;
  }

  if (request.method === "notifications/initialized") {
    continue;
  }

  if (request.method === "tools/list") {
    send({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        tools: [
          {
            name: "echo",
            description: "Echoes a message",
            inputSchema: {
              type: "object",
              properties: {
                message: { type: "string" }
              },
              required: ["message"]
            }
          }
        ]
      }
    });
    continue;
  }

  send({
    jsonrpc: "2.0",
    id: request.id,
    result: {}
  });
}
