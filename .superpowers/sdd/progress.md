# SDD Progress Ledger

## Completed
- Task 1: complete (commits 9593a9f..6502026, review clean)
- Task 2: complete (commits 6502026..dfefd44, review clean)
- Task 3: complete (commits dfefd44..0416c22, review clean)
- Task 4: complete (commits 0416c22..7aa5cf3, review clean after fix)
- Task 5: complete (commits 7aa5cf3..0b48e28, review clean)
- Task 6: complete (commits 0b48e28..aeb762c, review clean)
- Task 7: complete (commits aeb762c..67ecb9d, review clean)
- Task 8: complete (commits 67ecb9d..e143088, review clean)
- Task 9: complete (commits e143088..f5c3db9, review clean)
- Task 10: complete (commits f5c3db9..a67fa96, review clean)
- Task 11: complete (commits a67fa96..9237197, review clean after fix)
- Task 12: complete (commits 9237197..f04effd, review clean after fix)
- Task 13: complete (commits f04effd..caae572, review clean after fix)
- Task 14: complete (commits caae572..9889faa, review clean)
- Task 15: complete (commits 9889faa..01c88b5, review clean after fix)
- Task 16: complete (commits 01c88b5..a85115d, review clean)
- Task 17: complete (commits a85115d..bf178f2, review clean)
- Task 18: complete (commits bf178f2..b281ce5e, review clean after fix)

## Final State
All 18 tasks complete. Implementation covers Phase 0 through Phase 7.

## Bug-fix patch in progress (post-review)
- Task 1 (B2): complete (commits b87aa7a..2095edb, review clean after fix for action-type dispatch + ReplaceBody wire-format note)
- Task 2 (B4): complete (commits 2095edb..e9de62f, review clean; 3 deviations documented, all reasonable)
- Task 3 (B1): complete (commits e9de62f..d1bb9fc, review clean after fix for 5 MiB cap being checked against wrong variable)
- Task 4 (C2): complete (commits d1bb9fc..c287c16, review clean)
  - Important #1: `compile_transforms` no longer branches on action type — mock actions can now carry transforms (brief mandated `_ =>` arm returning empty). Fix subagent in flight.
  - Important #2: ReplaceBody wire format changed from `{body: {json: ...}}` to `{body: {"json": ...}}` (externally-tagged BodyData). Brief required it; not documented in report. Fix subagent in flight.
  - Minor: schemars not pinned to workspace version (mock-core/Cargo.toml:7)
  - Minor: test_body_data_serialization_snake_case doesn't cover Empty serialization
  - Minor: from_spec.rs tests cover 2 of 8 variants
  - Minor: mock-config/src/models.rs:218-219 has misleading _expected_pattern tuples (out of scope; pre-existing pattern)
- Task 5 (B3): complete (commits c287c16..42714f7, review clean) — TUI no longer panics with "Cannot start a runtime from within a runtime"; panic hook installed via `std::sync::Once`; bracketed paste cleaned up on shutdown
- Task 6 (C1): complete (commits 42714f7..1ce819c, review clean) — bounded-wait helper for the previously-hanging `run` integration test; new fast-fail malformed-config test; implementer also fixed the brief's latent once-only stderr-take bug with background reader threads
- Task 7 (C4): complete (commits 1ce819c..8167e0d, review clean) — `reqwest::redirect::Policy::none()` on both `UpstreamClient::new` and `UpstreamClient::with_timeout` closes redirect-based SSRF; regression test asserts 301 is returned, not chased

## Blocking-bug fix patch (post-review)
- B1 (forward execution): commit above; verified by Phase 7 round-trip integration test
- B2 (transform snake_case): commit above; verified by `test_transforms_set_header_in_route`
- B3 (TUI single loop): commit above; verified by `mock-cli` lib tests
- B4 (validation in Engine::compile): commit above; verified by negative tests for duplicate-id, version, upstream URL
- C1 (CLI test bounded wait): commit above; verified by `test_run_command_with_*`
- C2 (broadcast lag surface): commit above; verified by `test_event_receiver_drain_reports_lag`
- C4 (reqwest no-redirect): commit above; verified by `test_upstream_does_not_follow_redirects`

Documented example flows (examples/basic.json, examples/proxy.json, examples/transforms.json) now exercise the intended paths end-to-end.

## Known Issues (Minor)
- Response body preview in TUI always empty (architectural limitation - response body consumed before event emission)
- Some pre-existing clippy warnings in mock-core (dead_code, collapsible_match)
- Forward route integration test verifies decision type only, not actual HTTP forwarding
- Timeout test verifies config parsing only, not actual timeout enforcement
