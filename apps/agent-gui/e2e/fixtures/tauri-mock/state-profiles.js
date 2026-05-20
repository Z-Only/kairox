/**
 * Browser-side Tauri mock fragment — profile fixtures.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

state.profiles = [
  {
    alias: "fast",
    provider: "openai",
    model_id: "gpt-4o-mini",
    local: false,
    has_api_key: true,
    supports_reasoning: false
  },
  {
    alias: "smart",
    provider: "openai",
    model_id: "gpt-4o",
    local: false,
    has_api_key: true,
    supports_reasoning: true
  },
  {
    alias: "claude",
    provider: "anthropic",
    model_id: "claude-sonnet-4-20250514",
    local: false,
    has_api_key: true,
    supports_reasoning: true
  },
  {
    alias: "fake",
    provider: "fake",
    model_id: "fake-model",
    local: true,
    has_api_key: false,
    supports_reasoning: false
  }
];
