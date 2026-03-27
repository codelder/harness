---
gsd_state_version: 1.0
milestone: v0.4
milestone_name: Subagents
status: Ready to execute
stopped_at: Completed 03.2.1-08-PLAN.md
last_updated: "2026-03-27T18:55:39.224Z"
last_activity: 2026-03-27
progress:
  total_phases: 3
  completed_phases: 2
  total_plans: 11
  completed_plans: 11
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-24)

**Core value:** Performance and safety without sacrificing capability
**Current focus:** Phase 03.2.1 — tui-alignment-with-claude-code

## Milestone History

| Milestone | Status | Phases | Date |
|-----------|--------|--------|------|
| v0.1 Foundation | ✅ Shipped | 1 | 2026-03-22 |
| v0.2 Tool Use | ✅ Shipped | 1 | 2026-03-23 |
| v0.3 TodoWrite | ✅ Shipped | 2 | 2026-03-24 |

## Current Position

Phase: 03.2.1 (tui-alignment-with-claude-code) — EXECUTING
Plan: 3 of 7

## Accumulated Context

### Roadmap Evolution

- Phase 3.2 inserted after Phase 3.1: Replace reedline CLI UI with ratatui and add a unified frontend interface for CLI/Web/Desktop/Social channels (URGENT)
- Phase 3.2.1 inserted after Phase 3.2: TUI Alignment with Claude Code — 使用 Viewport::Inline 替代 alternate screen，添加颜色系统、Spinner 动画、简化消息格式等 (URGENT)

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.

- [Phase 03.2]: Keep the reedline CLI as a thin wrapper over SessionRuntime during migration. — This keeps current terminal behavior working while moving session lifecycle ownership behind a reusable runtime boundary for later ratatui and non-terminal adapters.
- [Phase 03.2]: Use tokio::sync::mpsc with must-deliver versus best-effort frontend event classes at the runtime boundary. — Adapters need a bounded channel contract that preserves lifecycle and transcript-completion events while allowing high-frequency status traffic to coalesce without blocking the runtime.
- [Phase 03.2]: Extend TodoUsageHook with a FrontendEventSender side channel instead of introducing a second hook type. — This preserves direct todo detection semantics and PromptHook integration while giving runtime/frontends a structured event escape hatch.
- [Phase 03.2]: Move retry reporting into a notifier callback passed to with_retry. — The retry helper now stays terminal-agnostic while still surfacing retry metadata to any frontend that listens to FrontendEventSender.
- [Phase 03.2]: Use a reducer-style CliApp over FrontendEvent and FrontendCommand instead of embedding UI state inside SessionRuntime. — This keeps rendering and keyboard behavior testable without a live terminal while preserving the shared runtime boundary for future non-terminal adapters.
- [Phase 03.2]: Centralize raw-mode and alternate-screen ownership in TerminalGuard with a dedicated crossterm input listener task. — This keeps terminal lifecycle restoration explicit and limits terminal side effects to one module instead of scattering them across main/session code.
- [Phase 03.2]: Favor protocol and lifecycle integration tests over brittle TUI snapshot assertions. — The frontend contract is the stable architectural boundary; testing it directly gives better regression coverage while allowing the visual layout to evolve.
- [Phase 03.2]: Use a non-interactive harness --help smoke path for startup verification. — This verifies the binary still boots through the migrated CLI stack without requiring a live model call or interactive terminal session in CI.
- [Phase 03.2]: Keep SESSION_START_TURN_ID as the explicit reserved identity for session-start todo snapshots and default hook construction.
- [Phase 03.2]: Keep ToolCallFinished.name alongside turn_id and call_id so reducers never need a side lookup to label tool results.
- [Phase 03.2]: Represent todo footer state as TodoSnapshot emitted from the runtime instead of parsing reminder text.
- [Phase 03.2]: Emit TodoSnapshot only when TodoManager state changes or reminders need the pinned footer, so tool observation alone does not churn the reducer.
- [Phase 03.2]: Align hook-focused test names with cargo test runtime --lib so the plan verification command exercises PromptHook observability.
- [Phase 03.2]: Keep flush_best_effort() ownership in run_ui_loop so runtime emission and UI draining stay separate.
- [Phase 03.2]: Track raw-mode and alternate-screen state explicitly in TerminalGuard so every partial-construction failure path can roll back safely.
- [Phase 03.2]: Use source-included terminal smoke helpers in integration tests to verify restore behavior without a live TTY or model call.
- [Phase 03.2.1]: Token usage data available in rig-core 0.31.0 via completion::Usage but wiring deferred - requires refactoring LlmProvider, AgentTurn, and agent_loop to propagate PromptResponse.total_usage
- [Phase 03.2.1]: Use extended_details() on PromptRequest to get PromptResponse with total_usage for token tracking in agent pipeline

### Pending Todos

None.

### Blockers/Concerns

None.

## Session Continuity

Last activity: 2026-03-27
Last session: 2026-03-27T18:55:39.222Z
Stopped At: Completed 03.2.1-08-PLAN.md

## Performance Metrics

| Phase | Plan | Duration | Tasks | Files | Date |
|-------|------|----------|-------|-------|------|
| 03.2 | 02 | 11min | 2 | 3 | 2026-03-26 |
| Phase 03.2 P03 | 22min | 3 tasks | 4 files |
| Phase 03.2.1 P07 | 5min | 5 tasks | 3 files |
| Phase 03.2.1 P08 | 4min | 2 tasks | 4 files |
