# Requirements: Rust Agent Harness

**Defined:** 2025-03-20
**Core Value:** Performance and safety without sacrificing capability

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Phase 1: Core Loop (s01-s02)

- [ ] **CORE-01**: Agent loop with stop_reason handling — while loop that processes tool_use until text response
- [ ] **CORE-02**: Tool dispatch system with handler registration — dispatch map: name -> handler function
- [ ] **CORE-03**: Bash tool for command execution — execute shell commands with output capture
- [ ] **CORE-04**: Read/Write/Edit file tools — atomic file operations for code manipulation
- [ ] **CORE-05**: Multi-backend LLM provider — abstract trait for Anthropic, OpenAI, Ollama/local
- [ ] **CORE-06**: CLI interface with clap — argument parsing and interactive mode

### Phase 2: Planning & Knowledge (s03-s06)

- [ ] **PLAN-01**: TodoWrite with nag reminders — task list that reminds agent of pending work
- [ ] **PLAN-02**: Subagent spawning with isolated messages[] — fresh context per child, clean return to parent
- [ ] **PLAN-03**: On-demand skill loading via tool_result — inject SKILL.md content when requested
- [ ] **PLAN-04**: Three-layer context compression — oldest messages → summary → discard
- [ ] **PLAN-05**: Glob/Grep tools for code search — pattern matching for file discovery

### Phase 3: Persistence (s07-s08)

- [ ] **PERS-01**: File-based task graph with dependencies — JSONL task storage with dependency edges
- [ ] **PERS-02**: Background task execution with daemon threads — tokio::spawn for async operations
- [ ] **PERS-03**: Notification queue for completion events — inject results when background tasks finish
- [ ] **PERS-04**: Sandbox execution with interactive confirmation — nix/seccomp isolation, confirm for system access

### Phase 4: Teams (s09-s12)

- [ ] **TEAM-01**: Multi-agent teams with persistent teammates — teammate registry with capabilities
- [ ] **TEAM-02**: Async JSONL mailboxes for communication — file-based message queues per agent
- [ ] **TEAM-03**: Team protocols (shutdown FSM, plan approval) — structured communication patterns
- [ ] **TEAM-04**: Autonomous task claiming by idle agents — idle cycle scans board, claims available work
- [ ] **TEAM-05**: Worktree isolation for parallel execution — git worktree per task
- [ ] **TEAM-06**: Task-to-worktree binding by ID — tasks and worktrees linked by identifier

### Cross-Cutting Concerns

- [ ] **CROSS-01**: MCP protocol support via rmcp — Model Context Protocol for extensibility
- [ ] **CROSS-02**: Error classification (retryable vs non-retryable) — proper error handling per provider
- [ ] **CROSS-03**: Graceful degradation when providers fail — fallback logic with user notification
- [ ] **CROSS-04**: Structured logging and observability — tracing/tracing-subscriber for diagnostics

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
|---------|--------|
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
|-------------|-------|--------|
| CORE-01 | Phase 1 | Pending |
| CORE-02 | Phase 1 | Pending |
| CORE-03 | Phase 1 | Pending |
| CORE-04 | Phase 1 | Pending |
| CORE-05 | Phase 1 | Pending |
| CORE-06 | Phase 1 | Pending |
| PLAN-01 | Phase 2 | Pending |
| PLAN-02 | Phase 2 | Pending |
| PLAN-03 | Phase 2 | Pending |
| PLAN-04 | Phase 2 | Pending |
| PLAN-05 | Phase 2 | Pending |
| PERS-01 | Phase 3 | Pending |
| PERS-02 | Phase 3 | Pending |
| PERS-03 | Phase 3 | Pending |
| PERS-04 | Phase 3 | Pending |
| TEAM-01 | Phase 4 | Pending |
| TEAM-02 | Phase 4 | Pending |
| TEAM-03 | Phase 4 | Pending |
| TEAM-04 | Phase 4 | Pending |
| TEAM-05 | Phase 4 | Pending |
| TEAM-06 | Phase 4 | Pending |
| CROSS-01 | Phase 1-4 | Pending |
| CROSS-02 | Phase 1-4 | Pending |
| CROSS-03 | Phase 1-4 | Pending |
| CROSS-04 | Phase 1-4 | Pending |

**Coverage:**
- v1 requirements: 25 total
- Mapped to phases: 25
- Unmapped: 0 ✓

---
*Requirements defined: 2025-03-20*
*Last updated: 2025-03-20 after initial definition*
