#!/usr/bin/env bash
set -euo pipefail

# Check that Rust EventPayload variants match TypeScript types

rust_variants=$(grep -oE 'Self::[A-Z][a-zA-Z]+' crates/agent-core/src/events.rs | sed 's/Self:://' | sort -u)
ts_variants=$(grep -oE 'type: "[A-Z][a-zA-Z]+"' apps/agent-gui/src/types/index.ts | sed 's/type: "//;s/"//' | sort -u)

rust_count=$(echo "$rust_variants" | wc -l | tr -d ' ')
ts_count=$(echo "$ts_variants" | wc -l | tr -d ' ')

echo "Rust variants ($rust_count):"
echo "$rust_variants"
echo ""
echo "TS variants ($ts_count):"
echo "$ts_variants"
echo ""

only_rust=$(comm -23 <(echo "$rust_variants") <(echo "$ts_variants") || true)
only_ts=$(comm -13 <(echo "$rust_variants") <(echo "$ts_variants") || true)

if [ -n "$only_rust" ]; then
  echo "⚠️  In Rust but missing from TS:"
  echo "$only_rust"
fi

if [ -n "$only_ts" ]; then
  echo "⚠️  In TS but missing from Rust:"
  echo "$only_ts"
fi

if [ -z "$only_rust" ] && [ -z "$only_ts" ]; then
  echo "✅ EventPayload variants are in sync"
else
  echo "❌ EventPayload variants are out of sync!"
  exit 1
fi
