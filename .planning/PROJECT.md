# Rust Agent Harness

## What This Is

A high-performance, memory-safe AI agent harness built in Rust, re-implementing the 12-session curriculum from [learn-claude-code](https://github.com/shareAI-lab/learn-claude-code). This project transforms the Python reference implementation into a production-ready Rust CLI tool.

The harness provides the environment that enables an LLM (the agent) to perceive, reason, and act - implementing the core pattern: `Harness = Tools + Knowledge + Observation + Action + Permissions`.

## Core Value

**Performance and safety without sacrificing capability.** The Rust implementation delivers memory safety, zero-cost abstractions, and superior async performance while maintaining full feature parity with the Python reference - enabling real-time, long-running agent sessions without resource concerns.

## Current State

**Shipped:** v0.3 TodoWrite (2026-03-24)

- 3,500+ lines of Rust code
- 100+ tests passing
- 7 production tools: Bash, Read, Write, Edit, Glob, Grep, TodoWrite
- Multi-provider support: Anthropic, OpenAI (Ollama pending)
- Nag reminder system for drift prevention

## Current Milestone: v0.4 Subagents

**Goal:** Subagent spawning with isolated context

**Target Features:**
1. Spawn child agents with fresh message arrays
2. Child agents execute without polluting parent context
3. Parent receives summarized results when child completes
4. Graceful error handling for child failures

**Reference:**
- [s04-subagents.md](https://github.com/shareAI-lab/learn-claude-code/blob/main/docs/en/s04-subagents.md)

**Phase:** 4 (s04 - Subagents)

## Requirements

### Validated

- ✓ Agent loop with stop_reason handling (s01) — v0.1
- ✓ Multi-backend LLM support: Anthropic, OpenAI — v0.1, v0.2
- ✓ CLI interface with interactive mode — v0.1
- ✓ Tool dispatch system with handler registration (s02) — v0.2
- ✓ File tools: Read, Write, Edit — v0.2
- ✓ Search tools: Glob, Grep — v0.2
- ✓ Bash tool with safety blacklist — v0.2

### Active

- [ ] TodoWrite for task planning with nag reminders (s03)
- [ ] Subagent spawning with isolated context (s04)
- [ ] On-demand skill loading via tool_result (s05)
- [ ] Three-layer context compression strategy (s06)
- [ ] File-based task graph with dependencies (s07)
- [ ] Background task execution with notifications (s08)
- [ ] Multi-agent teams with async JSONL mailboxes (s09)
- [ ] Team communication protocols (shutdown, plan approval FSM) (s10)
- [ ] Autonomous task claiming by idle agents (s11)
- [ ] Worktree isolation for parallel execution (s12)
- [ ] Ollama provider support (pending rig-core native support)
- [ ] MCP (Model Context Protocol) support (deferred)
- [ ] Full sandbox with path sanitization and allowlist

### Out of Scope

- Training or fine-tuning LLM models — This is a harness, not model development
- Mobile applications — Web-first, mobile-responsive later
- Cloud deployment infrastructure — Local-first tool
- Multi-user collaboration features — Personal use only
- Commercial licensing — Personal/open-source project

## Context

This project follows the learn-claude-code curriculum which teaches that **the model IS the agent**. The harness is everything the agent needs to function in a domain:

```text
Harness = Tools + Knowledge + Observation + Action Interfaces + Permissions
```

The 12 sessions build progressively:

- **Phase 1 (s01-s02)**: The Loop — basic agent loop and tool dispatch ✅ SHIPPED
- **Phase 2 (s03-s06)**: Planning & Knowledge — TodoWrite, subagents, skills, compression
- **Phase 3 (s07-s08)**: Persistence — task system and background execution
- **Phase 4 (s09-s12)**: Teams — multi-agent coordination and isolation

## Constraints

- **Language**: Rust (stable, latest) — Memory safety and performance
- **Async Runtime**: Tokio — Industry standard, best ecosystem
- **LLM Backends**: Multi-provider via trait abstraction — Anthropic, OpenAI, Ollama/local
- **Safety**: Sandbox-first with interactive escalation — Default isolation, confirm for system access

## Key Decisions

| Decision | Rationale | Outcome |
| -------- | --------- | ------- |
| Tokio async runtime | Industry standard, best ecosystem, proven at scale | ✓ Good |
| rig-core 0.31 for multi-provider LLM | Zero-cost abstraction, type-safe, compile-time exhaustiveness | ✓ Good |
| thiserror for library errors | Clean error types with #[from] conversion | ✓ Good |
| Enum-based Provider dispatch | Zero-cost abstraction, type-safe | ✓ Good |
| AgentBuilder.tool() chain for registration | Clean, declarative tool registration | ✓ Good |
| 50K char output truncation | Prevents context explosion | ✓ Good |
| Command blacklist for BashTool safety | Simple, effective baseline protection | ✓ Good |
| Ollama support via rig-core native | Waiting for upstream support | ⚠ Pending |
| MCP protocol via rmcp | Deferred to future phase | — Deferred |

---
Last updated: 2026-03-23 after v0.2 milestone
