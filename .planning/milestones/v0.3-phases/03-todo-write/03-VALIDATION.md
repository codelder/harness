---
phase: 03
slug: todo-write
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 03 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Built-in Rust tests + tokio::test |
| **Config file** | none — Cargo.toml already configured |
| **Quick run command** | `cargo test todo --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test todo --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | PLAN-01 | unit | `cargo test todo_manager --lib` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | PLAN-01 | unit | `cargo test todo_status --lib` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | PLAN-01 | unit | `cargo test todo_tool --lib` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 1 | PLAN-01 | unit | `cargo test todo_constraints --lib` | ❌ W0 | ⬜ pending |
| 03-03-01 | 03 | 2 | PLAN-01 | unit | `cargo test nag_reminder --lib` | ❌ W0 | ⬜ pending |
| 03-03-02 | 03 | 2 | PLAN-01 | unit | `cargo test session_todo --lib` | ❌ W0 | ⬜ pending |
| 03-04-01 | 04 | 2 | PLAN-01 | integration | `cargo test --lib` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/planning/todo.rs` — TodoManager implementation with inline tests
- [ ] `src/planning/mod.rs` — Module exports for planning types
- [ ] `src/tools/todo.rs` — TodoTool implementation with inline tests
- [ ] Update `src/tools/mod.rs` — Add todo module export
- [ ] Update `src/cli/session.rs` — Add todo_manager and rounds_since_todo fields
- [ ] Update `src/llm/provider.rs` — Add TodoTool to AgentBuilder chains

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end agent session with todos | PLAN-01 | Requires live LLM API and interactive session | 1. Run `cargo run` 2. Ask agent to track tasks 3. Verify nag reminder appears after 3 rounds 4. Verify todo tool updates persist |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
