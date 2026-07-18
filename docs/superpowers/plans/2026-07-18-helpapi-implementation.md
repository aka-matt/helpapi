# helpapi Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a Rust Mock API CLI tool with WASM export — loads JSON config, runs local HTTP mock server, forwards to upstream APIs with transforms, optional Ratatui TUI.

**Architecture:** "Shared core + host adapters" — `mock-core` holds pure business logic; `mock-config` handles JSON config parsing/validation; `mock-http` wraps native HTTP networking; `mock-runtime` manages service lifecycle; `mock-cli` provides CLI+TUI; `mock-wasm` exposes core as WASM. Native CLI links crates directly; WASM is a separate publishing artifact, not the runtime path.

**Tech Stack:** Rust 1.85, Cargo Workspace resolver="2", tokio, axum, reqwest, tower, serde, serde_json, ratatui, wasm-bindgen, wasm-pack, tracing, thiserror, uuid, url, jsonptr (RFC 6901)

---

## Global Constraints

- Rust edition 2024, rust-version 1.85, resolver 2
- `mock-core` has ZERO deps on tokio, axum, reqwest, ratatui, wasm-bindgen
- No arbitrary scripting (no lua/rhai/js execution)
- All payload transforms use RFC 6901 JSON Pointer only
- Config hot-reload: validate new config first, atomically replace on success (ArcSwap or RwLock)
- Sensitive headers (`authorization`, `cookie`, `proxy-authorization`) masked in logs/UI by default
- Max request body: 1 MiB; max response body: 5 MiB; upstream timeout: 10s; request history: 1000

---

## Phase 0: Workspace & Engineering Foundation

### Task 1: Create Cargo workspace root

**Files:**
- Create: `Cargo.toml`
- Create: `rustfmt.toml`
- Create: `deny.toml`
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: workspace root consumed by all crates

- [ ] **Step 1: Create Cargo.toml workspace**

```toml
[workspace]
resolver = "2"
members = [
    "crates/mock-core",
    "crates/mock-config",
    "crates/mock-http",
    "crates/mock-runtime",
    "crates/mock-cli",
    "crates/mock-wasm",
]

[workspace.package]
edition = "2024"
license = "MIT"
rust-version = "1.85"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
url = "2"
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 2: Create rustfmt.toml**

```toml
edition = "2024"
```

- [ ] **Step 3: Create deny.toml** (use default/standard deny config with MIT license)

- [ ] **Step 4: Create .github/workflows/ci.yml** with: cargo fmt --all --check, cargo clippy --workspace --all-targets --all-features -- -D warnings, cargo test --workspace, cargo doc --workspace --no-deps

- [ ] **Step 5: Create .gitignore** (already exists, verify it covers target/, *.wasm, Cargo.lock)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rustfmt.toml deny.toml .github/workflows/ci.yml .gitignore
git commit -m "phase0: add workspace root and CI"
```

---

### Task 2: Scaffold six crates (empty lib.rs + Cargo.toml)

**Files:**
- Create: `crates/mock-core/Cargo.toml`, `crates/mock-core/src/lib.rs`
- Create: `crates/mock-config/Cargo.toml`, `crates/mock-config/src/lib.rs`
- Create: `crates/mock-http/Cargo.toml`, `crates/mock-http/src/lib.rs`
- Create: `crates/mock-runtime/Cargo.toml`, `crates/mock-runtime/src/lib.rs`
- Create: `crates/mock-cli/Cargo.toml`, `crates/mock-cli/src/main.rs`
- Create: `crates/mock-wasm/Cargo.toml`, `crates/mock-wasm/src/lib.rs`

**Interfaces:**
- Produces: six bare crate stubs with correct internal deps per helpapi.md section 2.3

- [ ] **Step 1: Create mock-core/Cargo.toml** — name="mock-core", dependencies: serde, serde_json, thiserror, tracing, uuid, url

- [ ] **Step 2: Create mock-config/Cargo.toml** — name="mock-config", dependencies: serde, serde_json, thiserror, tracing + dev-dependency: schemars for JSON Schema generation

- [ ] **Step 3: Create mock-http/Cargo.toml** — name="mock-http", dependencies: mock-core, mock-config, tokio, axum, reqwest, tower, tracing

- [ ] **Step 4: Create mock-runtime/Cargo.toml** — name="mock-runtime", dependencies: mock-core, mock-config, mock-http, tokio, tracing

- [ ] **Step 5: Create mock-cli/Cargo.toml** — name="mock-cli", dependencies: mock-runtime, mock-core, mock-config, tokio, ratatui, crossterm, tracing

- [ ] **Step 6: Create mock-wasm/Cargo.toml** — name="mock-wasm", dependencies: mock-core, mock-config, wasm-bindgen, serde, serde_json, thiserror

- [ ] **Step 7: Create stub lib.rs for each crate** (empty or with a trivial pub struct)

- [ ] **Step 8: Verify cargo build --workspace compiles**

- [ ] **Step 9: Commit**

```bash
git add crates/
git commit -m "phase0: scaffold six crates"
```

---

## Phase 1: mock-config (Config Model & Validation)

### Task 3: Define config data models

**Files:**
- Create: `crates/mock-config/src/lib.rs`
- Create: `crates/mock-config/src/models.rs`
- Create: `crates/mock-config/src/error.rs`

**Interfaces:**
- Produces: Config, Route, MatchRule, Action, Transform structs consumed by mock-core and mock-cli

- [ ] **Step 1: Define Config struct** (version, server {host, port}, defaults {upstream_timeout_ms, max_body_bytes}, routes, unmatched)

- [ ] **Step 2: Define Route struct** (id, priority, match, action)

- [ ] **Step 3: Define MatchRule struct** (method, path, headers, query, body — all optional)

- [ ] **Step 4: Define Action enum** (Mock { response }, Forward { upstream, request_transforms, response_transforms })

- [ ] **Step 5: Define Transform structs** (SetHeader, RemoveHeader, SetQuery, RemoveQuery, SetJsonPointer, RemoveJsonPointer, ReplaceBody, SetStatus)

- [ ] **Step 6: Define Response struct** (status, headers, json_body, text_body, delay_ms) — use BodyData enum

- [ ] **Step 7: Define ValidationIssue struct** (code, path, message, severity, suggestion)

- [ ] **Step 8: Define ConfigError enum** with variants: Parse, Validation, Io

- [ ] **Step 9: Write unit tests for model JSON round-trip**

- [ ] **Step 10: Commit**

```bash
git add crates/mock-config/src/
git commit -m "phase1: add config data models"
```

---

### Task 4: Config parsing, validation, JSON Schema

**Files:**
- Create: `crates/mock-config/src/parse.rs`
- Create: `crates/mock-config/src/validate.rs`
- Create: `crates/mock-config/src/schema.rs`
- Modify: `crates/mock-config/src/lib.rs` (re-exports)

**Interfaces:**
- Produces: parse_json(), validate(), parse_and_validate(), generate_schema() functions

- [ ] **Step 1: Write parse_json()** — parse &str to Config, handle errors with context (JSON path)

- [ ] **Step 2: Write validate()** — semantic validation: required fields, priority uniqueness, transform correctness, upstream URL validity, severity levels

- [ ] **Step 3: Write parse_and_validate()** — compose parse + validate

- [ ] **Step 4: Write generate_schema()** — produce JSON Schema for Config (for editors)

- [ ] **Step 5: Write Integration tests** — valid config, invalid config (missing required fields, bad URL, circular priority)

- [ ] **Step 6: Commit**

```bash
git add crates/mock-config/src/
git commit -m "phase1: add config parsing, validation, schema generation"
```

---

### Task 5: mock-api validate CLI command

**Files:**
- Modify: `crates/mock-cli/src/main.rs`
- Create: `crates/mock-cli/src/commands/validate.rs`

**Interfaces:**
- Consumes: mock-config parse_and_validate()
- Produces: validate --config <path> CLI command

- [ ] **Step 1: Create basic CLI argument parser** using clap (subcommands: validate, run, schema, print-effective-config, version)

- [ ] **Step 2: Implement validate command** — read config file, call parse_and_validate(), print human-readable errors with JSON paths, exit code 0/1

- [ ] **Step 3: Add tests** — valid file, invalid file, non-existent file

- [ ] **Step 4: Commit**

```bash
git add crates/mock-cli/src/
git commit -m "phase1: add mock-api validate command"
```

---

## Phase 2: mock-core (Rule Engine)

### Task 6: Core data models (RequestData, ResponseData, BodyData)

**Files:**
- Modify: `crates/mock-core/src/lib.rs`
- Create: `crates/mock-core/src/request.rs`
- Create: `crates/mock-core/src/response.rs`
- Create: `crates/mock-core/src/body.rs`

**Interfaces:**
- Produces: RequestData, ResponseData, BodyData — consumed by mock-http and mock-wasm

- [ ] **Step 1: Define BodyData enum** (Empty, Text(String), Json(serde_json::Value), Binary(Vec<u8>))

- [ ] **Step 2: Define RequestData struct** (method, path, query Vec<(K,V)>, headers Vec<(K,V)>, body BodyData)

- [ ] **Step 3: Define ResponseData struct** (status, headers, body, delay_ms)

- [ ] **Step 4: Define ForwardPlan struct** (url, method, headers, body, timeout_ms, context MatchedRequestContext)

- [ ] **Step 5: Define MatchedRequestContext** (route_id, path_params HashMap, matched_rules)

- [ ] **Step 6: Define DecisionMetadata** (route_id, matcher_results, transforms_applied, elapsed_us)

- [ ] **Step 7: Write unit tests**

- [ ] **Step 8: Commit**

```bash
git add crates/mock-core/src/
git commit -m "phase2: add core data models"
```

---

### Task 7: Decision enum and error types

**Files:**
- Create: `crates/mock-core/src/decision.rs`
- Create: `crates/mock-core/src/error.rs`

**Interfaces:**
- Produces: Decision enum (Mock/Forward/Reject), EngineError

- [ ] **Step 1: Define Decision enum** with Mock { response ResponseData, metadata DecisionMetadata }, Forward { plan ForwardPlan, metadata DecisionMetadata }, Reject { response ResponseData, metadata DecisionMetadata }

- [ ] **Step 2: Define EngineError enum** (CompileError, MatchError, TransformError, UnsupportedBodyType)

- [ ] **Step 3: Write unit tests for Decision serialization**

- [ ] **Step 4: Commit**

```bash
git add crates/mock-core/src/
git commit -m "phase2: add Decision enum and error types"
```

---

### Task 8: Matcher implementations

**Files:**
- Create: `crates/mock-core/src/matcher/mod.rs`
- Create: `crates/mock-core/src/matcher/method.rs`
- Create: `crates/mock-core/src/matcher/path.rs`
- Create: `crates/mock-core/src/matcher/header.rs`
- Create: `crates/mock-core/src/matcher/query.rs`

**Interfaces:**
- Consumes: MatchRule, RequestData
- Produces: MatcherResult (matches bool, path_params HashMap)

- [ ] **Step 1: Define Matcher trait** — fn match(&self, request: &RequestData) -> MatcherResult

- [ ] **Step 2: Implement MethodMatcher** — exact string match, or "*" wildcard

- [ ] **Step 3: Implement PathMatcher** — exact match, :param segment capture, priority scoring

- [ ] **Step 4: Implement HeaderMatcher** — exact value, existence check, case-insensitive key comparison

- [ ] **Step 5: Implement QueryMatcher** — exact key-value, key-only existence

- [ ] **Step 6: Write unit tests for all matchers** — edge cases, path param extraction

- [ ] **Step 7: Commit**

```bash
git add crates/mock-core/src/matcher/
git commit -m "phase2: add matcher implementations"
```

---

### Task 9: Transform implementations

**Files:**
- Create: `crates/mock-core/src/transform/mod.rs`
- Create: `crates/mock-core/src/transform/headers.rs`
- Create: `crates/mock-core/src/transform/query.rs`
- Create: `crates/mock-core/src/transform/json.rs`
- Create: `crates/mock-core/src/transform/template.rs`

**Interfaces:**
- Consumes: Transform, RequestData/ResponseData
- Produces: transformed RequestData/ResponseData

- [ ] **Step 1: Define Transform trait** — fn transform_request(&self, request: RequestData) -> Result<RequestData, TransformError>; fn transform_response(&self, response: ResponseData) -> Result<ResponseData, TransformError>

- [ ] **Step 2: Implement SetHeader, RemoveHeader** (header transforms)

- [ ] **Step 3: Implement SetQuery, RemoveQuery** (query transforms)

- [ ] **Step 4: Implement SetJsonPointer, RemoveJsonPointer** using jsonptr crate for RFC 6901 JSON Pointer

- [ ] **Step 5: Implement ReplaceBody** (replace entire body)

- [ ] **Step 6: Implement SetStatus** (response only)

- [ ] **Step 7: Implement template substitution** — {{path.param}} in header values, body, status; handle missing variables gracefully

- [ ] **Step 8: Write comprehensive unit tests** — JSON Pointer edge cases, missing vars, non-JSON body + JSON transform = error

- [ ] **Step 9: Commit**

```bash
git add crates/mock-core/src/transform/
git commit -m "phase2: add transform implementations"
```

---

### Task 10: Rule engine (compile + decide)

**Files:**
- Create: `crates/mock-core/src/engine.rs`
- Create: `crates/mock-config/src/compiled.rs` (intermediate compiled representation)

**Interfaces:**
- Consumes: Config (from mock-config)
- Produces: Engine; Engine::decide(RequestData) -> Decision

- [ ] **Step 1: Define CompiledRule struct** — pre-parsed matchers, pre-resolved transforms, priority

- [ ] **Step 2: Define Engine struct** with compiled rules sorted by priority

- [ ] **Step 3: Implement Engine::compile(Config)** — parse each route into CompiledRule, validate references, sort by priority

- [ ] **Step 4: Implement Engine::decide(RequestData)** — iterate rules in priority order, run all matchers, first full match wins; handle unmatched via config.default_unmatched

- [ ] **Step 5: Implement Engine::transform_upstream_response(MatchedRequestContext, ResponseData)** — apply response_transforms from matched route

- [ ] **Step 6: Implement Engine::transform_request(RequestData, ForwardPlan)** — apply request_transforms before forwarding

- [ ] **Step 7: Write deterministic tests** — same input → same output; priority ordering; template variables

- [ ] **Step 8: Commit**

```bash
git add crates/mock-core/src/engine.rs crates/mock-config/src/compiled.rs
git commit -m "phase2: add rule engine (compile + decide)"
```

---

## Phase 3: mock-http (HTTP Server & Forwarding)

### Task 11: HTTP server setup with Axum

**Files:**
- Modify: `crates/mock-http/src/lib.rs`
- Create: `crates/mock-http/src/server.rs`
- Create: `crates/mock-http/src/handler.rs`
- Create: `crates/mock-http/src/client.rs` (upstream HTTP client)

**Interfaces:**
- Consumes: mock-core Engine, ServerConfig { host, port }
- Produces: HTTP server that calls Engine::decide per request

- [ ] **Step 1: Define ServerConfig struct** (host, port, max_body_bytes, upstream_timeout_ms)

- [ ] **Step 2: Implement HttpServer struct** with shutdown handle

- [ ] **Step 3: Implement start_server(config, engine, events) async fn** — bind axum router, spawn tokio tasks

- [ ] **Step 4: Implement request handler** — read method/uri/headers/body, check body size limit, convert to RequestData, call engine.decide(), build HTTP response for Mock/Reject, return response for Forward (with transformed request)

- [ ] **Step 5: Implement UpstreamClient** using reqwest — send ForwardPlan, apply transforms, handle timeout, map errors to stable error responses

- [ ] **Step 6: Implement graceful shutdown** — handle SIGINT/SIGTERM, drain in-flight requests up to timeout

- [ ] **Step 7: Write integration tests** — mock route returns expected response, curl can hit server

- [ ] **Step 8: Commit**

```bash
git add crates/mock-http/src/
git commit -m "phase3: add HTTP server with Axum"
```

---

### Task 12: Event emission

**Files:**
- Create: `crates/mock-http/src/events.rs`

**Interfaces:**
- Produces: RuntimeEvent enum — emitted to mock-runtime event channel

- [ ] **Step 1: Define RuntimeEvent enum** (ServerStarted, ServerStopped, ConfigReloaded, RequestStarted, RequestCompleted, RequestFailed)

- [ ] **Step 2: Define RequestSummary** (method, path, rule_id, upstream_url)

- [ ] **Step 3: Implement event emission** — emit RequestStarted before processing, RequestCompleted/RequestFailed after; emit ServerStarted/ServerStopped

- [ ] **Step 4: Commit**

```bash
git add crates/mock-http/src/events.rs
git commit -m "phase3: add event emission"
```

---

## Phase 4: mock-runtime + mock-cli Commands

### Task 13: Runtime lifecycle

**Files:**
- Create: `crates/mock-runtime/src/lib.rs`
- Create: `crates/mock-runtime/src/runtime.rs`
- Create: `crates/mock-runtime/src/error.rs`

**Interfaces:**
- Consumes: Config, ServerConfig, Engine
- Produces: Runtime struct with start/stop/reload/subscribe

- [ ] **Step 1: Define RuntimeStatus enum** (Stopped, Starting, Running, Reloading, Failed)

- [ ] **Step 2: Define Runtime struct** — holds engine, server, event channel, status

- [ ] **Step 3: Implement Runtime::new(config) async** — compile config, create engine, prepare server

- [ ] **Step 4: Implement Runtime::start() async** — spawn HTTP server task, set status = Running, emit ServerStarted

- [ ] **Step 5: Implement Runtime::reload(config) async** — compile new config first, on success atomically replace engine (RwLock or ArcSwap), emit ConfigReloaded; on failure keep old engine, emit ConfigReloadFailed

- [ ] **Step 6: Implement Runtime::stop() async** — signal server shutdown, wait for drain, set status = Stopped, emit ServerStopped

- [ ] **Step 7: Implement Runtime::subscribe() -> EventReceiver** — returns channel receiver for RuntimeEvent

- [ ] **Step 8: Write tests** — start/stop cycle, reload success, reload failure preserves old config

- [ ] **Step 9: Commit**

```bash
git add crates/mock-runtime/src/
git commit -m "phase4: add Runtime lifecycle management"
```

---

### Task 14: CLI commands (run, schema, print-effective-config, version)

**Files:**
- Modify: `crates/mock-cli/src/main.rs`
- Create: `crates/mock-cli/src/commands/run.rs`
- Create: `crates/mock-cli/src/commands/schema.rs`
- Create: `crates/mock-cli/src/commands/print_config.rs`

**Interfaces:**
- Consumes: Runtime, mock-config functions
- Produces: full CLI with run/validate/schema/print-effective-config/version

- [ ] **Step 1: Implement run --config <path>** — read file, create Runtime, start, keep running until SIGINT; support RUST_LOG env var

- [ ] **Step 2: Implement schema --output <path>** — call generate_schema(), write to file

- [ ] **Step 3: Implement print-effective-config --config <path>** — parse, validate, print resolved config

- [ ] **Step 4: Implement version** — print version from Cargo.toml

- [ ] **Step 5: Add file watching** (notify crate) — watch config file, auto-reload on change; print error if new config invalid, keep serving with old config

- [ ] **Step 6: Add log format option** (--log-format pretty/json)

- [ ] **Step 7: Write integration tests** — run command starts server, validate returns correct exit codes

- [ ] **Step 8: Commit**

```bash
git add crates/mock-cli/src/
git commit -m "phase4: add full CLI commands"
```

---

## Phase 5: Ratatui TUI

### Task 15: TUI layout and rendering

**Files:**
- Create: `crates/mock-cli/src/tui/mod.rs`
- Create: `crates/mock-cli/src/tui/app.rs`
- Create: `crates/mock-cli/src/tui/widgets/status_bar.rs`
- Create: `crates/mock-cli/src/tui/widgets/request_list.rs`
- Create: `crates/mock-cli/src/tui/widgets/request_details.rs`
- Create: `crates/mock-cli/src/tui/widgets/help.rs`
- Create: `crates/mock-cli/src/tui/state.rs`

**Interfaces:**
- Consumes: RuntimeEvent stream, RuntimeStatus
- Produces: rendered terminal UI

- [ ] **Step 1: Define AppState struct** (runtime_status, requests VecDeque<RequestRecord>, selected_request, active_panel, filter, config_error, should_quit)

- [ ] **Step 2: Implement StatusBar widget** — show runtime status, address, rule count

- [ ] **Step 3: Implement RequestList widget** — table with method, path, status, latency, rule_id; support selection highlight

- [ ] **Step 4: Implement RequestDetails widget** — show request/response headers, body preview (truncated to 32KiB), matched rule info

- [ ] **Step 5: Implement HelpBar widget** — show keyboard shortcuts

- [ ] **Step 6: Implement main render loop** — layout with Ratatui, crossterm event polling, runtime event receiver

- [ ] **Step 7: Implement keyboard handling** — q(quit), r(reload), j/k or arrows (navigate), / (filter input), Tab (switch panel), Enter (details), c (clear history), ? (toggle help)

- [ ] **Step 8: Implement filter** — filter request list by path/method/status

- [ ] **Step 9: Implement terminal state recovery** — save/restore terminal on panic, ensure cleanup on exit

- [ ] **Step 10: Write tests** — UI renders without panic, keyboard events change state

- [ ] **Step 11: Commit**

```bash
git add crates/mock-cli/src/tui/
git commit -m "phase5: add Ratatui TUI"
```

---

### Task 16: Integrate TUI with Runtime events

**Files:**
- Modify: `crates/mock-cli/src/commands/run.rs`
- Create: `crates/mock-cli/src/tui/event_handler.rs`

**Interfaces:**
- Consumes: Runtime events (RequestCompleted, RequestFailed)
- Produces: UI updates via tui-rs state

- [ ] **Step 1: Subscribe Runtime to events channel**

- [ ] **Step 2: Map RuntimeEvent -> AppState updates** — RequestCompleted adds to request list, ConfigReloadFailed sets config_error

- [ ] **Step 3: Implement sensitive header masking** — mask authorization, cookie, proxy-authorization in displayed headers

- [ ] **Step 4: Implement request body truncation** — preview only 32KiB in UI

- [ ] **Step 5: Commit**

```bash
git add crates/mock-cli/src/tui/event_handler.rs
git commit -m "phase5: integrate TUI with Runtime events"
```

---

## Phase 6: mock-wasm (WASM Package)

### Task 17: WASM bindings

**Files:**
- Modify: `crates/mock-wasm/src/lib.rs`
- Create: `crates/mock-wasm/src/engine.rs`
- Create: `crates/mock-wasm/src/error.rs`

**Interfaces:**
- Produces: wasm-bindgen JS-friendly API consumed by Node.js and browsers

- [ ] **Step 1: Implement WasmMockEngine struct** wrapping mock_core::Engine

- [ ] **Step 2: Implement constructor new(config_json: &str) -> Result<WasmMockEngine, JsValue>** — parse JSON, compile, return engine or JsValue error

- [ ] **Step 3: Implement decide(request_json: &str) -> Result<String, JsValue>** — parse request, call engine.decide(), serialize Decision to JSON string

- [ ] **Step 4: Implement transform_response(context_json: &str, response_json: &str) -> Result<String, JsValue>** — parse context and response, call engine.transform_upstream_response, serialize result

- [ ] **Step 5: Implement validate_config(config_json: &str) -> String** — free function, returns structured JSON error or ok:true

- [ ] **Step 6: Add package.json, tsconfig.json, README.md** for npm publishing

- [ ] **Step 7: Write tests** — Node.js test loads WASM, runs decide(), verifies output

- [ ] **Step 8: Commit**

```bash
git add crates/mock-wasm/
git commit -m "phase6: add WASM bindings"
```

---

## Phase 7: Integration, CI, Release

### Task 18: Example configs and integration tests

**Files:**
- Create: `examples/basic.json`
- Create: `examples/proxy.json`
- Create: `examples/transforms.json`
- Create: `tests/integration/main.rs`

- [ ] **Step 1: Create example configs** covering mock, forward, transforms

- [ ] **Step 2: Write integration tests** — spawn server, make HTTP requests, verify responses

- [ ] **Step 3: Add wasm-pack test step to CI**

- [ ] **Step 4: Commit**

```bash
git add examples/ tests/ .github/workflows/ci.yml
git commit -m "phase7: add examples and integration tests"
```

---

## Spec Coverage Check

| helpapi.md Section | Tasks |
|---|---|
| 2.1 Core architecture (6 crates) | Task 1, 2 |
| 2.2 WASM boundary | Task 17 |
| 2.3 Data flow | Task 11, 13 |
| 4.1 mock-core | Task 6, 7, 8, 9, 10 |
| 4.2 mock-config | Task 3, 4 |
| 4.3 mock-http | Task 11, 12 |
| 4.4 mock-runtime | Task 13 |
| 4.5 mock-cli | Task 5, 14, 15, 16 |
| 4.6 mock-wasm | Task 17 |
| 5 Config format | Task 3 |
| 6 Core data models | Task 6, 7 |
| 7 Rule matching/priority | Task 8, 10 |
| 8 Header/Payload handling | Task 9 |
| 9 Ratatui | Task 15, 16 |
| 10 WASM API | Task 17 |
| 11 Phased delivery | All tasks |
| 12 Tests | Per-task unit tests, Task 18 |
| 13 Security | Task 16 (masking), Task 11 (body limits) |
| 14 Tracing | Task 14 (RUST_LOG), Task 12 |
| 15 Performance | Task 11 (body limits) |
| 16 CI | Task 1, Task 18 |

All sections covered. No gaps.
