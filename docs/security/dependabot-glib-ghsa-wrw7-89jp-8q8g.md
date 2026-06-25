# Dependabot glib Advisory Triage

Date: 2026-06-25

## Alert

- Alert: Dependabot alert #5
- Advisory: GHSA-wrw7-89jp-8q8g
- State: open
- Package: glib
- Manifest: Cargo.lock
- Severity: medium
- Vulnerable range: >= 0.15.0, < 0.20.0
- First patched version: 0.20.0

## Current dependency path

`cargo tree --target all -i glib@0.18.5` confirms the locked vulnerable version is pulled in through the Linux GUI stack used by Tauri:

```text
glib v0.18.5
├── atk v0.18.2
│   └── gtk v0.18.2
│       ├── muda v0.19.2
│       │   └── tauri v2.11.3
│       │       └── agent-gui-tauri v0.41.0
│       ├── tao v0.35.3
│       │   └── tauri-runtime-wry v2.11.3
│       │       └── tauri v2.11.3
│       ├── webkit2gtk v2.0.2
│       │   ├── tauri v2.11.3
│       │   ├── tauri-runtime v2.11.3
│       │   ├── tauri-runtime-wry v2.11.3
│       │   └── wry v0.55.1
│       └── wry v0.55.1
├── gio v0.18.4
├── gtk v0.18.2
├── javascriptcore-rs v1.1.2
├── pango v0.18.3
├── soup3 v0.5.0
└── webkit2gtk v2.0.2
```

The full reverse tree also shows the same `glib v0.18.5` version behind related GTK crates including `cairo-rs`, `gdk`, `gdk-pixbuf`, `gdkx11`, and `webkit2gtk`.

## Compatible update attempts

The alert and dependency path were reconfirmed before editing this document:

```bash
gh api repos/Z-Only/kairox/dependabot/alerts/5 --jq '{number,state,package:.dependency.package.name,manifest:.dependency.manifest_path,severity:.security_vulnerability.severity,range:.security_vulnerability.vulnerable_version_range,patched:.security_vulnerability.first_patched_version.identifier,advisory:.security_advisory.ghsa_id}'
cargo tree --target all -i glib@0.18.5
cargo update -p glib --dry-run
cargo update -p tauri --dry-run --verbose
```

Outcomes:

- `cargo update -p glib --dry-run` reported `Locking 0 packages to latest compatible versions`; it did not move `glib` from 0.18.5 to the patched 0.20.0 line.
- `cargo update -p tauri --dry-run --verbose` also reported `Locking 0 packages to latest compatible versions`; it did not produce a compatible Tauri/Wry/GTK update path that reaches `glib >= 0.20.0`.
- The verbose Tauri dry run only listed unrelated unchanged packages and `muda v0.19.2` with `v0.19.3` available; it did not list a GTK/WebKit stack update that would replace the vulnerable `glib` series.

## Triage decision

No lockfile-only fix is currently available. The patched `glib 0.20.0` line would require moving the dependent GTK/WebKit ecosystem from the currently locked `0.18` series, and this lane must not force an incompatible GTK stack upgrade.

This alert is triaged, not fixed. Re-run this triage when Tauri, Wry, GTK/WebKit, or their Linux GUI stack dependencies release a compatible update path that uses `glib >= 0.20.0`. Treat the Dependabot alert as fixed only after the alert closes or `Cargo.lock` no longer contains a vulnerable `glib` version.
