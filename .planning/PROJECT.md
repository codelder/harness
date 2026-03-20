# Rust Agent Harness

## What This Is

A high-performance, memory-safe AI agent harness built in Rust, re-implementing the 12-session curriculum from [learn-claude-code](https://github.com/shareAI-lab/learn-claude-code). This project transforms the Python reference implementation into a production-ready Rust CLI tool with a SvelteKit-based web visualization interface.

The harness provides the environment that enables an LLM (the agent) to perceive, reason, and act - implementing the core pattern: `Harness = Tools + Knowledge + Observation + Action + Permissions`.

## Core Value

**Performance and safety without sacrificing capability.** The Rust implementation delivers memory safety, zero-cost abstractions, and superior async performance while maintaining full feature parity with the Python reference - enabling real-time, long-running agent sessions without resource concerns.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Agent loop with stop_reason handling (s01)
- [ ] Tool dispatch system with handler registration (s02)
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
- [ ] Multi-backend LLM support (Anthropic, OpenAI, local models)
- [ ] CLI interface with interactive mode
- [ ] SvelteKit web visualization platform
- [ ] Sandbox execution with interactive confirmation for system commands
- [ ] MCP (Model Context Protocol) support

### Out of Scope

- Training or fine-tuning LLM models — This is a harness, not model development
- Mobile applications — Web-first, mobile-responsive later
- Cloud deployment infrastructure — Local-first tool
- Multi-user collaboration features — Personal use only
- Commercial licensing — Personal/open-source project

## Context

This project follows the learn-claude-code curriculum which teaches that **the model IS the agent**. The harness is everything the agent needs to function in a domain:

```
Harness = Tools + Knowledge + Observation + Action Interfaces + Permissions
```

The 12 sessions build progressively:
- **Phase 1 (s01-s02)**: The Loop — basic agent loop and tool dispatch
- **Phase 2 (s03-s06)**: Planning & Knowledge — TodoWrite, subagents, skills, compression
- **Phase 3 (s07-s08)**: Persistence — task system and background execution
- **Phase 4 (s09-s12)**: Teams — multi-agent coordination and isolation

## Constraints

- **Language**: Rust (stable, latest) — Memory safety and performance
- **Async Runtime**: Tokio — Industry standard, best ecosystem
- **LLM Backends**: Multi-provider via trait abstraction — Anthropic, OpenAI, Ollama/local
- **Frontend**: SvelteKit — Type-safe, performant web visualization
- **Protocol**: MCP support required — Model Context Protocol for extensibility
- **Safety**: Sandbox-first with interactive escalation — Default isolation, confirm for system access

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Tokio async runtime | Industry standard, best ecosystem, proven at scale | — Pending |
| Multi-backend LLM | Flexibility for different models, future-proof | — Pending |
| Sandbox-first security | Safety by default, explicit trust escalation | — Pending |
| SvelteKit for web | Type-safe, performant, Rust-like developer experience | — Pending |
| MCP protocol support | Extensibility, standard tool integration | — Pending |

---
*Last updated: 2025-03-20 after initialization*
