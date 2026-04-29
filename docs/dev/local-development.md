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
cd apps/agent-gui && npm install
```

Run Vue unit tests:

```bash
cd apps/agent-gui && npm test
```

Run the Vite development server:

```bash
cd apps/agent-gui && npm run dev
```

## Privacy Defaults

The initial runtime stores event envelopes and full fake-session content in SQLite during tests. Production configuration must default to `minimal_trace` when a real model or shell tool is configured.
