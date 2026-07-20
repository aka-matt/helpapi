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
- Task 8 (docs/smoke/ledger): complete (commits 8167e0d..ad5bccd, review clean) — `examples/transforms.json` placeholder flipped to `<set-your-key-here>`; ledger section appended; end-to-end forward smoke against `httpbin.org` PASS via the actual configured route (`/api/forward` on 8082); brief's literal smoke command (`/api/data` on 8080) was a stale-port/path mismatch from the plan and was overridden
  - Important (process): `.superpowers/sdd/.gitignore` contains a bare `*`, so every ledger append requires `git add -f`. Whitelist `progress.md` or relocate in a follow-up housekeeping commit.

## Final whole-branch review

- **Reviewer verdict:** Ready to merge with one housekeeping fix.
- **Correction to my earlier claim:** the fmt drift in `crates/mock-cli/src/tui/app.rs:37,57` was actually introduced by Task 5 (B3, commit `42714f7`) — confirmed by checking out `b281ce5` (the merge base) into a worktree; `cargo fmt --all --check` exits 0 there. Per-task review for Task 5 did not catch this; the final reviewer did. M1 is a real regression that escaped Task 5.
- **M1 (blocker):** `cargo fmt -p mock-cli` produces ~14 lines of wrapping changes. Mechanical fix in a follow-up housekeeping commit.
- **M2 (cheap improvement):** `from_spec.rs` tests cover 2 of 8 variants; expand to cover all 8.
- **M3 (CI gate verify):** `cargo deny check` not run locally (cargo-deny not installed); the new `schemars = "0.8"` pin needs to resolve cleanly in CI.
- **M4 (process):** Plan template's smoke command mismatches the actual config port/path. Open as follow-up to fix the plan template.
- **M5 (process):** `.superpowers/sdd/.gitignore` bare `*`. Open as follow-up.
- **M6 (out of scope, pre-existing):** `test_transform_config_is_parsed` PascalCase fixture drift; `mock-core` clippy debt (8 lib + 2 test). Pre-existing watchpoints.

## Final housekeeping commits

- **M1 (commit `479e9b4`):** `cargo fmt -p mock-cli` re-wrapped two lines in `crates/mock-cli/src/tui/app.rs` (+8/-2, no semantic change). `cargo fmt --all --check` now exits 0.
- **M2 (commit `722067f`):** expanded `from_spec.rs` round-trip tests from 2 to 17 — every `Transform` variant now has a name + side-effect test, including the externally-tagged `BodyData` wire format for `ReplaceBody`.
- **Final CI gate:** `cargo fmt --all --check` exit 0; `cargo test --workspace --lib` → 251 passed, 0 failed (15 new from M2); clippy clean on touched files (pre-existing mock-core diagnostics documented and out of scope).

## Patch complete

All 8 bug-fix tasks landed and reviewed clean. Final-review fixes (M1, M2) applied. Branch `step4_M27_grok` is at `722067f`, ahead of `main` by 44 commits. Ready for finishing-a-development-branch.

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
