---
phase: 1
slug: s01-agent-loop
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test framework) |
| **Config file** | Cargo.toml (test profiles) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --all` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test --all`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | CORE-01 | unit | `cargo test agent_loop` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | CORE-05 | unit | `cargo test provider` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | CORE-06 | unit | `cargo test cli` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 2 | CROSS-02 | unit | `cargo test error_classification` | ❌ W0 | ⬜ pending |
| 01-03-02 | 03 | 2 | CROSS-03 | unit | `cargo test graceful_degradation` | ❌ W0 | ⬜ pending |
| 01-04-01 | 04 | 2 | CROSS-04 | unit | `cargo test logging` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/agent/mod.rs` — agent module structure
- [ ] `src/agent/loop.rs` — agent loop implementation
- [ ] `src/llm/mod.rs` — LLM provider abstraction
- [ ] `src/llm/provider.rs` — provider trait and implementations
- [ ] `src/cli/mod.rs` — CLI module
- [ ] `src/error.rs` — error types with classification
- [ ] `tests/agent_loop_test.rs` — integration tests for agent loop
- [ ] `tests/provider_test.rs` — integration tests for providers

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Interactive CLI session | CORE-06 | Requires real terminal I/O | Run `cargo run`, type message, verify streaming response |
| Ctrl+C graceful exit | CORE-06 | Requires signal handling | Run CLI, press Ctrl+C, verify clean exit with summary |
| Real API connectivity | CORE-05 | Requires valid API keys | Set API key, run CLI with `--provider anthropic`, verify response |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
