# Roadmap: Rust Agent Harness

## Milestones

- ✅ **v0.1 Foundation** — Phase 1 (shipped 2026-03-22)
- ✅ **v0.2 Tool Use** — Phase 2 (shipped 2026-03-23)
- ✅ **v0.3 TodoWrite** — Phases 3-3.1 (shipped 2026-03-24)
- 📋 **v0.4 Subagents** — Phase 4 (planned)
- 📋 **v1.0 Core** — Phases 5-8 (planned)
- 📋 **v2.0 Teams** — Phases 9-12 (planned)

## Phases

<details>
<summary>✅ v0.1 Foundation (Phase 1) — SHIPPED 2026-03-22</summary>

- [x] **Phase 1: s01 - Agent Loop** — Core agent loop with stop_reason handling (4/4 plans)

**Key deliverables:** Agent loop, multi-provider support, error classification, CLI interface

See: [v0.1-ROADMAP.md](milestones/v0.1-ROADMAP.md)

</details>

<details>
<summary>✅ v0.2 Tool Use (Phase 2) — SHIPPED 2026-03-23</summary>

- [x] **Phase 2: s02 - Tool Use** — Tool dispatch system and basic file/shell tools (3/3 plans)

**Key deliverables:** 6 tools (Bash, Read, Write, Edit, Glob, Grep), safety blacklist, multi-provider registration

See: [v0.2-ROADMAP.md](milestones/v0.2-ROADMAP.md)

</details>

<details>
<summary>✅ v0.3 TodoWrite (Phases 3-3.1) — SHIPPED 2026-03-24</summary>

- [x] **Phase 3: s03 - TodoWrite** — Task planning with nag reminders (4/4 plans)
- [x] **Phase 3.1: Code Review Fixes** — Async mutex, typed API, validation (1/1 plan)

**Key deliverables:** TodoManager state, TodoTool, nag reminder system, code review fixes

See: [v0.3-ROADMAP.md](milestones/v0.3-ROADMAP.md)

</details>

### ⚡ Inserted Work (Planned)

- [ ] **Phase 3.2: Terminal UI and Frontend Interface Abstraction (INSERTED)** — Replace the split-pane ratatui workbench with a single-column Claude Code-like terminal UI and repair the shared frontend protocol for future adapters
- [ ] **Phase 3.2.1: TUI Alignment with Claude Code (INSERTED)** — 将 TUI 体验与 Claude Code CLI 全面对齐：Viewport::Inline、颜色系统、Spinner 动画、简化消息格式、增强 Banner/Todo/Statusline

### 📋 v0.4 Subagents (Planned)

- [ ] **Phase 4: s04 - Subagents** — Subagent spawning with isolated context

### Phase 3.2: Terminal UI and Frontend Interface Abstraction (INSERTED)

**Goal**: Replace the split-pane ratatui workbench with a Claude Code-like single-column terminal UI and a corrected shared frontend protocol that can later support CLI, web, desktop, and social adapters.
**Depends on**: Phase 3.1
**Requirements**: SC-1, SC-2, SC-3, SC-4, SC-5
**Requirement Map**:

  - `SC-1` Single-column Claude Code-like terminal layout with compact empty state
  - `SC-2` Headless runtime remains independent from CLI-specific I/O
  - `SC-3` Shared protocol represents streaming, tool, retry, interrupt, and lifecycle events over typed channels
  - `SC-4` Tool results stay paired with their calls and todo state updates in place as a pinned footer
  - `SC-5` Overflow, resize, exit, and panic/error paths keep content visible and restore the terminal cleanly
**Success Criteria** (what must be TRUE):

  1. The interactive terminal experience uses a single-column Claude Code-like layout with banner, unified timeline, composer, and status
  2. The agent loop and session runtime no longer depend directly on CLI-specific input/output primitives
  3. A shared frontend/session protocol can represent streaming output, tool activity, interrupts, lifecycle events, and event identity
  4. Tool call/result groups stay paired in the unified stream and the todo list stays pinned at the bottom with in-place updates
  5. Overflowing content remains reachable through unified scroll/viewport behavior and terminal exit/error paths restore the shell cleanly

**Plans**: 3 plans in 3 waves

Plans:
- [x] 3.2-01-PLAN.md — Repair the frontend protocol for turn identity, tool correlation, and todo snapshots
- [x] 3.2-02-PLAN.md — Wire the headless runtime and PromptHook path to emit correlated unified-timeline events
- [ ] 3.2-03-PLAN.md — Rebuild the ratatui CLI as a single-column timeline with overflow and lifecycle regression coverage

### Phase 3.2.1: TUI Alignment with Claude Code (INSERTED)

**Goal:** 将 TUI 体验与 Claude Code CLI 全面对齐 — 使用 Viewport::Inline 替代 alternate screen，添加颜色系统、Spinner 动画、简化消息格式、增强 Banner/Todo/Statusline
**Requirements**: REQ-3.2.1-01 ~ REQ-3.2.1-07
**Depends on:** Phase 3.2
**Success Criteria** (what must be TRUE):

  1. 使用 Viewport::Inline 模式，退出后对话历史保留在终端 scrollback
  2. 完整的语义化颜色系统，不同消息类型有不同颜色
  3. ASCII 花朵 Spinner 动画 + 趣味状态文本
  4. 简化的消息格式（去掉冗长的 [turn_id]）
  5. 多行 Banner 信息面板（版本/模型/路径）
  6. Todo 完成时自动收起 + 写入历史
  7. Statusline 显示 token 上下文

**Plans:** 7 plans (0/7 complete)

Plans:
- [ ] 3.2.1-01-PLAN.md — 切换到 Inline Viewport 模式
- [ ] 3.2.1-02-PLAN.md — 添加颜色系统和主题
- [ ] 3.2.1-03-PLAN.md — Spinner 动画和趣味状态
- [ ] 3.2.1-04-PLAN.md — 简化消息格式
- [ ] 3.2.1-05-PLAN.md — 增强 Banner 信息面板
- [ ] 3.2.1-06-PLAN.md — 增强 Todo 显示（自动收起+写入历史）
- [ ] 3.2.1-07-PLAN.md — 增强 Statusline（token 上下文）

### Phase 4: s04 - Subagents

**Goal**: Agent can delegate subtasks to isolated child contexts
**Depends on**: Phase 3
**Requirements**: PLAN-02, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Agent can spawn child agents with fresh message arrays
  2. Child agents execute independently without polluting parent context
  3. Parent receives summarized results when child completes
  4. Child agent errors are handled gracefully without crashing parent

**Plans**: TBD

### 📋 v1.0 Core (Planned)

- [ ] **Phase 5: s05 - Skills** — On-demand skill loading via tool_result
- [ ] **Phase 6: s06 - Context Compact** — Three-layer context compression
- [ ] **Phase 7: s07 - Tasks** — File-based task graph with dependencies
- [ ] **Phase 8: s08 - Background Tasks** — Background execution and notifications

### 📋 v2.0 Teams (Planned)

- [ ] **Phase 9: s09 - Agent Teams** — Multi-agent teams with JSONL mailboxes
- [ ] **Phase 10: s10 - Team Protocols** — Structured team communication patterns
- [ ] **Phase 11: s11 - Autonomous Agents** — Idle agents claim available tasks
- [ ] **Phase 12: s12 - Worktree Isolation** — Git worktree isolation per task

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. s01 - Agent Loop | v0.1 | 4/4 | Complete | 2026-03-22 |
| 2. s02 - Tool Use | v0.2 | 3/3 | Complete | 2026-03-23 |
| 3. s03 - TodoWrite | v0.3 | 4/4 | Complete | 2026-03-24 |
| 3.1 Code Review Fixes | v0.3 | 1/1 | Complete | 2026-03-24 |
| 3.2 Terminal UI + Frontend Interface | v0.4 | 2/3 | In Progress | - |
| 4. s04 - Subagents | v0.4 | 0/TBD | Not started | - |
| 5. s05 - Skills | v1.0 | 0/TBD | Not started | - |
| 6. s06 - Context Compact | v1.0 | 0/TBD | Not started | - |
| 7. s07 - Tasks | v1.0 | 0/TBD | Not started | - |
| 8. s08 - Background Tasks | v1.0 | 0/TBD | Not started | - |
| 9. s09 - Agent Teams | v2.0 | 0/TBD | Not started | - |
| 10. s10 - Team Protocols | v2.0 | 0/TBD | Not started | - |
| 11. s11 - Autonomous Agents | v2.0 | 0/TBD | Not started | - |
| 12. s12 - Worktree Isolation | v2.0 | 0/TBD | Not started | - |
