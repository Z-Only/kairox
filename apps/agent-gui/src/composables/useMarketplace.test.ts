import { describe, it, expect } from "vitest";
import type { ServerEntryResponse } from "../generated/commands";
import { parseRequirements, parseDefaultEnv, parseInstallHeaders } from "./useMarketplace";

/** Builds a minimal `ServerEntryResponse` with the given JSON fields. */
function makeEntry(
  overrides: Partial<
    Pick<ServerEntryResponse, "requirements_json" | "default_env_json" | "install_spec_json">
  > = {}
): ServerEntryResponse {
  return {
    id: "test-server",
    source: "builtin",
    display_name: "Test Server",
    summary: "A test server",
    description: "",
    categories: [],
    tags: [],
    author: null,
    homepage: null,
    version: null,
    trust: "unverified",
    verified: false,
    icon: null,
    install_spec_json: overrides.install_spec_json ?? "{}",
    requirements_json: overrides.requirements_json ?? "[]",
    default_env_json: overrides.default_env_json ?? "[]"
  };
}

// ------- parseRequirements -------
describe("parseRequirements", () => {
  it("returns parsed requirements from valid JSON", () => {
    const entry = makeEntry({
      requirements_json: JSON.stringify([
        { kind: "binary", min_version: "1.0.0", install_hint: "brew install foo" },
        { kind: "runtime", min_version: null, install_hint: null }
      ])
    });
    const result = parseRequirements(entry);
    expect(result).toHaveLength(2);
    expect(result[0]).toEqual({
      kind: "binary",
      min_version: "1.0.0",
      install_hint: "brew install foo"
    });
    expect(result[1]).toEqual({ kind: "runtime", min_version: null, install_hint: null });
  });

  it("returns empty array for empty JSON array", () => {
    expect(parseRequirements(makeEntry({ requirements_json: "[]" }))).toEqual([]);
  });

  it("returns empty array when JSON is not an array", () => {
    expect(parseRequirements(makeEntry({ requirements_json: '{"not":"array"}' }))).toEqual([]);
  });

  it("returns empty array for invalid JSON", () => {
    expect(parseRequirements(makeEntry({ requirements_json: "not json" }))).toEqual([]);
  });

  it("returns empty array for empty string", () => {
    expect(parseRequirements(makeEntry({ requirements_json: "" }))).toEqual([]);
  });
});

// ------- parseDefaultEnv -------
describe("parseDefaultEnv", () => {
  it("returns parsed env vars from valid JSON", () => {
    const envVars = [
      {
        key: "API_KEY",
        label: "API Key",
        description: "Your API key",
        required: true,
        secret: true,
        default: null
      },
      {
        key: "BASE_URL",
        label: "Base URL",
        description: "Server URL",
        required: false,
        secret: false,
        default: "https://example.com"
      }
    ];
    const entry = makeEntry({ default_env_json: JSON.stringify(envVars) });
    const result = parseDefaultEnv(entry);
    expect(result).toHaveLength(2);
    expect(result[0].key).toBe("API_KEY");
    expect(result[0].required).toBe(true);
    expect(result[0].secret).toBe(true);
    expect(result[1].default).toBe("https://example.com");
  });

  it("returns empty array for empty JSON array", () => {
    expect(parseDefaultEnv(makeEntry({ default_env_json: "[]" }))).toEqual([]);
  });

  it("returns empty array when JSON is not an array", () => {
    expect(parseDefaultEnv(makeEntry({ default_env_json: '"scalar"' }))).toEqual([]);
  });

  it("returns empty array for invalid JSON", () => {
    expect(parseDefaultEnv(makeEntry({ default_env_json: "{broken" }))).toEqual([]);
  });
});

// ------- parseInstallHeaders -------
describe("parseInstallHeaders", () => {
  it("returns header specs for an sse transport with headers", () => {
    const entry = makeEntry({
      install_spec_json: JSON.stringify({
        transport: "sse",
        url: "https://api.example.com/sse",
        headers: { Authorization: "Bearer {{API_KEY}}", "X-Custom": "value" }
      }),
      default_env_json: JSON.stringify([
        {
          key: "Authorization",
          label: "Auth Header",
          description: "Bearer token",
          required: true,
          secret: true,
          default: null
        }
      ])
    });
    const result = parseInstallHeaders(entry);
    expect(result).toHaveLength(2);

    // Authorization header should have metadata from default_env
    const auth = result.find((h) => h.key === "Authorization")!;
    expect(auth.description).toBe("Bearer token");
    expect(auth.required).toBe(true);
    expect(auth.secret).toBe(true);

    // X-Custom header has no default_env match — gets defaults
    const custom = result.find((h) => h.key === "X-Custom")!;
    expect(custom.description).toBe("");
    expect(custom.required).toBe(false);
    expect(custom.secret).toBe(false);
    expect(custom.default).toBe("");
  });

  it("returns header specs for streamable_http transport", () => {
    const entry = makeEntry({
      install_spec_json: JSON.stringify({
        transport: "streamable_http",
        url: "https://api.example.com/stream",
        headers: { "X-Token": "tok" }
      })
    });
    const result = parseInstallHeaders(entry);
    expect(result).toHaveLength(1);
    expect(result[0].key).toBe("X-Token");
    expect(result[0].label).toBe("X-Token");
  });

  it("returns empty array for stdio transport", () => {
    const entry = makeEntry({
      install_spec_json: JSON.stringify({
        transport: "stdio",
        command: "node",
        args: ["server.js"]
      })
    });
    expect(parseInstallHeaders(entry)).toEqual([]);
  });

  it("returns empty array when transport is sse but no headers", () => {
    const entry = makeEntry({
      install_spec_json: JSON.stringify({ transport: "sse", url: "https://api.example.com/sse" })
    });
    expect(parseInstallHeaders(entry)).toEqual([]);
  });

  it("returns empty array for invalid JSON", () => {
    expect(parseInstallHeaders(makeEntry({ install_spec_json: "bad json" }))).toEqual([]);
  });

  it("returns empty array for non-object install spec", () => {
    expect(parseInstallHeaders(makeEntry({ install_spec_json: '"just a string"' }))).toEqual([]);
  });
});
