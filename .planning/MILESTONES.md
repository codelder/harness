# Milestones

## v0.3 TodoWrite (Shipped: 2026-03-23)

**Phases completed:** 4 phases, 12 plans, 22 tasks

**Key accomplishments:**

- Rust project foundation with Cargo.toml dependencies, module skeleton, and error classification types with is_retryable() logic
- Chosen: Enum-based Provider Dispatch
- 1. [Rule 3 - Blocking] rig-core Chat trait not object-safe
- Interactive REPL CLI with clap argument parsing, tracing-based logging, and graceful Ctrl+C exit with session summary
- One-liner:
- Primary Goal:
- All six tools (Bash, Read, Write, Edit, Glob, Grep) registered with AgentBuilder for both Anthropic and OpenAI providers via .tool() chain pattern
- 1. [Rule 1 - Bug] Added PartialEq/Eq derives to TodoError
- TodoTool implementing rig::tool::Tool trait with Arc<Mutex<TodoManager>> for shared state access, enabling LLM agent task tracking.
- Session integration with todo_manager field, TodoUsageHook for tool detection, and model-visible nag reminder injection

---

## v0.2 Tool Use (Shipped: 2026-03-23)

**Status:** ✅ SHIPPED
**Phases:** 2 (Phase 1 from v0.1 + Phase 2) | **Plans:** 7 (4 + 3)

### Delivered

Tool dispatch system with 6 production-ready tools (Bash, Read, Write, Edit, Glob, Grep), safety blacklist, and multi-provider support (Anthropic, OpenAI).

### Key Accomplishments

1. **File Tools** — ReadTool with 50K truncation, WriteTool with auto-mkdir, EditTool with single-match validation
2. **Search Tools** — GlobTool with recursive patterns, GrepTool with regex + truncation
3. **Safety System** — BashTool dangerous command blacklist (rm -rf /, sudo, mkfs, etc.)
4. **Tool Registration** — All 6 tools registered via AgentBuilder.tool() chain
5. **Multi-Provider** — Anthropic and OpenAI fully functional (Ollama pending rig-core support)

### Stats

- Lines of code: 2,832 (Rust)
- Tests: 87 passing
- Timeline: 1 day

### Known Gaps

- CROSS-01 (MCP Protocol): Deferred - not implemented
- CORE-05 (Ollama): Placeholder only, pending rig-core native support

### Archives

- [v0.2-ROADMAP.md](milestones/v0.2-ROADMAP.md)
- [v0.2-REQUIREMENTS.md](milestones/v0.2-REQUIREMENTS.md)
- [v0.2-MILESTONE-AUDIT.md](milestones/v0.2-MILESTONE-AUDIT.md)

---

## v0.1 Foundation — 2026-03-22

**Status:** ✅ SHIPPED
**Phases:** 1 | **Plans:** 4

### Delivered

Foundation milestone establishing core agent loop architecture with multi-provider LLM support, error handling, and CLI interface.

### Key Accomplishments

1. **Agent Loop** — Core while loop with stop_reason handling and retry logic
2. **Multi-Provider** — Anthropic, OpenAI, Ollama support via rig-core
3. **Error Classification** — HTTP status code mapping with retryable detection
4. **CLI Interface** — Full REPL with Ctrl+C/D support via reedline
5. **Dual Logging** — Separate harness.log and llm.log files

### Stats

- Lines of code: 1,825 (Rust)
- Timeline: 2 days
- Commits: 20+

### Archives

- [v0.1-ROADMAP.md](milestones/v0.1-ROADMAP.md)
- [v0.1-REQUIREMENTS.md](milestones/v0.1-REQUIREMENTS.md)

---

*Next milestone: v0.3 Advanced Features (Phases 3-4)*
