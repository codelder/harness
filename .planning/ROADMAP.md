# Roadmap: Rust Agent Harness

## Milestones

- ✅ **v0.1 Foundation** — Phase 1 (shipped 2026-03-22)
- ✅ **v0.2 Tool Use** — Phase 2 (shipped 2026-03-23)
- 🚧 **v0.3 TodoWrite** — Phase 3 + 03.1 code review fixes (in progress)
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

### 🚧 v0.3 TodoWrite (In Progress)

- [x] **Phase 3: s03 - TodoWrite** — Task planning with nag reminders (4/4 plans) ✅ 2026-03-24
- [ ] **Phase 03.1: Code Review Fixes** — Address issues from cross-AI review (0/1 plans)

Plans:
- [x] 03-01-PLAN.md — TodoManager core state management with constraint validation
- [x] 03-02-PLAN.md — TodoTool with Arc<Mutex<TodoManager>> for shared state
- [x] 03-03-PLAN.md — Session integration with nag reminder
- [x] 03-04-PLAN.md — Provider wiring and round counter reset
- [ ] 03.1-01-PLAN.md — Fix async mutex, error handling, API consistency issues

### 📋 v0.4 Subagents (Planned)

- [ ] **Phase 4: s04 - Subagents** — Subagent spawning with isolated context

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

## Phase Details

### Phase 3: s03 - TodoWrite ✅

**Goal**: Agent maintains persistent task list that prevents drift
**Depends on**: Phase 2
**Requirements**: PLAN-01, CROSS-02, CROSS-03, CROSS-04
**Reference**: [s03-todo-write.md](https://github.com/shareAI-lab/learn-claude-code/blob/main/docs/en/s03-todo-write.md)
**Success Criteria** (what must be TRUE):

  1. Agent can create, update, and delete tasks in a task list
  2. Agent receives nag reminders about pending tasks during loops (3+ rounds without todo call)
  3. Task state persists across agent turns
  4. Agent can mark tasks complete and track progress
  5. Only one task can be in_progress at a time (enforced constraint)
  6. Maximum 20 todos enforced (prevents context explosion)
  7. TodoTool returns formatted string via render() method

**Plans**: 4 plans in 3 waves — ✅ COMPLETED 2026-03-24

### Phase 03.1: Code Review Fixes

**Goal**: Fix issues identified in cross-AI code review
**Depends on**: Phase 3
**Requirements**: (none - quality improvement)
**Success Criteria** (what must be TRUE):

  1. `std::sync::Mutex` replaced with `tokio::sync::Mutex` in async paths
  2. Lock failures use dedicated error variant (not InvalidStatus)
  3. Session logs lock failures instead of silently suppressing
  4. TodoItemInput.status is typed (TodoStatus, not String)
  5. Provider chat API unified (remove Vec<String> variant)
  6. ID range and uniqueness validated

**Plans**: 1 plan

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

### Phase 5: s05 - Skills

**Goal**: Agent loads domain expertise on-demand to reduce context bloat
**Depends on**: Phase 4
**Requirements**: PLAN-03, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Agent knows available skill names without loading content
  2. Agent can request skill content which is injected via tool_result
  3. Skill content is not stored in persistent context (loaded on-demand)
  4. Multiple skills can be loaded in a single session
**Plans**: TBD

### Phase 6: s06 - Context Compact

**Goal**: Agent sessions can run indefinitely without context overflow
**Depends on**: Phase 5
**Requirements**: PLAN-04, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Old messages are automatically summarized when approaching token limits
  2. Verbose tool outputs are truncated to essential information
  3. Summarized context maintains conversation coherence
  4. Agent can continue working after compression without losing critical state
**Plans**: TBD

### Phase 7: s07 - Tasks

**Goal**: Agent goals persist across sessions with dependency tracking
**Depends on**: Phase 6
**Requirements**: PERS-01, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Tasks are stored in file-based JSONL format
  2. Tasks have dependency edges that define execution order
  3. Agent can resume work on tasks after session restart
  4. Task graph can be queried for available (unblocked) work
**Plans**: TBD

### Phase 8: s08 - Background Tasks

**Goal**: Long-running operations don't block agent loop
**Depends on**: Phase 7
**Requirements**: PERS-02, PERS-03, PERS-04 (complete), CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Agent can spawn background daemon threads for long operations
  2. Background task completion injects notification into agent context
  3. Agent can check status of running background tasks
  4. Sandbox isolation prevents unauthorized system access
**Plans**: TBD

### Phase 9: s09 - Agent Teams

**Goal**: Multiple specialized agents can coordinate asynchronously
**Depends on**: Phase 8
**Requirements**: TEAM-01, TEAM-02, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Team registry maintains persistent teammates with capabilities
  2. Agents communicate via async JSONL file-based mailboxes
  3. Agents can send and receive messages without blocking
  4. Team membership persists across sessions
**Plans**: TBD

### Phase 10: s10 - Team Protocols

**Goal**: Teams follow structured communication patterns for coordination
**Depends on**: Phase 9
**Requirements**: TEAM-03, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Shutdown protocol uses FSM to coordinate graceful termination
  2. Plan approval protocol enables team consensus on approach
  3. Protocol state machines are observable and debuggable
  4. Protocols handle edge cases (timeouts, failures, disagreements)
**Plans**: TBD

### Phase 11: s11 - Autonomous Agents

**Goal**: Idle agents self-assign work from shared task board
**Depends on**: Phase 10
**Requirements**: TEAM-04, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Idle agents periodically scan task board for available work
  2. Agents claim tasks atomically without conflicts
  3. Claimed tasks are visible to other agents to prevent duplicate work
  4. Agents release claims on failure or completion
**Plans**: TBD

### Phase 12: s12 - Worktree Isolation

**Goal**: Parallel agent work doesn't cause file conflicts
**Depends on**: Phase 11
**Requirements**: TEAM-05, TEAM-06, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Each task gets isolated git worktree for execution
  2. Tasks are bound to worktrees by unique identifier
  3. Multiple agents can work on different tasks simultaneously
  4. Worktree changes can be merged back to main branch
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. s01 - Agent Loop | v0.1 | 4/4 | Complete | 2026-03-21 |
| 2. s02 - Tool Use | v0.2 | 3/3 | Complete | 2026-03-23 |
| 3. s03 - TodoWrite | v0.3 | 4/4 | Complete | 2026-03-24 |
| 3.1 Code Review Fixes | v0.3 | 0/1 | In Progress | - |
| 4. s04 - Subagents | v0.4 | 0/TBD | Not started | - |
| 5. s05 - Skills | v1.0 | 0/TBD | Not started | - |
| 6. s06 - Context Compact | v1.0 | 0/TBD | Not started | - |
| 7. s07 - Tasks | v1.0 | 0/TBD | Not started | - |
| 8. s08 - Background Tasks | v1.0 | 0/TBD | Not started | - |
| 9. s09 - Agent Teams | v2.0 | 0/TBD | Not started | - |
| 10. s10 - Team Protocols | v2.0 | 0/TBD | Not started | - |
| 11. s11 - Autonomous Agents | v2.0 | 0/TBD | Not started | - |
| 12. s12 - Worktree Isolation | v2.0 | 0/TBD | Not started | - |
