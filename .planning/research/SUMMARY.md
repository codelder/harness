# Project Research Summary

**Project:** Rust Agent Harness
**Domain:** AI Agent Infrastructure
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

An AI agent harness is infrastructure that surrounds a language model to enable it to perceive, reason, and act in a specific domain. The fundamental insight from the learn-claude-code curriculum is that **the model IS the agent** - the harness provides tools, knowledge, observation interfaces, action capabilities, and permissions. This project re-implements a proven 12-session Python curriculum in Rust, delivering memory safety and zero-cost abstractions while maintaining full feature parity.

The recommended approach follows a four-phase build order: (1) Core agent loop with tool dispatch, (2) Planning and knowledge mechanisms (TodoWrite, subagents, skills, compression), (3) Persistence layer (task graphs, background execution), and (4) Multi-agent teams with isolation. Critical risks include context window overflow (mitigated by three-layer compression), security vulnerabilities (mitigated by sandbox-first design), and architecture complexity (mitigated by building single-agent capabilities before multi-agent orchestration).

The Rust implementation provides a competitive advantage over Python frameworks (LangChain, CrewAI, AutoGen) through guaranteed memory safety, no GC pauses during long-running sessions, and superior async performance. The rig-core framework offers multi-provider LLM support while rmcp provides official MCP protocol implementation, establishing a solid foundation for extensibility.

## Key Findings

### Recommended Stack

The stack centers on mature, production-ready Rust crates with strong ecosystem support. Tokio provides the async runtime (the only viable choice for production async Rust in 2025). The rig-core framework delivers multi-provider LLM abstraction with built-in streaming and agent primitives. The rmcp crate is the official Rust SDK for Model Context Protocol.

**Core technologies:**
- **tokio 1.44+** - Async runtime — industry standard, best ecosystem, proven at scale
- **rig-core 0.18+** - LLM framework — multi-provider support (Anthropic, OpenAI, Ollama), ergonomic agent abstractions, built-in streaming
- **rmcp 1.2+** - MCP protocol — official Rust SDK from modelcontextprotocol organization
- **clap 4.5+** - CLI parsing — de facto standard with derive macros for type-safe arguments
- **serde/serde_json 1.0+** - Serialization — JSON Lines (JSONL) is standard for agent communication and mailboxes
- **tracing 0.1+** - Observability — structured logging that survives async boundaries correctly

**Critical version notes:**
- async-std is officially discontinued as of 2025 - do NOT use
- Use thiserror for library errors (structured), anyhow for CLI errors (ergonomic with context)
- Avoid std::sync::mpsc in async contexts - use tokio::sync::mpsc instead

### Expected Features

The research identifies a clear progression from table stakes (users expect these) to differentiators (competitive advantage) to anti-features (commonly requested but problematic).

**Must have (table stakes):**
- Agent Loop with stop_reason handling — core mechanism that makes an LLM an agent
- Tool Dispatch System — maps tool names to handlers, extensible registration
- File Read/Write Tools — fundamental capability for codebase interaction
- Shell/Bash Execution — primary action interface (requires sandboxing)
- Multi-LLM Backend Support — provider flexibility via trait abstraction
- CLI Interface — primary developer interface with interactive mode
- Context/Message Management — conversation history foundation
- Error Handling & Recovery — graceful recovery, retry logic, timeouts

**Should have (differentiators):**
- TodoWrite with Nag Reminders — 2x task completion rate, prevents agent drift
- Subagent Spawning with Isolated Context — clean context per subtask, foundation for teams
- On-Demand Skill Loading (via tool_result) — reduces context bloat, modular expertise
- Three-Layer Context Compression — enables infinite sessions without context explosion
- File-Based Task Graph with Dependencies — persistent goals across sessions
- Background Task Execution + Notifications — non-blocking slow operations
- Multi-Agent Teams with Async JSONL Mailboxes — scale beyond single agent
- Sandbox-First Security — safe by default, explicit trust escalation

**Defer (v2+):**
- Autonomous Task Claiming — idle agents self-assign work (most users don't need initially)
- MCP Full Support — standard tool ecosystem integration (can be added incrementally)
- Advanced Web Platform — session replay, team dashboards, analytics

**Anti-features to avoid:**
- Real-time Everything / No Async — explodes complexity, poor UX for long tasks
- Over-Engineered Multi-Agent for Simple Tasks — 90% of tasks don't need orchestration
- Massive Tool Library Upfront — tool bloat causes agent confusion
- Complex Decision Trees / Rule-Based Routing — reimplements GOFAI, can't generalize
- No Sandbox / Full System Access — security nightmare, no trust boundaries

### Architecture Approach

The architecture follows a clean separation between the agent loop (core orchestration), tool dispatch (handler registry), LLM provider abstraction (multi-backend support), and context management (three-layer compression). The system is designed around the principle that the model makes decisions - the harness provides mechanisms, not routing logic.

**Major components:**
1. **Agent Loop** — Core orchestration: calls LLM, checks stop_reason, dispatches tools or returns text
2. **Tool Dispatch Registry** — HashMap pattern mapping tool names to async handlers
3. **LLM Provider Trait** — Abstraction for multi-backend support (Anthropic, OpenAI, Ollama)
4. **Context Manager** — Three-layer compression: summarize old turns, truncate verbose outputs, persist to disk
5. **Subagent Spawner** — Fresh messages[] per child agent to prevent context pollution
6. **Skill Loader** — On-demand knowledge injection via tool_result
7. **Task Graph** — File-based JSON/JSONL persistence with dependency edges
8. **Team Mailboxes** — Async JSONL communication between persistent teammates

**Key architectural patterns:**
- Agent Loop: while loop + stop_reason branching (under 50 lines of core logic)
- Tool Dispatch: HashMap<String, Box<dyn ToolHandler>> registration pattern
- Subagent Isolation: Fresh messages[] per spawn, summarized results merged back
- Context Compression: Hierarchical strategy to prevent context window overflow
- Team Communication: JSONL file-based mailboxes for async coordination

### Critical Pitfalls

The research identifies five critical pitfalls that must be avoided during implementation:

1. **Prompt Plumbing** — Wiring together LLM calls with if-else branches and hardcoded routing
   - **How to avoid:** Build clean agent loop with tools. Let the model decide. Add harness mechanisms, not routing logic.

2. **Stuffing Everything in System Prompt** — Putting all knowledge upfront causes context bloat
   - **How to avoid:** Use on-demand skill loading. Model knows skill names, loads content via tool_result when needed.

3. **No Context Isolation for Subtasks** — Running subtasks in same message array pollutes main context
   - **How to avoid:** Spawn subagents with fresh messages[]. Return only summary to main context.

4. **Trusting Tool Outputs Blindly** — Models can hallucinate dangerous commands (rm -rf, API key leaks)
   - **How to avoid:** Sandbox-first design. Require explicit approval for destructive operations. Validate inputs.

5. **Blocking on Long-Running Operations** — await bash commands that take minutes blocks agent loop
   - **How to avoid:** Background task execution. Daemon threads run commands, inject notifications when complete.

**Common failure modes from production research:**
- Context Rot: Context fills with tool outputs, model loses original instructions → Prevention: Context compression (s06)
- Hallucinated Tool Calls: Agent calls non-existent APIs with wrong parameters → Prevention: Tool validation layer
- Lost State on Failure: Network timeout wipes in-memory progress → Prevention: File-based persistence (s07)
- Infinite Loops: No max iterations or timeouts → Prevention: Iteration limits + escalation
- Over-Communication: Agents passing full history, token explosion → Prevention: Pass only necessary context

## Implications for Roadmap

Based on research, suggested phase structure follows the proven 12-session curriculum with clear dependency ordering:

### Phase 1: Core Loop (s01-s02)
**Rationale:** Foundation for all other features. The agent loop is the non-negotiable core that makes an LLM an agent. Without tool dispatch, the loop cannot execute actions.
**Delivers:** Working agent that can think, act, and observe. Basic CLI interaction. First tools (bash, read, write).
**Addresses:** Table stakes features (Agent Loop, Tool Dispatch, File Tools, Shell Execution, CLI)
**Avoids:** Prompt plumbing anti-pattern (clean loop with tool dispatch, not routing logic)
**Stack:** tokio async runtime, rig-core LLM abstraction, clap CLI, tracing observability

### Phase 2: Planning & Knowledge (s03-s06)
**Rationale:** Once core loop works, add mechanisms for task planning (TodoWrite), subtask delegation (subagents), modular expertise (skills), and session longevity (compression). These features multiply agent effectiveness.
**Delivers:** 2x task completion (TodoWrite), clean context for subtasks (subagents), reduced context bloat (skills), infinite sessions (compression).
**Addresses:** Differentiator features (TodoWrite, Subagents, Skill Loading, Context Compression)
**Avoids:** Context rot (three-layer compression), context pollution (subagent isolation), system prompt stuffing (on-demand skills)
**Stack:** Uses core loop + adds uuid for agent IDs, chrono for timestamps, tokio::sync channels for coordination

### Phase 3: Persistence (s07-s08)
**Rationale:** With planning mechanisms in place, add persistence for goals across sessions (task graph) and non-blocking execution (background tasks). Enables resumable sessions and async productivity.
**Delivers:** Persistent goals with dependencies, non-blocking long-running operations, session continuity.
**Addresses:** Differentiator features (Task Graph, Background Tasks, Sandbox Security)
**Avoids:** Lost state on failure (file-based persistence), blocking operations (background execution), untrusted tool execution (sandbox)
**Stack:** serde_json for JSONL persistence, tokio::process for daemon management, nix for Linux sandboxing

### Phase 4: Teams (s09-s12)
**Rationale:** Only after single-agent harness is validated, add multi-agent coordination. Most users don't need team orchestration initially, and multi-agent adds significant complexity.
**Delivers:** Scale beyond single agent, specialized teammates, parallel execution without conflicts, autonomous coordination.
**Addresses:** Future differentiator features (Multi-Agent Teams, Team Protocols, Autonomous Claiming, Worktree Isolation)
**Avoids:** Over-engineered multi-agent for simple tasks (defer to Phase 4), file conflicts (worktree isolation), coordination bottlenecks (autonomous claiming)
**Stack:** JSONL mailboxes for async communication, git worktrees for isolation, tokio::sync::broadcast for team events

### Cross-Cutting (Build Throughout)
- **LLM Provider Abstraction:** Support all phases with multi-backend capability
- **CLI Interface:** Grows with features, interactive mode evolves
- **Sandbox/Permissions:** Safety from day one, escalates with capabilities
- **Web Visualization:** Optional SvelteKit frontend, can be deferred to Phase 3+

### Phase Ordering Rationale

- **Dependency-driven:** Tool dispatch (s02) must exist before TodoWrite (s03) can use it; subagent spawning (s04) enables multi-agent teams (s09); task graph (s07) enables autonomous claiming (s11)
- **Risk mitigation:** Context compression (s06) comes early to prevent session death; sandbox security integrated throughout to prevent catastrophic failures
- **Value delivery:** Each phase delivers usable functionality - Phase 1 gives working agent, Phase 2 multiplies effectiveness, Phase 3 adds persistence, Phase 4 scales to teams
- **Complexity management:** Single-agent validation before multi-agent; proven patterns before experimental features

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (s07-s08):** Sandbox implementation requires platform-specific research (seccomp-bpf on Linux, alternatives on macOS/Windows). Background task notification injection patterns need API design research.
- **Phase 4 (s09-s12):** Multi-agent coordination protocols need deeper research into consensus algorithms, conflict resolution, and team communication patterns. Worktree isolation requires git internals research.

Phases with standard patterns (skip research-phase):
- **Phase 1 (s01-s02):** Agent loop and tool dispatch are well-documented patterns with clear implementations. Rig-core documentation provides sufficient guidance.
- **Phase 2 (s03-s06):** TodoWrite, subagent spawning, and context compression have established patterns from Claude Code reference implementation.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All recommended crates are mature, well-documented, and production-ready. Official sources (tokio.rs, docs.rig.rs, rmcp GitHub) provide high-confidence information. async-std discontinuation confirmed by community announcement. |
| Features | HIGH | Feature analysis based on reference implementation (learn-claude-code), competitor analysis (LangChain, CrewAI, AutoGen, Claude Code), and production research. Table stakes vs differentiators clearly identified with complexity estimates. |
| Architecture | HIGH | Architecture patterns derived from Anthropic engineering blog, learn-claude-code curriculum, and established frameworks. Component responsibilities and data flows are well-documented. Anti-patterns clearly identified. |
| Pitfalls | MEDIUM | Pitfalls extracted from architecture and features research (anti-patterns, failure modes). PITFALLS.md was empty, but relevant information was available in other research files. Production failure mode analysis provides practical guidance. |

**Overall confidence:** HIGH

The research is comprehensive and based on authoritative sources including official documentation (tokio.rs, docs.rig.rs, rmcp GitHub), reference implementations (learn-claude-code curriculum), and production experience (Anthropic engineering blog, Firecrawl analysis). The 12-session curriculum provides a proven build order with clear dependencies.

### Gaps to Address

**Minor gaps (can be resolved during implementation):**
- **PITFALLS.md incomplete:** While critical pitfalls were extracted from architecture and features research, a dedicated pitfalls research document would provide deeper analysis. Handle by referencing anti-patterns section in ARCHITECTURE.md during planning.
- **Platform-specific sandboxing:** Linux seccomp-bpf implementation is documented, but macOS/Windows sandboxing alternatives need research during Phase 3 planning. Handle by scoping Phase 3 research to sandbox implementation patterns.
- **Web visualization architecture:** SvelteKit integration patterns mentioned but not deeply researched. Handle by treating web frontend as optional enhancement, not core requirement.

**No critical gaps identified.** The research provides sufficient information to proceed with roadmap creation and Phase 1 implementation.

## Sources

### Primary (HIGH confidence)
- **learn-claude-code GitHub Repository** (https://github.com/shareAI-lab/learn-claude-code) — Reference curriculum for harness engineering, 12-session build order
- **rig.rs documentation** (https://docs.rig.rs/) — Official Rig framework documentation, multi-provider LLM abstraction
- **rmcp GitHub** (https://github.com/modelcontextprotocol/rust-sdk) — Official MCP Rust SDK, protocol implementation
- **Tokio documentation** (https://tokio.rs/tokio/tutorial) — Official Tokio tutorial, async runtime patterns
- **Anthropic Engineering Blog** (https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) — Harness architecture patterns from official source

### Secondary (MEDIUM confidence)
- **What Is an Agent Harness? - Firecrawl** (https://www.firecrawl.dev/blog/what-is-an-agent-harness) — Comprehensive harness architecture analysis
- **Top 10 AI Agent Frameworks 2026 - xpay** (https://www.xpay.sh/blog/article/top-ai-agent-frameworks/) — Framework comparison with MCP trends
- **Multi-Agent Orchestration Patterns 2026 - AI Agents Plus** (https://www.ai-agentsplus.com/blog/multi-agent-orchestration-patterns-2026) — Orchestration patterns catalog
- **LangChain: Choosing the Right Multi-Agent Architecture** (https://blog.langchain.com/choosing-the-right-multi-agent-architecture/) — Established framework perspective
- **Reddit: async-std discontinued** (https://www.reddit.com/r/rust/comments/1jc6gis/psa_asyncstd_has_been_officially_discontinued_use/) — Community announcement confirming async-std status
- **tracing vs log** (https://tokio.rs/tokio/topics/tracing) — Official Tokio tracing guide

### Tertiary (Context/Validation)
- **HuggingFace: Design Patterns for Building Agentic Workflows** (https://huggingface.co/blog/dcarpintero/design-patterns-for-building-agentic-workflows) — Industry analysis
- **AI Agent Memory Frameworks 2026 - Machine Learning Mastery** (https://machinelearningmastery.com/the-6-best-ai-agent-memory-frameworks-you-should-try-in-2026/) — Memory system analysis
- **Rust LLM ecosystem overview** (https://hackmd.io/@Hamze/Hy5LiRV1gg) — Community-maintained list
- **genai crate** (https://lib.rs/crates/genai) — Multi-provider alternative to rig-core
- **AutoAgents documentation** (https://liquidos-ai.github.io/AutoAgents/) — Multi-agent framework alternative

---
*Research completed: 2026-03-20*
*Ready for roadmap: yes*
