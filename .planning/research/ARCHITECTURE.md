# Architecture Research: AI Agent Harness Systems

**Domain:** AI Agent Harness (Rust Implementation)
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

An AI agent harness is the infrastructure that surrounds a language model to enable it to perceive, reason, and act in a specific domain. The core insight from the learn-claude-code curriculum and Anthropic's engineering blog is: **The model IS the agent. The code is the harness.**

The harness provides everything the agent needs to function:
```
Harness = Tools + Knowledge + Observation + Action Interfaces + Permissions
```

This research synthesizes patterns from Claude Code, LangChain, Anthropic's Agent SDK, Rig (Rust), and the broader agent ecosystem to define a recommended architecture for a production Rust agent harness.

## Standard Architecture

### System Overview

```
+------------------------------------------------------------------+
|                        CLI / Web Interface                        |
|   (Command parsing, session management, user interaction)         |
+------------------------------------------------------------------+
                                |
                                v
+------------------------------------------------------------------+
|                       Agent Loop (Core)                           |
|   +----------------------------------------------------------+   |
|   |  while True:                                             |   |
|   |      response = LLM(messages, tools)                     |   |
|   |      if stop_reason != "tool_use": return                |   |
|   |      results = execute_tools(response.tool_calls)         |   |
|   |      messages.append(results)                            |   |
|   +----------------------------------------------------------+   |
+------------------------------------------------------------------+
        |                    |                    |
        v                    v                    v
+---------------+  +------------------+  +------------------+
| Tool Dispatch |  | Context Manager  |  | LLM Provider     |
| (Handler Map) |  | (Memory/Compact) |  | (Multi-Backend)  |
+---------------+  +------------------+  +------------------+
        |                    |                    |
        v                    v                    v
+------------------------------------------------------------------+
|                        Harness Components                         |
+------------------------------------------------------------------+
|  +------------+  +------------+  +------------+  +-------------+ |
|  |   Tools    |  |   Skills   |  |  Subagents |  |    Tasks    | |
|  | (Handlers) |  | (On-Demand)|  | (Spawning) |  | (Graph/FS)  | |
|  +------------+  +------------+  +------------+  +-------------+ |
|                                                                   |
|  +------------+  +------------+  +------------+  +-------------+ |
|  |  Sandbox   |  |   Teams    |  | Background |  |  Worktree   | |
|  | (Perms)    |  | (Mailbox)  |  | (Async)    |  | (Isolation) | |
|  +------------+  +------------+  +------------+  +-------------+ |
+------------------------------------------------------------------+
                                |
                                v
+------------------------------------------------------------------+
|                     External Systems / APIs                       |
|   (Filesystem, Shell, Network, Database, MCP Servers, LLM APIs)   |
+------------------------------------------------------------------+
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **Agent Loop** | Core orchestration; calls LLM, handles stop_reason, dispatches tools | Single async loop with stop_reason branching |
| **Tool Dispatch** | Maps tool names to handler functions; executes and returns results | HashMap<String, AsyncHandler> pattern |
| **LLM Provider** | Abstracts LLM API calls; handles streaming, errors, retries | Trait-based multi-backend (Anthropic, OpenAI, Ollama) |
| **Context Manager** | Manages message history; implements compression/compaction | Three-layer strategy: summarize, truncate, persist |
| **Tools** | Atomic actions the agent can perform (read, write, bash, etc.) | Handler functions with input validation |
| **Skills** | On-demand knowledge/prompts loaded via tool_result | File-based skill directories loaded when needed |
| **Subagents** | Spawned agents with isolated context for subtasks | Fresh messages[] per child, results merged back |
| **Tasks** | Persistent goal tracking with dependencies | File-based graph (JSON/JSONL) |
| **Sandbox** | Permission governance; approval workflows | Trust boundaries, confirm prompts |
| **Teams** | Multi-agent coordination | Async JSONL mailboxes per agent |
| **Background** | Daemon execution for long-running operations | Tokio tasks with notification injection |
| **Worktree** | Directory isolation for parallel execution | Git worktree or isolated directories |

## Recommended Project Structure

```
harness/
+-- Cargo.toml
+-- src/
|   +-- main.rs                    # CLI entry point
|   +-- lib.rs                     # Library root
|   +-- agent/
|   |   +-- mod.rs                 # Agent module root
|   |   +-- loop.rs                # Core agent loop
|   |   +-- message.rs             # Message types (user, assistant, tool_result)
|   |   +-- stop_reason.rs         # Stop reason handling
|   |
|   +-- tools/
|   |   +-- mod.rs                 # Tool registry and dispatch
|   |   +-- handler.rs             # Handler trait
|   |   +-- bash.rs                # Shell execution tool
|   |   +-- read.rs                # File read tool
|   |   +-- write.rs               # File write tool
|   |   +-- edit.rs                # File edit tool
|   |   +-- glob.rs                # File pattern matching
|   |   +-- grep.rs                # Content search
|   |   +-- browser.rs             # Web interaction
|   |   +-- mcp.rs                 # MCP protocol tools
|   |
|   +-- llm/
|   |   +-- mod.rs                 # LLM provider abstraction
|   |   +-- provider.rs            # Provider trait
|   |   +-- anthropic.rs           # Anthropic API client
|   |   +-- openai.rs              # OpenAI API client
|   |   +-- ollama.rs              # Local model client
|   |   +-- streaming.rs           # Streaming response handling
|   |
|   +-- context/
|   |   +-- mod.rs                 # Context management
|   |   +-- history.rs             # Message history
|   |   +-- compression.rs         # Three-layer compression
|   |   +-- window.rs              # Context window management
|   |
|   +-- planning/
|   |   +-- mod.rs                 # Planning module
|   |   +-- todo.rs                # TodoWrite with nag reminders
|   |   +-- task.rs                # Task graph with dependencies
|   |   +-- skill.rs               # On-demand skill loading
|   |
|   +-- subagent/
|   |   +-- mod.rs                 # Subagent spawning
|   |   +-- spawner.rs             # Context isolation and spawning
|   |   +-- merge.rs               # Result merging
|   |
|   +-- team/
|   |   +-- mod.rs                 # Multi-agent coordination
|   |   +-- mailbox.rs             # JSONL async mailboxes
|   |   +-- protocol.rs            # Communication protocols (shutdown, approval FSM)
|   |   +-- autonomous.rs          # Idle cycle + auto-claim
|   |
|   +-- background/
|   |   +-- mod.rs                 # Background task execution
|   |   +-- daemon.rs              # Daemon thread management
|   |   +-- notify.rs              # Notification injection
|   |
|   +-- isolation/
|   |   +-- mod.rs                 # Execution isolation
|   |   +-- worktree.rs            # Worktree management
|   |   +-- sandbox.rs             # Permission boundaries
|   |
|   +-- cli/
|   |   +-- mod.rs                 # CLI interface
|   |   +-- args.rs                # Argument parsing (clap)
|   |   +-- session.rs             # Session management
|   |   +-- interactive.rs         # Interactive mode
|   |
|   +-- web/                       # (Optional SvelteKit frontend)
|       +-- src/
|       +-- package.json
|
+-- skills/                        # Skill definitions
|   +-- coding/
|   |   +-- SKILL.md
|   +-- research/
|   |   +-- SKILL.md
|
+-- tests/
|   +-- integration/
|   +-- e2e/
```

### Structure Rationale

- **agent/**: Core loop is sacred -- everything else supports it. The loop belongs to the agent.
- **tools/**: Each tool is one handler. Adding a tool = adding one handler to the dispatch map.
- **llm/**: Provider trait enables multi-backend support without vendor lock-in.
- **context/**: Context management is critical for long-running sessions; three-layer compression prevents overflow.
- **planning/**: TodoWrite and Tasks are separate concerns -- TodoWrite is in-memory planning, Tasks are persistent goal tracking.
- **subagent/**: Isolated from main agent; subagents get fresh messages[] to prevent context pollution.
- **team/**: Multi-agent coordination is distinct from single-agent subtask delegation.
- **isolation/**: Worktree and sandbox are security/correctness boundaries.

## Architectural Patterns

### Pattern 1: The Agent Loop

**What:** The fundamental pattern all agent harnesses implement. The loop calls the LLM, checks stop_reason, and either returns text or executes tools.

**When to use:** Always. This is the non-negotiable core.

**Trade-offs:** Simple but requires careful handling of edge cases (streaming, errors, timeouts).

```rust
pub async fn agent_loop(
    messages: &mut Vec<Message>,
    tools: &[Tool],
    provider: &dyn LlmProvider,
) -> Result<String, AgentError> {
    loop {
        let response = provider
            .create_message(messages.clone(), tools)
            .await?;

        messages.push(Message::assistant(response.content.clone()));

        match response.stop_reason {
            StopReason::EndTurn | StopReason::StopSequence => {
                return Ok(response.text_content());
            }
            StopReason::ToolUse => {
                let results = execute_tools(&response.tool_calls).await?;
                messages.push(Message::user_tool_results(results));
            }
            StopReason::MaxTokens => {
                // Trigger compression
                compress_context(messages)?;
            }
        }
    }
}
```

### Pattern 2: Tool Dispatch (Handler Registry)

**What:** A registry mapping tool names to handler functions. Adding a tool means adding one entry to the map.

**When to use:** All tools should be registered this way for extensibility.

**Trade-offs:** Requires upfront registration but enables dynamic tool discovery and MCP integration.

```rust
pub struct ToolRegistry {
    handlers: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn register(&mut self, name: &str, handler: Box<dyn ToolHandler>) {
        self.handlers.insert(name.to_string(), handler);
    }

    pub async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult, ToolError> {
        match self.handlers.get(&tool_call.name) {
            Some(handler) => handler.execute(tool_call.input.clone()).await,
            None => Err(ToolError::UnknownTool(tool_call.name.clone())),
        }
    }
}

// Usage: Adding a tool = one handler
registry.register("bash", Box::new(BashHandler::new()));
registry.register("read", Box::new(ReadHandler::new()));
```

### Pattern 3: Subagent Spawning (Context Isolation)

**What:** Spawn child agents with fresh message arrays to prevent intermediate noise from polluting main context.

**When to use:** Complex subtasks that would generate many intermediate messages (research, multi-file refactoring).

**Trade-offs:** Extra model calls but keeps main context clean. Results are summarized back.

```rust
pub struct SubagentSpawner {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
}

impl SubagentSpawner {
    pub async fn spawn(
        &self,
        task: String,
        tools: Vec<String>, // Filtered tool access
        system_prompt: String,
    ) -> Result<SubagentResult, SpawnError> {
        // Fresh messages[] -- isolated context
        let mut messages = vec![Message::user(task)];

        // Filtered tool access
        let available_tools: Vec<Tool> = self.tool_registry
            .get_subset(&tools);

        // Run isolated agent
        let result = agent_loop(
            &mut messages,
            &available_tools,
            self.provider.as_ref(),
        ).await?;

        Ok(SubagentResult {
            output: result,
            turns: messages.len(),
        })
    }
}
```

### Pattern 4: Three-Layer Context Compression

**What:** Hierarchical compression strategy to handle context overflow: (1) summarize old turns, (2) truncate verbose outputs, (3) persist to disk.

**When to use:** Long-running sessions that approach context limits.

**Trade-offs:** Compression is lossy; important context may be lost. Requires careful prompt engineering for summarization.

```rust
pub struct ContextCompressor {
    summarizer: Arc<dyn LlmProvider>,
    persistence: ContextPersistence,
}

impl ContextCompressor {
    pub async fn compress(&self, messages: &mut Vec<Message>) -> Result<(), CompressError> {
        // Layer 1: Summarize old conversation turns
        if messages.len() > TURNS_THRESHOLD {
            let summary = self.summarize_turns(&messages[..OLD_TURNS]).await?;
            messages.splice(0..OLD_TURNS, vec![Message::system_summary(summary)]);
        }

        // Layer 2: Truncate verbose tool outputs
        for msg in messages.iter_mut() {
            if msg.estimated_tokens() > MAX_OUTPUT_TOKENS {
                msg.truncate_output(MAX_OUTPUT_TOKENS);
            }
        }

        // Layer 3: Persist oldest to disk, keep reference
        if messages.len() > PERSIST_THRESHOLD {
            let persisted = self.persistence.save(&messages[..PERSIST_COUNT]).await?;
            messages.splice(0..PERSIST_COUNT, vec![Message::reference(persisted)]);
        }

        Ok(())
    }
}
```

### Pattern 5: On-Demand Skill Loading

**What:** Load specialized knowledge/prompts via tool_result rather than stuffing everything in system prompt.

**When to use:** Agents with many possible specializations; skills loaded only when relevant.

**Trade-offs:** Requires model to know skill names upfront; adds one turn per skill load.

```rust
pub struct SkillLoader {
    skills_dir: PathBuf,
    loaded_skills: HashSet<String>,
}

impl ToolHandler for SkillLoader {
    async fn execute(&self, input: Value) -> Result<ToolResult, ToolError> {
        let skill_name = input["skill"].as_str().ok_or(ToolError::MissingInput)?;

        let skill_path = self.skills_dir.join(skill_name).join("SKILL.md");
        let content = fs::read_to_string(skill_path).await?;

        // Inject via tool_result, not system prompt
        Ok(ToolResult::success(content))
    }
}
```

### Pattern 6: Multi-Agent Team (Async Mailbox)

**What:** Persistent teammates with async JSONL mailboxes for coordination. Agents communicate by writing to each other's mailboxes.

**When to use:** Complex projects requiring specialized agents working in parallel.

**Trade-offs:** Complex coordination logic; requires protocol design (shutdown, approval FSMs).

```rust
pub struct AgentMailbox {
    inbox: PathBuf,  // JSONL file
    agent_id: String,
}

impl AgentMailbox {
    pub async fn send(&self, message: TeamMessage) -> Result<(), MailboxError> {
        let line = serde_json::to_string(&message)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.inbox)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }

    pub async fn receive(&self) -> Result<Vec<TeamMessage>, MailboxError> {
        let content = fs::read_to_string(&self.inbox).await?;
        let messages: Vec<TeamMessage> = content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        Ok(messages)
    }
}
```

## Data Flow

### Request Flow (Single Agent)

```
User Input
    |
    v
[CLI/Web Interface] --> Parse command, create Message
    |
    v
[Agent Loop] --> Call LLM Provider
    |
    v
[LLM Response] --> Check stop_reason
    |                    |
    | tool_use           | end_turn
    v                    v
[Tool Dispatch]     [Return to User]
    |
    v
[Execute Handler] --> Bash/Read/Write/etc.
    |
    v
[Tool Result] --> Append to messages[]
    |
    v
[Loop Back] --> Call LLM again
```

### Subagent Flow

```
Main Agent receives complex task
    |
    v
[Subagent Tool Call] --> "delegate_task"
    |
    v
[Spawner] --> Create fresh messages[]
    |           Filter tools
    |           Set system prompt
    v
[Child Agent Loop] --> Execute in isolation
    |
    v
[Result Summary] --> Return to main agent
    |
    v
[Main Agent] --> Continue with summarized result
```

### Team Coordination Flow

```
[Lead Agent] reads task board
    |
    v
[Task Assignment] --> Write to teammate mailbox
    |
    v
[Teammate Agent] reads mailbox (polling/event)
    |
    v
[Execute Task] --> May spawn own subagents
    |
    v
[Post Result] --> Write to lead's mailbox
    |
    v
[Lead Agent] synthesizes results
```

### Key Data Flows

1. **Message Flow:** User -> messages[] -> LLM -> response -> tools -> tool_results -> messages[] -> (loop)
2. **Context Compression:** messages[] -> compress -> summarized_messages[] -> messages[]
3. **Subagent Isolation:** main_messages[] -> spawn -> child_messages[] -> execute -> summary -> main_messages[]
4. **Team Communication:** agent_a.mailbox <- JSONL <- agent_b.mailbox

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| **Single user, single session** | Monolith is fine. Single Tokio runtime, in-memory context. |
| **Single user, multi-session** | Add session persistence. Context compression becomes critical. |
| **Multi-user, concurrent** | Connection pooling for LLM APIs. Per-user worktrees. Rate limiting. |
| **Team of agents** | Full mailbox protocol. Worktree isolation per agent. Background task queue. |

### Scaling Priorities

1. **First bottleneck: Context window.** Implement three-layer compression early. Without it, sessions die at ~50-100 turns.
2. **Second bottleneck: LLM API rate limits.** Implement request queuing and retry with exponential backoff.
3. **Third bottleneck: Concurrent file access.** Use worktree isolation to prevent conflicts when running parallel agents.

## Anti-Patterns

### Anti-Pattern 1: Prompt Plumbing

**What people do:** Wire together LLM API calls with if-else branches, node graphs, and hardcoded routing logic, calling it an "agent."

**Why it's wrong:** The agent is the model, not the plumbing. Rube Goldberg machines are fragile, unscalable, and fundamentally incapable of generalization.

**Do this instead:** Build a clean agent loop with tools. Let the model decide. Add harness mechanisms, not routing logic.

### Anti-Pattern 2: Stuffing Everything in System Prompt

**What people do:** Put all knowledge, rules, and instructions in the system prompt upfront.

**Why it's wrong:** Context bloat. The model wastes tokens processing irrelevant information. Context window fills before useful work begins.

**Do this instead:** Use on-demand skill loading. The model knows skill names, loads content via tool_result when needed.

### Anti-Pattern 3: No Context Isolation for Subtasks

**What people do:** Run subtasks in the same message array as the main conversation.

**Why it's wrong:** Intermediate noise pollutes main context. Each subtask adds dozens of turns, exhausting context rapidly.

**Do this instead:** Spawn subagents with fresh messages[]. Return only the summary to main context.

### Anti-Pattern 4: Trusting Tool Outputs Blindly

**What people do:** Execute any tool the model requests without validation.

**Why it's wrong:** Models can hallucinate dangerous commands. rm -rf, API key leaks, etc.

**Do this instead:** Sandbox-first design. Require explicit approval for destructive operations. Validate inputs.

### Anti-Pattern 5: Blocking on Long-Running Operations

**What people do:** await bash commands that take minutes, blocking the agent loop.

**Why it's wrong:** Agent can't think while waiting. Can't make progress on other tasks.

**Do this instead:** Background task execution. Daemon threads run commands, inject notifications when complete.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| **Anthropic API** | Provider trait implementation | Primary LLM backend; streaming support |
| **OpenAI API** | Provider trait implementation | Alternative backend |
| **Ollama/Local** | Provider trait implementation | Privacy-first, offline capability |
| **MCP Servers** | Tool discovery + execution | Protocol for external tool integration |
| **Git** | Shell tool wrapper | Version control for agent changes |
| **Filesystem** | Native Rust std::fs | Core tool for file I/O |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| **Agent Loop <-> Tool Dispatch** | Direct function call | Synchronous dispatch |
| **Agent Loop <-> LLM Provider** | Async trait | Provider abstraction |
| **Agent Loop <-> Context Manager** | Mutable reference | Context compression |
| **Main Agent <-> Subagents** | Message passing | Isolated contexts |
| **Team Agents <-> Mailboxes** | JSONL file I/O | Async coordination |
| **Agent <-> Background Tasks** | Notification queue | Non-blocking execution |

## Build Order Implications

Based on the architecture and dependencies, recommended build order:

### Phase 1: Core Loop (s01-s02)
**Dependencies:** None
**Build first:**
1. Message types (user, assistant, tool_result)
2. Stop reason handling
3. Basic agent loop
4. Tool dispatch registry
5. First tools: bash, read, write

### Phase 2: Planning & Knowledge (s03-s06)
**Dependencies:** Core loop must work
**Build:**
1. TodoWrite with nag reminders (s03)
2. Subagent spawning (s04)
3. On-demand skill loading (s05)
4. Context compression (s06) -- critical for session longevity

### Phase 3: Persistence (s07-s08)
**Dependencies:** Planning layer
**Build:**
1. File-based task graph (s07)
2. Background task execution (s08)

### Phase 4: Teams (s09-s12)
**Dependencies:** All previous phases
**Build:**
1. Multi-agent teams with mailboxes (s09)
2. Team communication protocols (s10)
3. Autonomous task claiming (s11)
4. Worktree isolation (s12)

### Cross-Cutting (Build in Parallel)
- LLM provider abstraction (supports all phases)
- CLI interface (grows with features)
- Sandbox/permissions (safety from day one)
- Web visualization (optional, can be deferred)

## Sources

- [Anthropic Engineering: Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) (HIGH confidence - official)
- [learn-claude-code: Harness Engineering for Real Agents](https://github.com/shareAI-lab/learn-claude-code) (HIGH confidence - reference curriculum)
- [LangChain: Choosing the Right Multi-Agent Architecture](https://blog.langchain.com/choosing-the-right-multi-agent-architecture/) (HIGH confidence - established framework)
- [HuggingFace: Design Patterns for Building Agentic Workflows](https://huggingface.co/blog/dcarpintero/design-patterns-for-building-agentic-workflows) (MEDIUM confidence - industry analysis)
- [FutureAGI: LLM Agent Architectures Core Components](https://futureagi.com/blogs/llm-agent-architectures-core-components) (MEDIUM confidence - synthesis)
- [VoltAgent: LLM Agent Framework](https://voltagent.dev/blog/llm-agent-framework/) (MEDIUM confidence - framework documentation)
- [Rig: Rust AI Agent Framework](https://github.com/0xplaygrounds/rig) (MEDIUM confidence - Rust ecosystem)

---
*Architecture research for: Rust Agent Harness*
*Researched: 2026-03-20*
