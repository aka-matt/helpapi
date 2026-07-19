# Fix Four Blocking Bugs (B1-B4) and Three Emergent Bugs (C1, C2, C4)

**Date:** 2026-07-19
**Status:** Draft
**Authors:** Code review at `.superpowers/sdd/code-review-report.md`; supplementary review at second-agent report (internal session log).
**Scope:** `mock-core`, `mock-http`, `mock-runtime`, `mock-cli`, `mock-wasm`, `examples/*`, integration tests, `CLAUDE.md`

## Background

The post-implementation code review identified four blocking correctness bugs that together mean `mock-api` cannot demonstrate the MVP flows the README and example configs describe. A second, focused review surfaced three additional bugs (C1, C2, C4) that the original B1-B4 fix would either leave reintroducing or actively expose. This spec describes a single, self-contained patch that resolves all seven.

## Goals

1. `Decision::Forward` actually executes an upstream call.
2. Transform specs (`set_header`, `set_json_pointer`, etc.) the example configs use are no longer silently dropped.
3. The TUI starts a single render loop, not two.
4. `Engine::compile` performs the same semantic validation as `mock_config::validate`, so configuration errors surface in every execution path.
5. The "documented example flows" referenced in `examples/basic.json`, `examples/proxy.json`, `examples/transforms.json` (and any related docs) actually exercise those flows end-to-end against the fixed code.
6. **C1:** The CLI integration test that currently hangs forever is bounded so the workspace test suite completes.
7. **C2:** With B1 wiring forward execution, every `RequestStarted` has a paired terminal event under broadcast lag, and the TUI surfaces lag as a counter rather than dropping events silently.
8. **C4:** With B1 wiring forward execution, redirect chains from a configured upstream cannot be used as an SSRF channel; reqwest follows no redirects.

## Non-Goals

- I-row (Important) and N-row (Nit) findings from the original review remain out of scope (these now include C3 partial body on timeout, C5/C7 reload not applying server-side limits, C6 reloading-stopped-runtime status corruption, C8 fake debounce, C9 watcher not shut down in TUI, C10 incomplete terminal restoration on panic, C11 input consumption speed).
- Clippy warning cleanups in `mock-core` and `mock-cli` remain out of scope unless they block compile.
- WASM-side sensitive-header masking remains out of scope.
- `BodyData` serde convergence was promoted into B2's implementation outline (free side effect).
- Phase 7+ features (recording, replay, fault injection, WebSocket) remain out of scope.

## Affected Crates and Files

| File | Reason |
|------|--------|
| `crates/mock-core/Cargo.toml` | No new deps (B2 uses type moved from mock-config) |
| `crates/mock-core/src/transform/spec.rs` | New: re-host `Transform` enum from `mock-config` to break cycle |
| `crates/mock-core/src/engine.rs` | B2 (reuse `Transform` enum), B4 (call `parse_and_validate` first) |
| `crates/mock-http/src/server.rs` | B1 (wire `UpstreamClient`), cleanup |
| `crates/mock-http/src/handler.rs` | B1 (execute forward), B3-adjacent (no change) |
| `crates/mock-http/src/client.rs` | B1 (return 502-class signal) |
| `crates/mock-http/src/error.rs` | B1 (new `UpstreamFailed` variant → 502 mapping) |
| `crates/mock-runtime/src/runtime.rs` | Update `bad_engine_config` test for B4 |
| `crates/mock-cli/src/tui/app.rs` | B3 (delete duplicate `run_loop` call; once-install panic hook), C2 (drain events) |
| `crates/mock-cli/src/tui/state.rs` | C2 (add lag_count field) |
| `crates/mock-cli/src/tui/widgets/status_bar.rs` | C2 (display Lagged counter) |
| `crates/mock-cli/tests/integration.rs` | C1 (bounded-wait helper) |
| `crates/mock-cli/src/commands/run.rs` | (no changes from B1-B4; C8/C9 deferred) |
| `crates/mock-runtime/src/runtime.rs` | Update `bad_engine_config` test for B4; C2 receiver API |
| `crates/mock-wasm/src/engine.rs` | B2 already correct after change (no edit expected) |
| `examples/*.json` | Verify against fixed validator; remove fake `"value": "test-key-12345"` placeholder |
| `crates/mock-integration-tests/tests/integration/main.rs` | Add a real-HTTP forward round-trip test |
| `CLAUDE.md` | Update the phase list / known issues if examples need a contract change |

## B1: Forward execution

### Behavior change

A `Decision::Forward { plan, .. }` produced by `Engine::decide` is now sent to the upstream URL via `UpstreamClient::send`. The client receives the upstream's response (status, headers, body). When `UpstreamClient::send` returns `Err`, the client receives HTTP 502 with a body of `upstream error: <kind>`.

`UpstreamClient::send` already exists at `crates/mock-http/src/client.rs:42`. The whole bug is that **nothing calls it** — `handle_route` in `server.rs:206-310` returns `build_forward_response` (a 102 Processing with the plan in `X-Forward-Plan`), and the runtime never wires `UpstreamClient`.

### Implementation outline

1. `crates/mock-http/src/error.rs` — add a `HttpError::UpstreamFailed { reason: String }` variant. The mapping in `handle_route` (already used for `BodyTooLarge` → 413) extends to `UpstreamFailed` → `BAD_GATEWAY`.
2. `crates/mock-http/src/server.rs` — `AppState` gains `client: UpstreamClient`. `handle_route` inspects the decision:
   - `Decision::Mock` / `Reject` → existing `build_mock_response` / `build_reject_response`.
   - `Decision::Forward` → call `self.client.send(plan.clone(), request_data_from_handle_request, &engine)`. On `Ok(response_data)` build a response from it. On `Err(upstream_err)` build 502 with `upstream error: <kind>`.
3. `handle_request` (`crates/mock-http/src/handler.rs:26`) needs to surface the *original* `RequestData` even on the forward path. Currently it returns `(Response, Decision, Vec<(String, String)>, Vec<u8>)`. For forward we need the `RequestData` itself, not just headers and bytes. Two options:
   - **A (recommended):** Change `handle_request` to return `(Response, Decision, RequestData)` and build the body from `RequestData` separately when needed for events.
   - **B:** Add an explicit path that bypasses `handle_request` for forward decisions.
   
   Pick A. The current tuple's 4th element is already a duplicate of the body inside `RequestData` for forwarding purposes. Simplifying to 3 elements with one of them being `RequestData` is cleaner. The mock-http integration tests need updating.
4. Delete `build_forward_response`, `extract_response_body`, `extract_forward_plan` from `handler.rs:243-264, 267-272, 259-264` — all are dead code that existed only for the abandoned 102-response path. `UpstreamClient::send` already returns the proper upstream response, so no plan extraction is needed downstream.
5. `crates/mock-runtime/src/runtime.rs:127-132` — pass the server config timeout to `UpstreamClient` so per-plan timeouts and the upstream client default agree.

### Test plan

- Update `test_handle_request_forward_decision` in `handler.rs:357-388` to expect a real upstream response (or assert the function returns `HttpError::UpstreamFailed` because no upstream is bound).
- Add `mock-integration-tests/tests/integration/main.rs::test_forward_round_trip_end_to_end`:
  1. Start a tiny upstream `axum::Router` on `127.0.0.1:0` that returns `200 {"echo": "<method>"}`.
  2. Build `Runtime` with a forward route pointing at that upstream.
  3. POST `{a: 1}` to the runtime.
  4. Assert status 200 and body equals `{"echo": "POST"}`.
- Add a second test that asserts an unreachable upstream produces 502 (use a config pointing at `http://127.0.0.1:1` which is reserved).

### Boundary condition: `plan.timeout_ms` precedence

`UpstreamClient::send` already overrides its own default with `plan.timeout_ms` if set. Verified at `client.rs:85-87`. No change needed.

### Boundary condition: hop-by-hop headers on forward

`UpstreamClient::send` filters hop-by-hop headers (`is_hop_by_hop_header` at `client.rs:193-206`). Verified correct.

### Boundary condition: response payload size

When forwarding upstream responses back to the client, `build_mock_response` ultimately calls `Response::builder().body(Body::from(body_bytes))`. There is no body-size cap on upstream responses. CLAUDE.md says "Max response body: 5 MiB". Add a check: if the upstream response body exceeds the server's `max_response_bytes` (default 5 MiB, new field on `ServerConfig`), return `HttpError::ResponseTooLarge` → 502 (forwarding a too-large response is not the client's fault; client returns a body saying upstream exceeded limits).

## B2: Transform variant matching

### Behavior change

`Engine::compile` now deserialises the `request_transforms` and `response_transforms` arrays via a typed `Transform` enum (snake_case-tagged) instead of matching raw `serde_json::Value` strings case-sensitively. Unknown or malformed variants surface as `EngineError::CompileError(<message>)` instead of being silently dropped.

### Cycle problem (why this section is more involved than B1)

`mock_config` already depends on `mock_core`. Adding `mock_core → mock_config` would create a circular workspace dependency that Cargo rejects. The fix moves the `Transform` data enum's *canonical home* to `mock_core`, where its transformer implementations already live. `mock_config::Transform` becomes a re-export from `mock_core`, preserving the public API while breaking the dependency cycle.

### Implementation outline

1. `crates/mock-core/src/transform/spec.rs` — new file. Defines the canonical `Transform` enum with `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]` and `#[serde(rename_all = "snake_case", tag = "type")]` (currently at `crates/mock-config/src/models.rs:130`). The `ReplaceBody` variant uses `mock_core::BodyData` directly — which means a single `BodyData` enum is canonical in `mock-core`, and `mock_config::BodyData` becomes a re-export from `mock_core` to preserve API compatibility (this also resolves the latent I8 finding from the review as a side effect). `mock-core` does not gain new dependencies; it uses its existing `serde_json::Value` and the `serde` features already enabled.
2. `crates/mock-core/src/body.rs` — apply `#[serde(rename_all = "snake_case")]` to `BodyData` (matching the user-facing config format and the existing `mock_config::BodyData` form). Three Rust call sites use `BodyData::Empty` / `BodyData::Text` / `BodyData::Json` / `BodyData::Binary` as variant constructors — those are unaffected by serde attributes. The serde tag change only affects serialized/deserialized strings; pick snake_case to align with the config docs and existing `mock-config` schema.
3. `crates/mock-config/src/models.rs` — `pub use mock_core::transform::spec::Transform;` and `pub use mock_core::body::BodyData;` (replace the local definitions). All existing `Transform::*` and `BodyData::*` references in mock-config and downstream crates continue to compile.
3. `crates/mock-core/src/engine.rs:564-598` — replace `compile_transforms` and the now-redundant `convert_transform` (engine.rs:677+) with:
   ```rust
   fn compile_transforms(action_val: &serde_json::Value) -> Result<(Vec<Box<dyn Transform>>, Vec<Box<dyn Transform>>), String> {
       let req_t = load_transforms(action_val.get("request_transforms"))?;
       let resp_t = load_transforms(action_val.get("response_transforms"))?;
       Ok((req_t, resp_t))
   }
   fn load_transforms(v: Option<&serde_json::Value>) -> Result<Vec<Box<dyn Transform>>, String> {
       match v {
           None | Some(serde_json::Value::Null) => Ok(Vec::new()),
           Some(arr) => {
               let specs: Vec<Transform> = serde_json::from_value(arr.clone())
                   .map_err(|e| format!("invalid transforms: {}", e))?;
               specs.into_iter().map(transform_from_spec).collect()
           }
       }
   }
   fn transform_from_spec(spec: Transform) -> Result<Box<dyn Transform>, String> { /* match spec -> new SetHeader etc */ }
   ```
4. The silent `.filter_map(...ok())` disappears — every transform must deserialize cleanly or compile fails.
5. **Backward compatibility for `examples/transforms.json`:** the example already uses `set_header`/`set_json_pointer` snake_case (per review). Verify during implementation; no edits expected.

### Test plan

- Existing transform unit tests in `transform/{headers,query,json,body,template}.rs` continue to pass (they test the post-load `Transform` trait methods, which are unchanged).
- Add `engine.rs::tests::test_transforms_set_header_in_route` that compiles a config with `request_transforms: [{type: "set_header", name: "X-Foo", value: "bar"}]` and asserts the header appears on the forwarded request.
- Add `engine.rs::tests::test_transforms_bad_variant_fails` that asserts `{type: "set_color", ...}` results in `EngineError::CompileError`.
- Add `engine.rs::tests::test_transforms_missing_required_field_fails` for `set_header` without `value`.

### Side effect on `transforms.json` example

After B2, the loader accepts `{type: "set_header", ...}` because the canonical `Transform::SetHeader` is snake_case-tagged. The example already uses this form (per the review); no change needed.

## B3: TUI double-loop

### Behavior change

`App::run` (`crates/mock-cli/src/tui/app.rs:34-82`) calls `run_loop` twice — once via `tokio::runtime::Handle::current().block_on(...)` (line 65) and once via direct `.await` (line 73). The first call is unreachable / dead code; the second is the real loop. After this fix, only one call exists.

### Implementation outline

1. Delete `app.rs:64-69` (the `block_on`-wrapped `run_loop` invocation and the `let res = ` reassignment).
2. Keep the cleanup lines (77-79).
3. Replace the panic-hook installation with a `std::sync::Once` so repeated `App::run` calls in tests or relaunched CLI instances do not nest hooks.

### Test plan

- `cargo test --workspace --all-features` for `mock-cli` should still pass.
- Manual smoke test (described in PR description; not a code-level test): start `mock-api run --config examples/basic.json` and confirm the TUI renders one frame, `q` quits cleanly, terminal state is restored.

## C1: CLI integration test bounded wait

### Behavior change

The CLI integration test at `crates/mock-cli/tests/integration.rs:189-197` currently spawns `mock-api run` and calls `.output()` on it, which blocks indefinitely. Replace with a bounded-wait approach that signals the child to exit and asserts the captured output within a deadline.

### Implementation outline

1. Replace `Command::new(...).output()` with `Command::spawn()`.
2. Capture stdout/stderr into pipes.
3. Poll a "ready" signal (e.g., `127.0.0.1:PORT` accepts connections) with a 5-second timeout using `std::sync::mpsc::channel` + `recv_timeout`.
4. On the readiness signal succeeding, send a `kill()` or `Child::kill()` to stop the server.
5. `child.wait()` for cleanup.
6. Assert the captured output within a second 5-second budget; fail the test if exceeded.

### Test plan

- Replace the test body with the new bounded-wait flow.
- A second test asserts that a malformed config (used by B4's existing `bad_engine_config` pattern) terminates within 1 second with a non-zero exit code and a clear error message.

---

## C2: Broadcast lag surfacing

### Behavior change

When a burst of runtime events arrives faster than the TUI consumes them, `broadcast` either drops the oldest events (returning `RecvError::Lagged`) or blocks new subscribers. The TUI must drain all immediately available events per render frame and surface lag as a counter — never silently lose events.

### Implementation outline

1. `crates/mock-runtime/src/runtime.rs:279-285` (`EventReceiver::recv_timeout`) — replace single-event receive with a `try_recv` loop:
   ```rust
   pub async fn drain(&mut self) -> (Vec<RuntimeEvent>, u64 /* lag_count */) {
       let mut out = Vec::new();
       let mut lag = 0;
       loop {
           match self.0.try_recv() {
               Ok(event) => out.push(event),
               Err(broadcast::error::TryRecvError::Lagged(n)) => lag += n,
               Err(broadcast::error::TryRecvError::Empty) => break,
               Err(broadcast::error::TryRecvError::Closed) => break,
           }
       }
       (out, lag)
   }
   ```
   Keep `recv_timeout` for callers that want a bounded wait; add a single-frame `drain_all_until_empty` helper that combines both (poll `try_recv` for up to `EVENT_POLL_INTERVAL_MS`, then return whatever buffered events remain).
2. `crates/mock-cli/src/tui/state.rs` — add a `pub lag_count: u64` field and increment from the TUI main loop.
3. `crates/mock-cli/src/tui/app.rs:96-102` — replace `recv_timeout` with `drain_all_until_empty`. On lag > 0, increment `state.lag_count` and log a `tracing::warn!`.
4. `crates/mock-cli/src/tui/widgets/status_bar.rs` — when `state.lag_count > 0`, render `Lagged: <n>` next to status. Cap the displayed count (e.g., "99+").

### Test plan

- Add a unit test for `EventReceiver::drain`: send 1000 events to a freshly-subscribed receiver, call `drain`, expect a non-zero `lag_count` and that subsequent drains return remaining events.
- Add a TUI smoke test that asserts `state.lag_count` is incremented after the burst.

### Note

`broadcast::channel(128)` buffer size is 128. Bursts > 128 events will always incur lag even with this fix; surfacing it is the only honest answer. Increasing the buffer is not done here because it only delays the problem and adds memory pressure.

---

## C4: Upstream redirect policy

### Behavior change

Reqwest's default redirect policy is unlimited; once B1 wires forward execution, an upstream that issues redirects can pivot a request to 127.0.0.1, 169.254.169.254, or any internal service. Forbid all redirects: relay the 3xx response to the client instead of chasing it.

### Implementation outline

1. `crates/mock-http/src/client.rs:20-22` — `UpstreamClient::new` (and `with_timeout`) build with `Client::builder().redirect(reqwest::redirect::Policy::none())`.
2. No further code change — reqwest's behavior on redirect with this policy is to *return* the 3xx response (the same as if the client were a browser). The mock then relays that response to the caller, who can decide what to do.

### Test plan

- Add a unit test that builds a tiny upstream server returning 301 + `Location: http://127.0.0.1:1/`, calls `UpstreamClient::send`, and asserts the returned `ResponseData.status` is the upstream's 301 — not a successful fetch of the redirect target.

---

## B4: Validation in `Engine::compile`

### Behavior change

`Engine::compile` calls `mock_config::parse_and_validate(config_json)` before doing anything else. Any `ValidationIssue` becomes an `EngineError::CompileError` with the issue's `code`, `path`, and `message` joined by `; `. The function no longer silently accepts invalid configs.

### Implementation outline

1. Inside `Engine::compile` (`crates/mock-core/src/engine.rs:116-161`), before the existing `serde_json::from_str(config_json)` step:
   - Call `let _config: mock_config::Config = mock_config::parse_and_validate(config_json).map_err(|e| EngineError::CompileError(e.to_string()))?;`
   - The existing logic continues to parse `config` as `serde_json::Value` for its raw manipulation (matchers, transforms, etc.). We don't reuse `_config` directly because the engine still needs raw access to fields the typed struct doesn't expose (e.g., the raw `match_rule` JSON for compile_match_rule's bespoke handling). The call's purpose is exclusively validation.
2. The pre-existing `convert_transform` is replaced by B2's path; nothing else about validate overlaps.

### Test plan

- Add `engine.rs::tests::test_compile_duplicate_route_id_fails`: two routes with `id: "get-user"` returns `EngineError::CompileError` with "DUPLICATE_ROUTE_ID" in the message.
- Add `test_compile_invalid_version_fails`: `version: 2` returns `EngineError::CompileError` containing "INVALID_VERSION".
- Add `test_compile_invalid_upstream_url_fails`: forward route with `upstream: "not-a-url"` → `CompileError` with "INVALID_UPSTREAM_URL".
- Update the existing `mock-runtime::tests::test_reload_bad_engine_preserves_old` (`runtime.rs:498-524`): the fixture `bad_engine_config` no longer causes Engine::compile to fail because `mock_config::validate` accepts `match_rule.method: null` (it's `Option<String>` and None is fine). Replace the fixture with one that intentionally fails `mock_config::validate` — e.g., duplicate route IDs — so the test still exercises the failure path through `parse_and_validate`.
- Update `mock-runtime::tests::test_reload_bad_engine_preserves_old`'s comment from "Config that passes mock_config validation but fails Engine::compile" to "Config that fails mock_config validation, surfaced through Engine::compile via B4 fix".

## Documentation updates

### `examples/*.json`

After B1+B2 work, verify each example:

- `examples/basic.json` — mock-only routes; B2 doesn't change behavior. No edit needed.
- `examples/proxy.json` — forward route with `request_transforms`. After B2 the transform runs; no JSON edit needed.
- `examples/transforms.json` — multiple transforms. After B2 they run. No edit needed.

The review surfaced `N9` (a `"value": "test-key-12345"` placeholder). Replace with `"value": "REPLACE_ME"` and add a top-level `// This file uses placeholder values — see README for details` comment is overkill. Use `"value": "<set-your-key-here>"` to make it visually obvious it's a placeholder without adding a comment to JSON (which isn't valid JSON anyway).

### `CLAUDE.md`

CLAUDE.md mentions Phase 7+ features that remain future work and references the dependency hierarchy. No changes needed for these bugs. The "Known limitations" mentioned in sub-agent reports are not in CLAUDE.md itself; the SDD progress ledger at `.superpowers/sdd/progress.md` is the right place to update if we want a paper trail.

### `.superpowers/sdd/progress.md`

After implementation, append a section noting the four bug fixes (commit SHAs once landed). This is documentation about completed work, not future work.

### `.superpowers/sdd/code-review-report.md`

No edit. The report describes findings as observed.

## End-to-end success criteria

1. `cargo fmt --all --check` clean.
2. `cargo test --workspace --all-features` green **and completes** (C1 ensures no test hangs forever).
3. `cargo clippy --workspace --all-targets --all-features` does not regress relative to current state for the crates we touch (mock-core, mock-http, mock-runtime, mock-cli, mock-wasm). Pre-existing clippy errors in mock-core remain out of scope per Non-Goals.
4. `examples/proxy.json` end-to-end: start `mock-api run --config examples/proxy.json` against an upstream that echoes the request method in the response body; `curl -X POST http://localhost:8080/api/data -d '{"a":1}'` returns the echoed response, not `102 Processing`.
5. `examples/transforms.json` end-to-end: confirms the `set_header` transform visibly modifies the upstream request (test that introspects reqwest on the upstream side).
6. TUI start: `mock-api run --config examples/basic.json --tui` renders one frame and quits cleanly on `q`.
7. **C2:** Under a synthetic burst of 200 runtime events arriving within one render-frame interval, the TUI's `state.lag_count` is ≥ 0 (never negative), the status bar shows `Lagged: <n>`, and no exception / panic occurs.
8. **C4:** A test upstream that responds with 301 + `Location: http://127.0.0.1:1/` causes the forwarded response to reach the client as the 301 status, not as a successful fetch from the redirect target.

## Out of scope (explicit)

- I-row (Important) findings 1–10 from `.superpowers/sdd/code-review-report.md`.
- C3 partial-body timeout; C5/C7 reload-and-update-limits; C6 reload-when-stopped; C8 file-watch debounce; C9 TUI watcher shutdown; C10 panic terminal restoration; C11 TUI input speed.
- N-row (Nit) findings 1–11.
- WASM opaque errors (WASM-side types need separate work).
- Path-template header injection (would need a deeper templating redesign).
- Pre-existing clippy errors across all crates.

## Open questions

None. The user pre-approved all design choices:

1. Validation: push `validate()` into `Engine::compile`.
2. Transforms: re-host canonical `Transform` in `mock_core::transform::spec` (resolved the Cargo cycle).
3. Forward execution: `UpstreamClient::send` in `handle_route`.
4. Forward errors: HTTP 502.
5. **C2 Lag handling:** surface `Lagged: <n>` counter in status bar.
6. **C4 SSRF:** `reqwest::redirect::Policy::none()` — relay 3xx responses.
7. **C1 bounded wait:** kill-on-timeout pattern for the hanging CLI test.
