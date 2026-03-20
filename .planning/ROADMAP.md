# Roadmap: Rust Agent Harness

## Overview

This roadmap transforms the 12-session learn-claude-code curriculum into a phased delivery plan, building a production-ready Rust agent harness from the core loop to multi-agent teams. Each phase corresponds to one curriculum session (s01-s12), delivering incremental capabilities that compound into a complete agent infrastructure.

## Phases

**Phase Numbering:**

- Integer phases (1-12): Planned milestone work aligned with curriculum sessions
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

- [ ] **Phase 1: s01 - Agent Loop** - Core agent loop with stop_reason handling
- [ ] **Phase 2: s02 - Tool Use** - Tool dispatch system and basic file/shell tools
- [ ] **Phase 3: s03 - TodoWrite** - Task planning with nag reminders
- [ ] **Phase 4: s04 - Subagents** - Subagent spawning with isolated context
- [ ] **Phase 5: s05 - Skills** - On-demand skill loading via tool_result
- [ ] **Phase 6: s06 - Context Compact** - Three-layer context compression
- [ ] **Phase 7: s07 - Tasks** - File-based task graph with dependencies
- [ ] **Phase 8: s08 - Background Tasks** - Background execution and notifications
- [ ] **Phase 9: s09 - Agent Teams** - Multi-agent teams with JSONL mailboxes
- [ ] **Phase 10: s10 - Team Protocols** - Structured team communication patterns
- [ ] **Phase 11: s11 - Autonomous Agents** - Idle agents claim available tasks
- [ ] **Phase 12: s12 - Worktree Isolation** - Git worktree isolation per task

## Phase Details

### Phase 1: s01 - Agent Loop

**Goal**: LLM becomes an agent through core loop that processes responses until text output
**Depends on**: Nothing (first phase)
**Requirements**: CORE-01, CORE-05 (partial), CORE-06, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Agent can process LLM responses and distinguish text from tool_use
  2. Agent loops until receiving text response (stop_reason handling)
  3. Agent can connect to at least one LLM provider (Anthropic, OpenAI, or Ollama)
  4. CLI can start interactive session with agent
  5. Errors are classified as retryable or non-retryable with appropriate handling
**Plans**: 4 plans in 4 waves

Plans:
- [ ] 01-01-PLAN.md — Project foundation & error infrastructure (Wave 0)
- [ ] 01-02-PLAN.md — LLM provider layer (Wave 1)
- [ ] 01-03-PLAN.md — Agent loop core (Wave 2)
- [ ] 01-04-PLAN.md — CLI session interface (Wave 3)

### Phase 2: s02 - Tool Use

**Goal**: Agent can execute actions through registered tool handlers
**Depends on**: Phase 1
**Requirements**: CORE-02, CORE-03, CORE-04, CORE-05 (complete), PLAN-05, PERS-04 (partial), CROSS-01, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Tool dispatch registry maps tool names to async handlers
  2. Agent can execute bash commands and capture output
  3. Agent can read, write, and edit files atomically
  4. Agent can search codebase using glob and grep patterns
  5. Agent can switch between multiple LLM backends (Anthropic, OpenAI, Ollama)
  6. Destructive operations require interactive confirmation
  7. MCP protocol support enables external tool integration
**Plans**: TBD

### Phase 3: s03 - TodoWrite

**Goal**: Agent maintains persistent task list that prevents drift
**Depends on**: Phase 2
**Requirements**: PLAN-01, CROSS-02, CROSS-03, CROSS-04
**Success Criteria** (what must be TRUE):

  1. Agent can create, update, and delete tasks in a task list
  2. Agent receives nag reminders about pending tasks during loops
  3. Task state persists across agent turns
  4. Agent can mark tasks complete and track progress
**Plans**: TBD

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

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → ... → 12

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. s01 - Agent Loop | 0/4 | Ready for execution | - |
| 2. s02 - Tool Use | 0/TBD | Not started | - |
| 3. s03 - TodoWrite | 0/TBD | Not started | - |
| 4. s04 - Subagents | 0/TBD | Not started | - |
| 5. s05 - Skills | 0/TBD | Not started | - |
| 6. s06 - Context Compact | 0/TBD | Not started | - |
| 7. s07 - Tasks | 0/TBD | Not started | - |
| 8. s08 - Background Tasks | 0/TBD | Not started | - |
| 9. s09 - Agent Teams | 0/TBD | Not started | - |
| 10. s10 - Team Protocols | 0/TBD | Not started | - |
| 11. s11 - Autonomous Agents | 0/TBD | Not started | - |
| 12. s12 - Worktree Isolation | 0/TBD | Not started | - |
