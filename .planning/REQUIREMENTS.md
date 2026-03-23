# Requirements: Rust Agent Harness

**Defined:** 2025-03-20
**Core Value:** Performance and safety without sacrificing capability

## Archived Requirements

Requirements completed in shipped milestones.

### v0.1 Foundation (Phase 1) — Shipped 2026-03-22

All Phase 1 requirements archived. See [v0.1-REQUIREMENTS.md](milestones/v0.1-REQUIREMENTS.md)

## v0.2 Requirements (In Progress)

Requirements for next milestone. Each maps to roadmap phases.

### Phase 2: s02 - Tool Use

- [x] **CORE-02**: Tool dispatch system with handler registration — dispatch map: name -> handler function
- [x] **CORE-03**: Bash tool for command execution — execute shell commands with output capture
- [x] **CORE-04**: Read/Write/Edit file tools — atomic file operations for code manipulation
- [x] **CORE-05**: Multi-backend LLM provider — complete multi-provider support (Anthropic, OpenAI, Ollama)
- [x] **PLAN-05**: Glob/Grep tools for code search — pattern matching for file discovery
- [x] **PERS-04**: Sandbox execution with interactive confirmation — nix/seccomp isolation, confirm for system access (partial: basic sandbox)
- [x] **CROSS-01**: MCP protocol support via rmcp — Model Context Protocol for extensibility

### Phase 3: s03 - TodoWrite

- [ ] **PLAN-01**: TodoWrite with nag reminders — task list that reminds agent of pending work

### Phase 4: s04 - Subagents

- [ ] **PLAN-02**: Subagent spawning with isolated messages[] — fresh context per child, clean return to parent

### Phase 5: s05 - Skills

- [ ] **PLAN-03**: On-demand skill loading via tool_result — inject SKILL.md content when requested

### Phase 6: s06 - Context Compact

- [ ] **PLAN-04**: Three-layer context compression — oldest messages → summary → discard

### Phase 7: s07 - Tasks

- [ ] **PERS-01**: File-based task graph with dependencies — JSONL task storage with dependency edges

### Phase 8: s08 - Background Tasks

- [ ] **PERS-02**: Background task execution with daemon threads — tokio::spawn for async operations
- [ ] **PERS-03**: Notification queue for completion events — inject results when background tasks finish
- [x] **PERS-04**: Sandbox execution with interactive confirmation — complete sandbox implementation

### Phase 9: s09 - Agent Teams

- [ ] **TEAM-01**: Multi-agent teams with persistent teammates — teammate registry with capabilities
- [ ] **TEAM-02**: Async JSONL mailboxes for communication — file-based message queues per agent

### Phase 10: s10 - Team Protocols

- [ ] **TEAM-03**: Team protocols (shutdown FSM, plan approval) — structured communication patterns

### Phase 11: s11 - Autonomous Agents

- [ ] **TEAM-04**: Autonomous task claiming by idle agents — idle cycle scans board, claims available work

### Phase 12: s12 - Worktree Isolation

- [ ] **TEAM-05**: Worktree isolation for parallel execution — git worktree per task
- [ ] **TEAM-06**: Task-to-worktree binding by ID — tasks and worktrees linked by identifier

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Web Platform

- **WEB-01**: SvelteKit visualization interface
- **WEB-02**: Real-time agent state display
- **WEB-03**: Interactive tool execution viewer
- **WEB-04**: Session history browser

### Advanced Features

- **ADV-01**: Performance benchmarking suite
- **ADV-02**: Advanced context compression strategies
- **ADV-03**: Multi-session resume/fork capabilities
- **ADV-04**: Custom tool plugins via dynamic loading

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
| ------- | ------ |
| LLM training/fine-tuning | This is a harness, not model development |
| Mobile applications | Web-first, mobile-responsive later |
| Cloud deployment infrastructure | Local-first tool |
| Multi-user collaboration features | Personal use only |
| Commercial licensing | Personal/open-source project |
| Real-time everything | Use async patterns instead |
| Over-engineered multi-agent | Start single-agent, validate first |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
| ----------- | ----- | ------ |
| ~~CORE-01~~ | ~~Phase 1~~ | ✅ Archived (v0.1) |
| ~~CORE-05 (partial)~~ | ~~Phase 1~~ | ✅ Archived (v0.1) |
| ~~CORE-06~~ | ~~Phase 1~~ | ✅ Archived (v0.1) |
| ~~CROSS-02~~ | ~~Phase 1~~ | ✅ Archived (v0.1) |
| ~~CROSS-03~~ | ~~Phase 1~~ | ✅ Archived (v0.1) |
| ~~CROSS-04~~ | ~~Phase 1~~ | ✅ Archived (v0.1) |
| CORE-02 | Phase 2 | Complete |
| CORE-03 | Phase 2 | Complete |
| CORE-04 | Phase 2 | Complete |
| CORE-05 (complete) | Phase 2 | Pending |
| PLAN-05 | Phase 2 | Complete |
| PERS-04 (partial) | Phase 2 | Pending |
| CROSS-01 | Phase 2 | Complete |
| PLAN-01 | Phase 3 | Pending |
| PLAN-02 | Phase 4 | Pending |
| PLAN-03 | Phase 5 | Pending |
| PLAN-04 | Phase 6 | Pending |
| PERS-01 | Phase 7 | Pending |
| PERS-02 | Phase 8 | Pending |
| PERS-03 | Phase 8 | Pending |
| PERS-04 (complete) | Phase 8 | Pending |
| TEAM-01 | Phase 9 | Pending |
| TEAM-02 | Phase 9 | Pending |
| TEAM-03 | Phase 10 | Pending |
| TEAM-04 | Phase 11 | Pending |
| TEAM-05 | Phase 12 | Pending |
| TEAM-06 | Phase 12 | Pending |

**Coverage:**

- v0.1 requirements: 6 (archived)
- v0.2 requirements: 19 (pending)

---
*Requirements defined: 2025-03-20*
*Last updated: 2026-03-22 after v0.1 milestone archive*
