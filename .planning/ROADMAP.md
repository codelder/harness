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

- [x] **Phase 3.2: Terminal UI and Frontend Interface Abstraction (INSERTED)** — Replace reedline with ratatui and define a shared interface for CLI, web, desktop, and social frontends

### 📋 v0.4 Subagents (Planned)

- [ ] **Phase 4: s04 - Subagents** — Subagent spawning with isolated context

### Phase 3.2: Terminal UI and Frontend Interface Abstraction (INSERTED)

**Goal**: Replace the reedline-based REPL with a ratatui-driven terminal experience and introduce a shared frontend session interface that can connect the agent loop to CLI, web, desktop, and social surfaces.
**Depends on**: Phase 3.1
**Requirements**: TBD
**Success Criteria** (what must be TRUE):

  1. The interactive terminal experience runs on ratatui instead of reedline
  2. The agent loop no longer depends directly on CLI-specific input/output primitives
  3. A shared frontend/session interface can drive the existing CLI and future web, desktop, or social adapters
  4. Streaming model output, tool activity, and user interrupts can be represented through the shared interface
  5. Existing session lifecycle behavior remains functional during the migration

**Plans**: 4 plans in 4 waves

Plans:
- [x] 3.2-01-PLAN.md — Extract shared runtime and typed frontend protocol
- [x] 3.2-02-PLAN.md — Refactor hook/retry observability into structured events
- [x] 3.2-03-PLAN.md — Implement ratatui CLI adapter and main boot integration
- [x] 3.2-04-PLAN.md — Add migration verification, cleanup, and regression coverage

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
| 3.2 Terminal UI + Frontend Interface | v0.4 | 4/4 | Complete | 2026-03-25 |
| 4. s04 - Subagents | v0.4 | 0/TBD | Not started | - |
| 5. s05 - Skills | v1.0 | 0/TBD | Not started | - |
| 6. s06 - Context Compact | v1.0 | 0/TBD | Not started | - |
| 7. s07 - Tasks | v1.0 | 0/TBD | Not started | - |
| 8. s08 - Background Tasks | v1.0 | 0/TBD | Not started | - |
| 9. s09 - Agent Teams | v2.0 | 0/TBD | Not started | - |
| 10. s10 - Team Protocols | v2.0 | 0/TBD | Not started | - |
| 11. s11 - Autonomous Agents | v2.0 | 0/TBD | Not started | - |
| 12. s12 - Worktree Isolation | v2.0 | 0/TBD | Not started | - |
