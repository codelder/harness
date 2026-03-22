# Milestones

Historical record of shipped versions.

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

*Next milestone: v0.2 Tool Use (Phase 2-4)*
