# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

A high-performance, memory-safe AI agent harness built in Rust. The project re-implements the 12-session curriculum from [learn-Codex](https://github.com/shareAI-lab/learn-Codex), transforming the Python reference into a production-ready Rust CLI tool.

**Core principle:** The model IS the agent. The harness provides:
```
Harness = Tools + Knowledge + Observation + Action Interfaces + Permissions
```

## Architecture

The system is built around a core agent loop with stop_reason branching:

```
while True:
    response = LLM(messages, tools)
    if stop_reason != "tool_use": return
    results = execute_tools(response.tool_calls)
    messages.append(results)
```

### Key Components (by phase)

| Phase | Component | Description |
|-------|-----------|-------------|
| s01 | Agent Loop | Core orchestration with stop_reason handling |
| s02 | Tool Dispatch | HashMap registry: tool_name -> async handler |
| s03 | TodoWrite | Task list with nag reminders |
| s04 | Subagents | Child agents with fresh messages[] |
| s05 | Skills | On-demand knowledge via tool_result |
| s06 | Context Compression | Three-layer: summarize, truncate, persist |
| s07 | Task Graph | File-based JSONL with dependencies |
| s08 | Background Tasks | Daemon threads + notification injection |
| s09-s12 | Multi-Agent | Teams, protocols, autonomous claiming, worktree isolation |

### Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library root
├── agent/               # Core loop, messages, stop_reason
├── tools/               # Tool registry and handlers
├── llm/                 # Multi-provider abstraction
├── context/             # History and compression
├── planning/            # TodoWrite, tasks, skills
├── subagent/            # Spawning and context isolation
├── team/                # Mailboxes and protocols
├── background/          # Daemon execution
├── isolation/           # Worktree and sandbox
└── cli/                 # Argument parsing and session
```

## Tech Stack

| Crate | Purpose |
|-------|---------|
| **tokio** | Async runtime (only viable choice) |
| **rig-core** | Multi-provider LLM abstraction (Anthropic, OpenAI, Ollama) |
| **rmcp** | MCP protocol (official Rust SDK) |
| **clap** | CLI with derive macros |
| **tracing** | Structured logging (replaces `log` crate) |
| **thiserror** | Library error types |
| **anyhow** | CLI application errors |
| **serde_json** | JSONL for agent communication |

## Critical Constraints

### Must Use
- `tokio::sync::mpsc` for async channels (NOT `std::sync::mpsc`)
- `tracing` for logging (NOT `log` crate)
- `thiserror` for library errors, `anyhow` for CLI

### Must NOT Use
- `async-std` — discontinued as of 2025
- `std::sync::mpsc` — blocks async executor
- `crossbeam-channel` in async contexts — blocks executor
- `reqwest::blocking` — blocks runtime
- `unwrap()` in library code — use `Result` propagation

### Anti-Patterns to Avoid

1. **Prompt Plumbing** — Don't wire LLM calls with if-else branches. Build clean agent loop with tools, let model decide.

2. **System Prompt Stuffing** — Don't put all knowledge upfront. Use on-demand skill loading via tool_result.

3. **No Context Isolation** — Subtasks MUST use fresh messages[]. Return only summary to parent context.

4. **Blind Tool Trust** — Sandbox-first design. Require approval for destructive operations.

5. **Blocking on Long Ops** — Use background daemon threads, inject notifications on completion.

## Planning Documentation

- `.planning/ROADMAP.md` — 12-phase delivery plan with success criteria
- `.planning/REQUIREMENTS.md` — Requirements mapped to phases (CORE-*, PLAN-*, PERS-*, TEAM-*, CROSS-*)
- `.planning/PROJECT.md` — Project vision and constraints
- `.planning/research/` — Architecture, stack, and pitfalls research

## Build Commands (once Cargo.toml exists)

```bash
cargo build          # Debug build
cargo build --release  # Release build
cargo test           # Run all tests
cargo test test_name  # Run specific test
cargo clippy         # Lint
cargo fmt            # Format
cargo run            # Run CLI
```

## API Keys

Set via environment variables:
- `HARNESS_ANTHROPIC_KEY` — Anthropic Claude API key
- `HARNESS_ANTHROPIC_URL` — Anthropic base URL (optional)
- `HARNESS_OPENAI_KEY` — OpenAI API key
- `HARNESS_OPENAI_URL` — OpenAI base URL (optional)
- Ollama runs locally, no key needed
