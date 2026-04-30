# Local Development

## Rust

Run all Rust tests:

```bash
cargo test --workspace
```

Run the TUI fake session:

```bash
cargo run -p agent-tui
```

## GUI

Install frontend dependencies:

```bash
pnpm install
```

Run Vue unit tests:

```bash
pnpm --filter agent-gui run test
```

Run the Vite development server:

```bash
pnpm --filter agent-gui run dev
```

## Tauri desktop app

Run the Tauri app in development mode (starts both Vite dev server and the native window with hot-reload):

```bash
pnpm --filter agent-gui run tauri:dev
```

Build the Tauri desktop app:

```bash
pnpm --filter agent-gui run tauri:build
```

## Privacy Defaults

The initial runtime stores event envelopes and full fake-session content in SQLite during tests. Production configuration must default to `minimal_trace` when a real model or shell tool is configured.
