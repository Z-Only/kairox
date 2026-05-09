#!/usr/bin/env bash
set -euo pipefail

pilot_js_string() {
  python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$1"
}

pilot_fill_textarea() {
  local selector="$1" text="$2" selector_json text_json
  selector_json="$(pilot_js_string "$selector")"
  text_json="$(pilot_js_string "$text")"
  tauri-pilot eval - <<EOF || return 1
const el = document.querySelector(${selector_json});
if (!el) throw new Error('selector not found: ' + ${selector_json});
const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set;
setter.call(el, ${text_json});
el.dispatchEvent(new Event('input', { bubbles: true }));
'ok'
EOF
}

pilot_set_reduced_motion() {
  local mode="$1"
  if [[ "$mode" == "on" ]]; then
    tauri-pilot eval - <<'EOF'
let s = document.getElementById('audit-reduced-motion');
if (!s) {
  s = document.createElement('style');
  s.id = 'audit-reduced-motion';
  s.textContent = '*,*::before,*::after{transition:none!important;animation:none!important}';
  document.head.appendChild(s);
}
'on'
EOF
  else
    tauri-pilot eval - <<'EOF'
const s = document.getElementById('audit-reduced-motion');
if (s) s.remove();
'off'
EOF
  fi
}

pilot_measure_fps() {
  local duration="$1"
  tauri-pilot eval - <<EOF
new Promise(resolve => {
  let frames = 0;
  const start = performance.now();
  function tick() {
    frames++;
    if (performance.now() - start < ${duration}) requestAnimationFrame(tick);
    else resolve((frames * 1000 / (performance.now() - start)).toFixed(1));
  }
  requestAnimationFrame(tick);
})
EOF
}

pilot_run_axe() {
  tauri-pilot eval - <<'EOF'
(async () => {
  if (!window.axe) {
    const code = await (await fetch('/audit/axe.min.js')).text();
    new Function(code)();
  }
  const r = await window.axe.run({ resultTypes: ['violations'] });
  return JSON.stringify({
    violations: r.violations.map(v => ({
      id: v.id,
      impact: v.impact,
      help: v.help,
      nodes: v.nodes.map(n => ({ target: n.target, html: n.html.slice(0, 200) }))
    }))
  });
})()
EOF
}

pilot_probe_tab_order() {
  local count="${1:-30}"
  tauri-pilot eval - <<EOF
(async () => {
  const trail = [];
  document.body.focus();
  for (let i = 0; i < ${count}; i++) {
    const focusables = Array.from(document.querySelectorAll(
      'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'
    )).filter(el => {
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
    });
    const cur = document.activeElement;
    const idx = focusables.indexOf(cur);
    const next = focusables[(idx + 1) % focusables.length];
    if (next) next.focus();
    const a = document.activeElement;
    const sel = a.dataset?.test ? '[data-test=' + JSON.stringify(a.dataset.test) + ']'
              : a.id ? '#' + a.id
              : a.tagName.toLowerCase() + (a.className ? '.' + String(a.className).split(/\s+/).join('.') : '');
    trail.push({ step: i, selector: sel });
  }
  return JSON.stringify(trail);
})()
EOF
}

pilot_probe_focus_ring() {
  local selector="$1" out="$2" selector_json
  selector_json="$(pilot_js_string "$selector")"
  mkdir -p "$out"
  tauri-pilot eval - <<EOF >/dev/null
new Promise(resolve => {
  const el = document.querySelector(${selector_json});
  if (!el) throw new Error('not found: ' + ${selector_json});
  el.blur();
  requestAnimationFrame(() => resolve('blurred'));
})
EOF
  tauri-pilot screenshot "$out/focus-blur.png" --selector "$selector" >/dev/null
  tauri-pilot eval - <<EOF >/dev/null
new Promise(resolve => {
  document.querySelector(${selector_json}).focus();
  requestAnimationFrame(() => resolve('focused'));
})
EOF
  tauri-pilot screenshot "$out/focus-focus.png" --selector "$selector" >/dev/null
  if command -v compare >/dev/null 2>&1; then
    local diff_pixels compare_status total_pixels
    set +e
    diff_pixels="$(compare -metric AE "$out/focus-blur.png" "$out/focus-focus.png" "$out/focus-diff.png" 2>&1)"
    compare_status=$?
    set -e
    if [[ "$compare_status" -ne 0 && "$compare_status" -ne 1 ]]; then
      return "$compare_status"
    fi
    total_pixels="$(identify -format '%[fx:w*h]' "$out/focus-blur.png")"
    awk -v diff_pixels="$diff_pixels" -v total="$total_pixels" 'BEGIN { printf("%.4f\n", diff_pixels*100/total) }'
  else
    local pixelmatch_output diff_pixels pixelmatch_status
    set +e
    pixelmatch_output="$(npx -y pixelmatch "$out/focus-blur.png" "$out/focus-focus.png" "$out/focus-diff.png" 2>&1)"
    pixelmatch_status=$?
    set -e
    diff_pixels="$(printf '%s\n' "$pixelmatch_output" | awk '/different pixels:/ { print $3; exit }')"
    if [[ -z "$diff_pixels" && "$pixelmatch_output" =~ ^[0-9]+$ ]]; then
      diff_pixels="$pixelmatch_output"
    fi
    if ! [[ "$diff_pixels" =~ ^[0-9]+$ ]]; then
      printf '%s\n' "$pixelmatch_output" >&2
      return "$pixelmatch_status"
    fi
    local total_pixels
    total_pixels="$(node -e "const fs = require('fs'); const png = fs.readFileSync(process.argv[1]); const width = png.readUInt32BE(16); const height = png.readUInt32BE(20); console.log(width * height);" "$out/focus-blur.png")"
    awk -v diff_pixels="$diff_pixels" -v total="$total_pixels" 'BEGIN { printf("%.4f\n", diff_pixels*100/total) }'
  fi
}

pilot_collect_evidence() {
  local scenario="$1"
  local ts dir prev_theme_raw prev_theme
  ts="$(date -u +%Y%m%dT%H%M%SZ)"
  dir="audit-runs/${scenario}-${ts}"
  mkdir -p "${dir}/screenshots"
  tauri-pilot snapshot -i --json > "${dir}/snapshot.json"
  tauri-pilot logs --level error > "${dir}/logs.txt" || true
  tauri-pilot network --failed > "${dir}/network.json" || true

  prev_theme_raw="$(tauri-pilot eval - <<'EOF'
const v = localStorage.getItem('kairox.color-mode'); v ?? 'auto'
EOF
)"
  if command -v jq >/dev/null 2>&1 && prev_theme="$(printf '%s' "$prev_theme_raw" | jq -r '.' 2>/dev/null)"; then
    :
  else
    prev_theme="$(printf '%s' "$prev_theme_raw" | sed -e 's/^"//' -e 's/"$//')"
  fi
  case "$prev_theme" in auto|light|dark) ;; *) prev_theme="auto" ;; esac

  tauri-pilot eval - <<'EOF' >/dev/null
localStorage.setItem('kairox.color-mode', 'light');
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: 'light' }));
'light'
EOF
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  tauri-pilot screenshot "${dir}/screenshots/light.png"

  tauri-pilot eval - <<'EOF' >/dev/null
localStorage.setItem('kairox.color-mode', 'dark');
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: 'dark' }));
'dark'
EOF
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  tauri-pilot screenshot "${dir}/screenshots/dark.png"

  prev_theme_json="$(pilot_js_string "$prev_theme")"
  tauri-pilot eval - <<EOF >/dev/null
localStorage.setItem('kairox.color-mode', ${prev_theme_json});
window.dispatchEvent(new StorageEvent('storage', { key: 'kairox.color-mode', newValue: ${prev_theme_json} }));
'restored'
EOF
  tauri-pilot eval 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))' >/dev/null
  pilot_set_reduced_motion on
  tauri-pilot screenshot "${dir}/screenshots/reduced-motion.png"
  pilot_set_reduced_motion off

  pilot_run_axe > "${dir}/axe.json" || true
  echo "${dir}"
}
