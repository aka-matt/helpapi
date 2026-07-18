# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`helpapi` is a Rust Mock API CLI tool with WASM export capability. It loads mock rules from JSON config, runs a local HTTP mock server, forwards requests to real upstream APIs with transforms, and provides a Ratatui terminal UI.

## Build & Development Commands

```bash
# Format
cargo fmt --all --check

# Lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Test
cargo test --workspace

# Full CI check (includes docs and deny)
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features && cargo doc --workspace --no-deps && cargo deny check

# WASM build
wasm-pack build crates/mock-wasm --target bundler

# WASM test
wasm-pack test --node crates/mock-wasm
```

## Architecture

### Crate Structure (Cargo Workspace)

```
mock-api/
├── crates/
│   ├── mock-core/      # Pure business logic: rule matching, decisions, transforms
│   ├── mock-config/    # JSON config parsing, validation, schema
│   ├── mock-http/      # HTTP server, upstream forwarding (Axum + Reqwest)
│   ├── mock-runtime/   # Service lifecycle, event dispatch
│   ├── mock-cli/       # CLI args, Ratatui TUI
│   └── mock-wasm/      # WASM bindings for browser/Node.js
```

### Dependency Hierarchy

```
mock-cli → mock-runtime → mock-http → mock-core
                                ↘         ↗
                                  mock-config
mock-wasm → mock-core
          ↘
            mock-config
```

Key principle: `mock-core` has **zero** dependencies on network, CLI, or WASM layers.

### Data Flow

```
JSON Config → mock-cli reads → mock-config parses/validates → mock-runtime creates engine
    → mock-http receives request → mock-core matches rules → Decision
    ├── MockResponse: return local response
    ├── ForwardPlan: modify and forward to upstream
    └── Reject: return config error
    → mock-http executes network ops → mock-core applies response transforms → TUI event
```

### Core Types (in `mock-core`)

- `RequestData`: method, path, query, headers, body (host-agnostic)
- `ResponseData`: status, headers, body, delay_ms
- `ForwardPlan`: url, method, headers, body, timeout_ms, context
- `Decision`: `Mock` | `Forward` | `Reject` — each with metadata

## Key Design Decisions

1. **No WASM runtime for CLI**: Native CLI directly links `mock-core`, does not go through WASM
2. **Atomic config reload**: New config compiled first, only swapped on success (using `ArcSwap` or `RwLock`)
3. **Body limits**: In-memory buffering with configurable max sizes; streaming deferred to later
4. **JSON Pointer transforms**: All payload modifications use RFC 6901 JSON Pointer, not custom syntax
5. **Sensitive header masking**: `authorization`, `cookie`, `proxy-authorization` masked in logs/UI by default
6. **Strict error handling**: Non-JSON body + JSON transform rule = error (no silent skip)

## Configuration Format

```json
{
  "version": 1,
  "server": { "host": "127.0.0.1", "port": 8080 },
  "defaults": { "upstream_timeout_ms": 10000, "max_body_bytes": 1048576 },
  "routes": [
    {
      "id": "get-user",
      "priority": 100,
      "match": { "method": "GET", "path": "/users/:id", "headers": {} },
      "action": { "type": "mock", "response": { "status": 200, "headers": {}, "json": {} } }
    }
  ]
}
```

## Phased Implementation

The project follows a staged delivery plan:

- **Phase 0**: Workspace scaffold, six crates, rustfmt/Clippy CI
- **Phase 1**: Config model + validation + `mock-api validate` command
- **Phase 2**: Core rule engine (matching, transforms, decisions)
- **Phase 3**: HTTP Server + upstream forwarding
- **Phase 4**: Runtime lifecycle + CLI commands + hot reload
- **Phase 5**: Ratatui MVP (request list, details, keyboard nav)
- **Phase 6**: WASM package for browser/Node.js reuse
- **Phase 7+**: Advanced features (recording, replay, fault injection, WebSocket)

## Performance Constraints (MVP)

- Max request body: 1 MiB
- Max response body: 5 MiB
- Default upstream timeout: 10s
- Request history: 1000 entries
- Default max concurrency: 100
- Config rule soft limit: 10,000

## Recommended Development Order

```
Config model → Core models → Rule matching → Transforms → Mock decision → ForwardPlan
    → HTTP Server → Upstream forwarding → Runtime → CLI commands → Ratatui → WASM
```

Do NOT build the TUI first. Prove config, rule engine, and forwarding work via CLI/integration tests first.
