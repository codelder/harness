# Phase 1: s01 - Agent Loop - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

LLM becomes an agent through core loop that processes responses until text output. This phase delivers the foundational agent loop with stop_reason handling, multi-provider LLM support, and interactive CLI.

**In scope:**
- Core agent loop with stop_reason branching (text vs tool_use)
- Multi-backend LLM provider support (Anthropic, OpenAI, Ollama)
- REPL-style interactive CLI session
- Error classification and retry logic
- Structured logging and observability

**Out of scope:**
- Tool dispatch system (Phase 2)
- File operations (Phase 2)
- Task planning (Phase 3)
- Context compression (Phase 6)
- Session persistence (Phase 7)

</domain>

<decisions>
## Implementation Decisions

### LLM Provider Support
- Support all three providers in Phase 1: Anthropic, OpenAI, Ollama
- Use rig-core's multi-provider abstraction layer
- Provider selection via CLI flag: `--provider anthropic|openai|ollama`
- API keys from environment variables: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`

### CLI Interaction Mode
- REPL-style interactive session (not single-shot command)
- Single-line input, Enter to send
- Graceful exit on Ctrl+C with session summary
- Essential flags only: `--provider`, `--model`, `--verbose`

### Error Handling & Retry
- Auto-retry on network errors and rate limits with exponential backoff
- Maximum 3 retries before failing
- User-friendly error messages by default
- Full technical details with `--verbose` flag
- Graceful handling when hitting context window limit (notify user, don't crash)

### Streaming & Output
- Stream LLM output in real-time (not wait for complete response)
- Default silent logging, `-v` for info level, `-vv` for debug
- Structured tracing via tracing-subscriber

### System Prompt
- Minimal hardcoded system prompt for Phase 1
- Tell the model it's an agent with loop capability
- Full skill system deferred to Phase 5

### Session State
- No persistence between runs (fresh session each start)
- Session persistence deferred to Phase 7 task system

### Claude's Discretion
- Exact retry backoff timing (start with 1s, double each retry)
- Specific error message wording
- Context limit threshold percentage (e.g., warn at 80%)
- Exact CLI welcome message and exit summary format

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Patterns
- `.planning/research/ARCHITECTURE.md` — Agent loop pattern, stop_reason handling, system architecture
- `.planning/research/SUMMARY.md` — Build order, phase dependencies, critical pitfalls
- `.planning/research/STACK.md` — rig-core usage, tokio patterns, tracing setup

### Requirements
- `.planning/REQUIREMENTS.md` — CORE-01, CORE-05 (partial), CORE-06, CROSS-02, CROSS-03, CROSS-04

### Roadmap
- `.planning/ROADMAP.md` — Phase 1 success criteria and dependency chain

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
None yet — this is a greenfield project. Research documents provide architectural guidance.

### Established Patterns
From research, recommended patterns for Phase 1:
- **Agent Loop:** `while` loop + `stop_reason` branching (under 50 lines core logic)
- **Provider Trait:** rig-core provides multi-provider abstraction
- **CLI:** clap derive macros for type-safe argument parsing
- **Logging:** tracing + tracing-subscriber with env-filter

### Integration Points
- LLM API calls via rig-core
- CLI entry point via `src/main.rs`
- Async runtime via tokio

</code_context>

<specifics>
## Specific Ideas

- "The model IS the agent" — harness provides mechanisms, not routing logic
- Agent loop should be clean and simple, under 50 lines of core logic
- Fail gracefully, never crash on recoverable errors
- User experience should feel like Claude Code CLI: responsive, streaming, helpful errors

</specifics>

<deferred>
## Deferred Ideas

- Tool dispatch and file operations — Phase 2
- TodoWrite with nag reminders — Phase 3
- Subagent spawning — Phase 4
- On-demand skill loading — Phase 5
- Context compression — Phase 6
- Session persistence — Phase 7
- Multi-agent teams — Phase 9

</deferred>

---

*Phase: 01-s01-agent-loop*
*Context gathered: 2026-03-20*
