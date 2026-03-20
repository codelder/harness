# Feature Research

**Domain:** AI Agent Harness (Rust-based)
**Researched:** 2026-03-20
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Agent Loop with stop_reason handling | Core mechanism that makes an LLM an agent; every agent needs the think-act-observe cycle | LOW | Under 50 lines; while loop + tool_use detection |
| Tool Dispatch System | Without tools, agent cannot interact with the world; dispatch maps tool names to handlers | LOW | Simple name-to-handler map; extensible registration |
| File Read/Write Tools | Agents need to read codebases and write changes; fundamental capability | LOW | Standard file I/O wrappers |
| Shell/Bash Execution | Agents need to run commands (tests, builds, git); primary action interface | MEDIUM | Requires sandboxing for safety |
| Multi-LLM Backend Support | Users have different model preferences (Anthropic, OpenAI, local); provider flexibility | MEDIUM | Trait abstraction for LLM backends |
| CLI Interface | Command-line access is the primary interface for developer tools | MEDIUM | Interactive mode + one-shot commands |
| Context/Message Management | Accumulating conversation history; foundation for all other features | LOW | List of messages with role/content |
| Error Handling & Recovery | Agents fail; system needs graceful recovery, not crashes | MEDIUM | Try/catch, retry logic, timeout handling |
| Basic Observability (Logging) | Developers need to see what the agent is doing; debugging impossible without | LOW | Structured logging, step tracing |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Rust Performance & Memory Safety | Zero-cost abstractions, no GC pauses, memory-safe execution; superior for long-running sessions | N/A (language choice) | Core differentiator from Python implementations |
| TodoWrite with Nag Reminders | 2x task completion rate; agents with plans don't drift; reminders keep focus | MEDIUM | TodoManager + periodic check-ins |
| Subagent Spawning with Isolated Context | Clean context per subtask; main conversation stays focused; parallel execution potential | HIGH | Fresh messages[] per child agent |
| On-Demand Skill Loading (via tool_result) | Load knowledge when needed, not upfront; reduces context bloat; modular expertise | MEDIUM | SKILL.md files injected dynamically |
| Three-Layer Context Compression | Infinite sessions without context explosion; handles long-running tasks | HIGH | Summarization + retrieval + structured state |
| File-Based Task Graph with Dependencies | Persistent goals across sessions; ordered execution; foundation for multi-agent | HIGH | JSON task files with dependency edges |
| Background Task Execution + Notifications | Non-blocking slow operations; agent keeps thinking; async productivity | MEDIUM | Daemon threads + notification queue |
| Multi-Agent Teams with Async JSONL Mailboxes | Scale beyond single agent; specialized teammates; persistent communication | HIGH | Agent teammates + mailbox files |
| Team Communication Protocols | Shared rules for negotiation; shutdown handling; plan approval FSM | HIGH | Request-response patterns, state machines |
| Autonomous Task Claiming | Idle agents self-assign work; no central bottleneck; emergent coordination | MEDIUM | Idle cycle + board scanning |
| Worktree Isolation for Parallel Execution | No file conflicts; true parallelism; safe concurrent development | HIGH | Git worktrees + task-directory binding |
| MCP (Model Context Protocol) Support | Standard tool ecosystem; 270+ MCP servers available; future-proof extensibility | MEDIUM | Protocol implementation for tool discovery |
| Sandbox-First Security | Safe by default; explicit trust escalation; interactive confirmation for dangerous ops | HIGH | Docker/container isolation + permission prompts |
| SvelteKit Web Visualization | Real-time agent state visibility; session replay; team coordination dashboard | HIGH | Web UI for observability beyond CLI |
| Human-in-the-Loop Approval Workflows | Safe automation; explicit confirmation for high-risk actions; trust boundaries | MEDIUM | Approval queues + confidence thresholds |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Real-time Everything / No Async | Simpler mental model; immediate feedback | Explodes complexity; blocking on slow operations; poor UX for long tasks | Background tasks + notifications; async-first design |
| Over-Engineered Multi-Agent for Simple Tasks | Impressive demos; future-proofing | 90% of tasks don't need orchestration; adds latency and failure points | Start single-agent; add multi-agent only when proven necessary |
| Massive Tool Library Upfront | More capabilities = better agent | Tool bloat causes agent confusion; maintenance burden; context pollution | Minimal core tools + on-demand skill loading |
| Complex Decision Trees / Rule-Based Routing | Control freak mindset; predictable behavior | Reimplements GOFAI; brittle; can't generalize; fights model intelligence | Trust the model; build harness mechanisms, not decision logic |
| No Sandbox / Full System Access | Simplicity; no friction | Security nightmare; accidental destruction; no trust boundaries | Sandbox-first with interactive escalation |
| Global State / Shared Memory Everywhere | Easy data sharing | Context pollution; cascade failures; hard to debug isolation | Clean context boundaries; explicit state passing |
| Training/Fine-Tuning in the Harness | Custom behavior; competitive moat | Harness is infrastructure, not intelligence; wrong abstraction layer | Focus on harness engineering; model training is separate concern |
| Mobile-First UI | Market expansion | Agent harnesses are developer tools; mobile adds complexity without value | Web-first, mobile-responsive later; CLI is primary |

## Feature Dependencies

```
Agent Loop (s01)
    └──requires──> Tool Dispatch (s02)
                       │
                       ├──enables──> TodoWrite (s03)
                       │                └──enables──> Task Graph (s07)
                       │
                       ├──enables──> Subagent Spawning (s04)
                       │                └──enables──> Multi-Agent Teams (s09)
                       │
                       ├──enables──> Skill Loading (s05)
                       │
                       └──enables──> Context Compression (s06)

Task Graph (s07)
    └──enables──> Background Tasks (s08)
    └──enables──> Autonomous Claiming (s11)
    └──enables──> Worktree Isolation (s12)

Multi-Agent Teams (s09)
    └──requires──> Team Protocols (s10)
    └──enables──> Autonomous Claiming (s11)
    └──enables──> Worktree Isolation (s12)

MCP Support
    └──enhances──> Tool Dispatch (s02)
    └──enhances──> Skill Loading (s05)

Sandbox Security
    └──wraps──> Shell Execution
    └──wraps──> File Operations
    └──conflicts──> Unrestricted System Access

Web Visualization
    └──requires──> Observability/Tracing
    └──enhances──> Multi-Agent Teams (visibility)
```

### Dependency Notes

- **Agent Loop requires Tool Dispatch:** The loop needs handlers for tool_use blocks; without dispatch, loop cannot execute actions
- **Subagent Spawning enables Multi-Agent Teams:** Teammates are spawned subagents with persistent identities; fresh context per agent is the foundation
- **Task Graph enables Autonomous Claiming:** Agents need a shared task board to scan and claim; file-based persistence enables cross-session visibility
- **MCP enhances Tool Dispatch:** MCP provides standard tool discovery; integrates with existing dispatch mechanism
- **Sandbox conflicts with Unrestricted Access:** Security model choice; cannot have both safe-by-default and full system access
- **Context Compression enables Infinite Sessions:** Without compression, context fills and agent loses earlier context; critical for long-running tasks
- **Team Protocols require Multi-Agent Teams:** Communication rules only make sense when multiple agents exist to communicate

## MVP Definition

### Launch With (v1) - Phase 1-2 (s01-s06)

Minimum viable product -- what's needed to validate the concept.

- [x] Agent Loop with stop_reason handling (s01) -- Core mechanism; without this, not an agent
- [x] Tool Dispatch System (s02) -- Extensible tool registration; bash + file tools minimum
- [x] TodoWrite for Task Planning (s03) -- 2x completion improvement; plan-first execution
- [x] Subagent Spawning (s04) -- Isolated context for subtasks; foundation for teams
- [x] On-Demand Skill Loading (s05) -- Modular knowledge; reduces context bloat
- [x] Three-Layer Context Compression (s06) -- Enables long-running sessions
- [x] Multi-LLM Backend Support -- Provider flexibility; Anthropic + OpenAI minimum
- [x] CLI Interface -- Primary developer interface
- [x] Basic Observability -- Logging and tracing for debugging

**MVP Rationale:** Sessions s01-s06 cover the core harness mechanisms for a single agent. This validates the fundamental architecture before adding persistence (s07-s08) and teams (s09-s12). Users can accomplish real work with a single capable agent.

### Add After Validation (v1.x) - Phase 3 (s07-s08)

Features to add once core is working.

- [ ] File-Based Task Graph with Dependencies (s07) -- Persistent goals; enables session continuity
- [ ] Background Task Execution (s08) -- Non-blocking operations; async productivity
- [ ] Sandbox Security -- Docker isolation; permission prompts; safe defaults
- [ ] Web Visualization (basic) -- Real-time agent state visibility

**Trigger for adding:** Users report losing context between sessions; need for persistent task tracking; requests for safer execution environments.

### Future Consideration (v2+) - Phase 4 (s09-s12)

Features to defer until product-market fit is established.

- [ ] Multi-Agent Teams with Async Mailboxes (s09) -- Scale beyond single agent
- [ ] Team Communication Protocols (s10) -- Shutdown, plan approval FSM
- [ ] Autonomous Task Claiming (s11) -- Self-assigning idle agents
- [ ] Worktree Isolation (s12) -- Parallel execution without conflicts
- [ ] MCP Full Support -- Standard tool ecosystem integration
- [ ] Advanced Web Platform -- Session replay; team dashboards; analytics

**Why defer:** Multi-agent coordination adds significant complexity. Validate single-agent harness first. Most users don't need team orchestration initially.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Agent Loop (s01) | CRITICAL | LOW | P1 |
| Tool Dispatch (s02) | CRITICAL | LOW | P1 |
| Multi-LLM Backend | HIGH | MEDIUM | P1 |
| TodoWrite (s03) | HIGH | MEDIUM | P1 |
| Subagent Spawning (s04) | HIGH | HIGH | P2 |
| Skill Loading (s05) | MEDIUM | MEDIUM | P2 |
| Context Compression (s06) | HIGH | HIGH | P2 |
| Task Graph (s07) | HIGH | HIGH | P2 |
| Background Tasks (s08) | MEDIUM | MEDIUM | P3 |
| Sandbox Security | HIGH | HIGH | P2 |
| Multi-Agent Teams (s09) | MEDIUM | HIGH | P3 |
| Team Protocols (s10) | MEDIUM | HIGH | P3 |
| Autonomous Claiming (s11) | LOW | MEDIUM | P3 |
| Worktree Isolation (s12) | MEDIUM | HIGH | P3 |
| MCP Support | MEDIUM | MEDIUM | P3 |
| Web Visualization | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for launch (MVP)
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | LangChain/LangGraph | CrewAI | AutoGen | Claude Code | Our Approach |
|---------|---------------------|--------|---------|-------------|--------------|
| Agent Loop | Yes (Chains) | Yes | Yes | Yes | Yes (s01) |
| Tool Dispatch | 700+ integrations | Growing | Yes | Yes | Yes (s02) |
| Multi-Agent | LangGraph | Core focus | Core focus | Subagents | s09-s12 |
| Memory | Multiple types | Limited | Session | Context compression | 3-layer (s06) |
| Orchestration | Graph-based | Role-based | Conversation | Subagent delegation | Hierarchical + autonomous |
| MCP Support | Via tools | No | No | Native | Planned |
| Performance | Python (GIL) | Python | Python | Native | Rust (zero-cost) |
| Memory Safety | GC | GC | GC | Native | Rust (guaranteed) |
| Sandbox | External | No | Limited | Yes | Yes (core feature) |
| Observability | LangSmith | Basic | Limited | Built-in | Structured logging + web |

**Competitive Positioning:**
- vs Python frameworks: Rust provides performance and memory safety without sacrificing capability
- vs Claude Code: Open-source alternative with same architecture; customizable; no vendor lock-in
- vs CrewAI/AutoGen: Simpler mental model (trust the model); less orchestration overhead

## Key Insights from Research

### The Harness Philosophy (from learn-claude-code)

The fundamental insight from the reference implementation:

> "The model IS the agent. Not a framework. Not a prompt chain. Not a drag-and-drop workflow."

**Harness = Tools + Knowledge + Observation + Action Interfaces + Permissions**

The harness does NOT make the model smart. The model is already smart. The harness gives the model hands, eyes, and a workspace.

### What Makes a Great Harness (from Firecrawl analysis)

1. **Context Engineering** -- Deciding what to include and what to compress at each step
2. **Verification** -- Running tests, checking outputs, not trusting agent declarations
3. **State Persistence** -- Externalizing memory so sessions can resume
4. **Tool Validation** -- Intercepting and validating calls before execution
5. **Model Agnosticism** -- Harness logic separate from model choice

### Common Failure Modes (from production research)

| Failure Mode | Cause | Prevention |
|--------------|-------|-------------|
| Context Rot | Context fills with tool outputs; model loses original instructions | Context compression (s06) |
| Hallucinated Tool Calls | Agent calls non-existent APIs with wrong parameters | Tool validation layer |
| Lost State on Failure | Network timeout wipes in-memory progress | File-based persistence (s07) |
| Infinite Loops | No max iterations or timeouts | Iteration limits + escalation |
| Over-Communication | Agents passing full history; token explosion | Pass only necessary context |

## Sources

### High Confidence (Official Documentation, Primary Sources)
- [learn-claude-code GitHub Repository](https://github.com/shareAI-lab/learn-claude-code) - Reference curriculum for harness engineering
- [What Is an Agent Harness? - Firecrawl](https://www.firecrawl.dev/blog/what-is-an-agent-harness) - Comprehensive harness architecture analysis
- [Top 10 AI Agent Frameworks 2026 - xpay](https://www.xpay.sh/blog/article/top-ai-agent-frameworks/) - Framework comparison with MCP trends
- [Multi-Agent Orchestration Patterns 2026 - AI Agents Plus](https://www.ai-agentsplus.com/blog/multi-agent-orchestration-patterns-2026) - Orchestration patterns catalog

### Medium Confidence (Community Sources, Verified Claims)
- [AI Agent Memory Frameworks 2026 - Machine Learning Mastery](https://machinelearningmastery.com/the-6-best-ai-agent-memory-frameworks-you-should-try-in-2026/) - Memory system analysis
- [Evaluating AI Harness Dimensions - Agensi](https://www.agensi.io/skills/evaluating-ai-harness-dimensions) - Five-dimension evaluation framework
- [AI Agent Mistakes 2026 - Forbes/AgentWeb](https://www.forbes.com/sites/bernardmarr/2026/01/05/the-5-ai-agent-mistakes-that-could-cost-businesses-millions/) - Common pitfalls analysis
- [Human-in-the-Loop Patterns 2026](https://www.buildmvpfast.com/blog/human-in-the-loop-ai-agents-implementation-patterns-2026) - HITL implementation patterns

### Research Papers Referenced
- ICML 2025: "General Modular Harness for LLM Agents in Multi-Turn Gaming Environments" - Harness vs no-harness performance
- arXiv 2601.07190: "Active Context Compression: Autonomous Memory Management in LLM Agents"
- arXiv 2602.07072: "AgentSpawn: Adaptive Multi-Agent Collaboration Through Dynamic Spawning"

---
*Feature research for: Rust Agent Harness*
*Researched: 2026-03-20*
