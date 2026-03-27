# Phase 4: s04 - Subagents - Research

**Researched:** 2026-03-28
**Domain:** Subagent spawning with isolated context (Rust async, rig-core Tool trait)
**Confidence:** HIGH

## Summary

Phase 4 adds subagent capability to the Rust agent harness. The parent agent gains a `task` tool that spawns child agents with fresh message arrays. Child agents execute independently using the same tool set (minus the `task` tool to prevent recursion), and only their final text response is returned to the parent as a tool result. This is the core "context isolation" pattern described in CLAUDE.md anti-pattern #3.

The implementation centers on a `SubagentTool` struct that implements rig-core's `Tool` trait. When the LLM calls the `task` tool, the `call()` method creates a fresh child `LlmProvider` with all base tools except `task`, runs an isolated agent loop, and returns the summary text. The existing `agent_loop()` function from `src/agent/loop_.rs` can be reused directly since it already accepts arbitrary history slices and a provider reference.

The key architectural decision is how to construct child agents. The reference implementation (learn-claude-code s04) creates a new LLM client for each subagent invocation. In Rust, this means calling `create_provider()` inside the tool's `call()` method, which requires the tool to hold provider configuration (not a provider instance). The `SubagentTool` must carry `ProviderType`, model name, base URL, thinking config, and a `TodoManager` handle so it can build child agents on demand.

**Primary recommendation:** Implement `SubagentTool` as a rig `Tool` that holds provider configuration and constructs child `LlmProvider` instances on each invocation, reusing the existing `agent_loop()` function for isolated execution.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PLAN-02 | Task planning and delegation (subagent spawning) | SubagentTool implements rig Tool trait, spawned via agent_loop with fresh messages[] |
| CROSS-02 | Cross-cutting: error isolation between parent and child | SubagentTool::call() catches all child errors, returns error summary string instead of propagating |
| CROSS-03 | Cross-cutting: context isolation (fresh messages for child) | agent_loop() accepts &[Message] as history; child starts with empty history, only summary returns |
| CROSS-04 | Cross-cutting: result summarization | Child agent's final text response is the tool_result returned to parent; intermediate tool calls are discarded |

Note: `.planning/REQUIREMENTS.md` does not exist in this repository. Requirement IDs are inferred from ROADMAP.md references.
</phase_requirements>

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None -- discuss phase was skipped per workflow.skip_discuss setting.

### Claude's Discretion
All implementation choices are at Claude's discretion. Use ROADMAP phase goal, success criteria, and learn-claude-code reference to guide decisions.

Key reference: The learn-claude-code project's s04 session demonstrates subagent spawning patterns. Study its approach for context isolation, result summarization, and error handling.

### Deferred Ideas (OUT OF SCOPE)
None -- discuss phase skipped.
</user_constraints>

## Project Constraints (from CLAUDE.md)

- Must use `tokio::sync::mpsc` for async channels (NOT `std::sync::mpsc`)
- Must use `tracing` for logging (NOT `log` crate)
- Must use `thiserror` for library errors, `anyhow` for CLI
- Must NOT use `unwrap()` in library code
- Must NOT use `async-std`, `std::sync::mpsc`, `crossbeam-channel` in async contexts, `reqwest::blocking`
- Anti-pattern #3: "No Context Isolation -- Subtasks MUST use fresh messages[]. Return only summary to parent context."

## Standard Stack

### Core (already in Cargo.toml)
| Crate | Version | Purpose | Why Standard |
|-------|---------|---------|--------------|
| rig-core | 0.31 | LLM abstraction, Tool trait, Agent, PromptHook | Project foundation for all LLM interactions |
| tokio | 1.44 | Async runtime | Only viable async runtime per project constraints |
| serde / serde_json | 1.0 | Serialization for tool args/results | Required by rig Tool trait Args/Output types |
| schemars | 0.8 | JSON Schema generation for tool args | Required by rig Tool trait for tool definitions |
| thiserror | 1.0 | Error types for library code | Project standard for error definitions |

### No New Dependencies Required
All subagent functionality can be built with existing crates. The `SubagentTool` uses the same rig `Tool` trait, same `LlmProvider` enum, and same `agent_loop()` function already in the project.

## Architecture Patterns

### Recommended Project Structure
```
src/
├── subagent/              # NEW: Subagent spawning module
│   ├── mod.rs             # Public exports (SubagentTool, SubagentError, SubagentConfig)
│   └── tool.rs            # SubagentTool implementation (rig Tool trait)
├── agent/                 # EXISTING: Reuse agent_loop, Message, AgentTurn
├── llm/                   # EXISTING: Reuse create_provider, LlmProvider, ProviderType
├── tools/                 # EXISTING: Child tools (bash, read, write, edit, glob, grep, todo)
└── lib.rs                 # UPDATE: Add pub mod subagent; and re-exports
```

### Pattern 1: SubagentTool as rig Tool (Primary Integration Point)
**What:** The subagent appears to the parent agent as just another tool called `task`.
**When to use:** This is the only pattern needed -- rig's tool dispatch handles everything.
**Example:**
```rust
// src/subagent/tool.rs
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use crate::llm::{create_provider, LlmProvider, ProviderType};
use crate::planning::TodoManager;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Arguments for the task (subagent) tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TaskArgs {
    /// The prompt to send to the subagent
    pub prompt: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SubagentError {
    #[error("Subagent execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Provider creation failed: {0}")]
    ProviderError(String),
}

/// Configuration needed to create child agents
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    pub provider_type: ProviderType,
    pub model: String,
    pub base_url: Option<String>,
    pub thinking: bool,
    pub thinking_budget: u64,
    pub todo_manager: Arc<Mutex<TodoManager>>,
    pub max_turns: usize,
}

pub struct SubagentTool {
    config: SubagentConfig,
}

impl Tool for SubagentTool {
    const NAME: &'static str = "task";
    type Error = SubagentError;
    type Args = TaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "task".to_string(),
            description: "Spawn a subagent with fresh context to handle a subtask.".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(TaskArgs))
                .expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Create child provider (without task tool -- no recursion)
        let provider = create_provider(
            self.config.provider_type,
            &self.config.model,
            self.config.base_url.as_deref(),
            self.config.thinking,
            self.config.thinking_budget,
            self.config.todo_manager.clone(),
        ).map_err(|e| SubagentError::ProviderError(e.to_string()))?;

        // Run agent_loop with empty history (fresh context)
        match crate::agent::agent_loop(
            &[],  // fresh messages -- no parent context
            &args.prompt,
            &provider,
            crate::agent::TodoUsageHook::default(),
        ).await {
            Ok(turn) => Ok(turn.response),
            Err(e) => Ok(format!("Subagent error: {}", e)),  // Graceful error, not crash
        }
    }
}
```

### Pattern 2: Provider Construction for Child Agents
**What:** Child agents need their own `LlmProvider` built from config, NOT from the parent's instance.
**Why:** The parent's `LlmProvider` includes the `task` tool. Children must NOT have it (prevents infinite recursion). Each child also needs its own rig `Agent` with a separate tool set.

The key insight from `create_provider()` in `src/llm/provider.rs` is that it registers exactly 7 tools:
```rust
.tool(BashTool)
.tool(ReadTool)
.tool(WriteTool)
.tool(EditTool)
.tool(GlobTool)
.tool(GrepTool)
.tool(todo_tool)
```

For the parent agent, we add an 8th tool: `SubagentTool`. For child agents, we call `create_provider()` as-is (7 tools, no `task`). This naturally prevents recursion.

### Pattern 3: Error Isolation
**What:** Child agent errors must NOT crash the parent.
**When to use:** Always -- this is success criterion #4.
**Example:**
```rust
// In SubagentTool::call():
match agent_loop(&[], &args.prompt, &provider, hook).await {
    Ok(turn) => Ok(turn.response),
    Err(e) => {
        tracing::warn!("Subagent failed: {}", e);
        Ok(format!("Subagent error: {}", e))
        // ^ Returns Ok() with error description, NOT Err()
    }
}
```

### Anti-Patterns to Avoid
- **Shared mutable state between parent and child:** Do NOT share the parent's `messages` Vec or `TodoManager` state with child agents. Each child gets its own TodoManager.
- **Recursive subagent spawning:** Child agents MUST NOT have the `task` tool. Enforce this at the `create_provider` level.
- **Propagating child errors as Err():** The tool's `call()` must return `Ok(String)` even on child failure, so the parent LLM can see the error and decide what to do.
- **Blocking the parent's event loop:** Child agent execution runs via `agent_loop()` which is already async. No special handling needed -- tokio handles concurrency.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| LLM tool dispatch | Custom message routing | rig-core's Agent + ToolSet | rig handles stop_reason branching, tool calling, and multi-turn loops internally |
| Agent loop for children | New loop from scratch | `agent_loop()` from `src/agent/loop_.rs` | Already handles retry, hooks, history conversion |
| Tool schema generation | Manual JSON Schema | schemars derive macros + `schema_for!()` | Consistent with all other tools in the project |
| Error classification in child | New error types | `classify_prompt_error()` + `AgentError` | Already maps rig errors to domain errors |

## Common Pitfalls

### Pitfall 1: Provider Configuration vs Provider Instance
**What goes wrong:** Trying to pass `LlmProvider` into the SubagentTool directly.
**Why it happens:** The parent's provider has the `task` tool registered. If a child reuses it, the child can spawn its own children (infinite recursion).
**How to avoid:** Pass `SubagentConfig` (provider_type, model, base_url, etc.) into `SubagentTool`. Create fresh `LlmProvider` inside `call()` via `create_provider()` which only registers the 7 base tools.
**Warning signs:** Child agents calling `task` tool; stack overflow in logs.

### Pitfall 2: TodoManager Sharing Between Parent and Child
**What goes wrong:** Sharing the parent's `TodoManager` with child agents means child todo operations pollute the parent's task list.
**Why it happens:** `create_provider()` takes `Arc<Mutex<TodoManager>>` and passes it to `TodoTool`.
**How to avoid:** Create a fresh `TodoManager::new()` for each child invocation. The SubagentTool creates its own `Arc<Mutex<TodoManager>>` per `call()`.
**Warning signs:** Parent's todo list shows items added by subagents; duplicate todo items.

### Pitfall 3: Token Usage Tracking Gap
**What goes wrong:** Child agent token usage is invisible to the parent. The parent's `FrontendEvent::TokenUsage` only tracks the parent's direct LLM calls.
**Why it happens:** `agent_loop()` returns usage in `AgentTurn.usage`, but `SubagentTool::call()` only returns the response text. Usage data is discarded.
**How to avoid:** For Phase 4 initial implementation, accept this gap. Log child usage via `tracing::info!`. Track as a future enhancement for the statusline.
**Warning signs:** Token usage in statusline doesn't account for subagent work.

### Pitfall 4: Unbounded Child Execution
**What goes wrong:** A child agent enters an infinite tool-use loop and never returns.
**Why it happens:** `create_provider()` sets `default_max_turns(50)` but that's per provider. If the child's prompt is ambiguous, it could loop extensively.
**How to avoid:** The existing `DEFAULT_MAX_TURNS = 50` in `create_provider()` already caps tool-calling turns. Additionally, consider a configurable `max_turns` in `SubagentConfig`.
**Warning signs:** Subagent calls taking > 60 seconds; 50 tool calls for a simple query.

### Pitfall 5: Result Truncation
**What goes wrong:** Child agent produces very long output that exceeds LLM context when returned as tool_result.
**Why it happens:** The reference implementation truncates tool results to 50,000 characters. Our implementation currently has no truncation.
**How to avoid:** Truncate `turn.response` in `SubagentTool::call()` to a reasonable limit (e.g., 10,000 characters) with a "... (truncated)" suffix.
**Warning signs:** Context limit errors after subagent returns; very large tool_result payloads.

## Code Examples

### SubagentTool Integration with create_provider (Parent-Side)

The parent agent needs a new `create_parent_provider()` function (or modification of `create_provider()`) that adds the SubagentTool:

```rust
// In src/llm/provider.rs -- extend create_provider or add create_parent_provider

pub fn create_parent_provider(
    provider_type: ProviderType,
    model: &str,
    base_url: Option<&str>,
    thinking: bool,
    thinking_budget: u64,
    todo_manager: Arc<Mutex<TodoManager>>,
) -> Result<LlmProvider, ProviderError> {
    let subagent_config = SubagentConfig {
        provider_type,
        model: model.to_string(),
        base_url: base_url.map(|s| s.to_string()),
        thinking,
        thinking_budget,
        todo_manager: todo_manager.clone(),
        max_turns: 30,  // safety limit for children
    };
    let subagent_tool = SubagentTool::new(subagent_config);

    // Build agent with 8 tools (7 base + task)
    let mut agent_builder = AgentBuilder::new(completion_model)
        .preamble(PARENT_SYSTEM_PROMPT)  // mentions the task tool
        .tool(BashTool)
        .tool(ReadTool)
        .tool(WriteTool)
        .tool(EditTool)
        .tool(GlobTool)
        .tool(GrepTool)
        .tool(todo_tool)
        .tool(subagent_tool)  // 8th tool -- only for parent
        .default_max_turns(DEFAULT_MAX_TURNS);
    // ...
}
```

### Child Provider Creation (Inside SubagentTool::call)

```rust
// Inside SubagentTool::call():
let child_todo = Arc::new(Mutex::new(TodoManager::new()));
let provider = create_provider(
    self.config.provider_type,
    &self.config.model,
    self.config.base_url.as_deref(),
    self.config.thinking,
    self.config.thinking_budget,
    child_todo,  // fresh TodoManager for child
)?;
```

### FrontendEvent Integration for Subagent Observability

The existing `FrontendEvent` protocol already has `ToolCallStarted` and `ToolCallFinished`. When the parent agent calls `task`, these events will fire through the existing `TodoUsageHook`. No new FrontendEvent variants are strictly required for Phase 4.

However, consider adding structured subagent events in a future phase:
```rust
// Future enhancement (NOT Phase 4):
FrontendEvent::SubagentStarted { turn_id, call_id, prompt_preview: String }
FrontendEvent::SubagentCompleted { turn_id, call_id, summary_preview: String }
```

For Phase 4, the existing tool call events suffice -- the parent's hook will emit `ToolCallStarted { name: "task", ... }` and `ToolCallFinished { name: "task", result_preview: "..." }`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single agent with growing context | Subagent with fresh context per task | s04 in learn-claude-code | Core pattern for context management |
| Shared state between parent/child | Isolated state per child | s04 design principle | Prevents context pollution |
| Error propagation crashes parent | Error isolation (return as text) | s04 design principle | Resilient multi-task execution |

**Reference implementation learn-claude-code s04 key design:**
- Parent has `task` tool, child has all tools EXCEPT `task` (prevents recursion)
- Child starts with `messages = []` (fresh context)
- Safety limit of 30 tool-use iterations per child
- Tool results truncated to 50,000 characters per block
- Only final text returned to parent, entire child message history discarded

## Open Questions

1. **Should `create_provider` be modified or wrapped?**
   - What we know: Current `create_provider()` registers exactly 7 tools. Parent needs 8 (with `task`).
   - What's unclear: Whether to add a parameter to `create_provider()` for extra tools, or create a separate `create_parent_provider()`.
   - Recommendation: Create a separate `create_parent_provider()` function that builds the parent agent with the subagent tool. This keeps `create_provider()` clean for child agents and avoids adding complexity to the existing function.

2. **Should SubagentTool hold a reference to the parent's LlmProvider or build fresh ones?**
   - What we know: The rig `Tool` trait's `call()` takes `&self`, meaning the tool must own its config.
   - What's unclear: Whether child LLM providers are cheap to construct.
   - Recommendation: Build fresh `LlmProvider` per `call()`. This is what the reference implementation does (new API client per subagent). The cost is negligible compared to LLM API latency.

3. **What system prompt should child agents use?**
   - What we know: Parent uses `SYSTEM_PROMPT` from `src/llm/provider.rs`. Reference uses `SUBAGENT_SYSTEM`.
   - What's unclear: Exact text for child system prompt.
   - Recommendation: Create a `CHILD_SYSTEM_PROMPT` that is similar to the parent's but does NOT mention the `task` tool. Keep it simple: "You are a subagent. Complete the assigned task using available tools."

4. **Should parent and child share the same TodoManager?**
   - What we know: Reference implementation does NOT share todo state.
   - What's unclear: Whether the harness should track subagent todos separately.
   - Recommendation: Fresh `TodoManager::new()` per child. Subagent todos are independent. The parent sees only the final summary text.

## Environment Availability

Step 2.6: SKIPPED (no external dependencies identified beyond what the project already uses -- Rust toolchain, tokio runtime, and LLM API keys which are existing requirements).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework + tokio::test |
| Config file | Cargo.toml (no separate config) |
| Quick run command | `cargo test --lib -- -q` |
| Full suite command | `cargo test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PLAN-02 | SubagentTool implements rig Tool trait with correct NAME/args | unit | `cargo test --lib subagent -- -q` | Wave 0 |
| PLAN-02 | SubagentTool::call() spawns child with fresh messages | unit | `cargo test --lib subagent::test_spawn_fresh_context -- -q` | Wave 0 |
| CROSS-02 | Child agent errors return Ok(error_text) not Err() | unit | `cargo test --lib subagent::test_child_error_isolation -- -q` | Wave 0 |
| CROSS-03 | Child messages[] starts empty, parent messages unaffected | unit | `cargo test --lib subagent::test_context_isolation -- -q` | Wave 0 |
| CROSS-04 | Only final text returned to parent, intermediate history discarded | unit | `cargo test --lib subagent::test_result_summarization -- -q` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib -- -q`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/subagent/tool.rs` -- SubagentTool implementation with unit tests
- [ ] `src/subagent/mod.rs` -- Module exports
- [ ] `src/lib.rs` -- Add `pub mod subagent;`
- [ ] `src/llm/provider.rs` -- Add `create_parent_provider()` function
- [ ] Integration test for parent spawning child via `submit_message_with()` pattern

## Sources

### Primary (HIGH confidence)
- Project source code: `src/agent/loop_.rs`, `src/llm/provider.rs`, `src/tools/bash.rs`, `src/frontend/protocol.rs`, `src/session/runtime.rs`
- rig-core 0.31.0 source: `~/.cargo/registry/src/.../rig-core-0.31.0/src/tool/mod.rs` -- Tool trait definition, ToolSet, ToolDyn
- learn-claude-code s04 documentation: `docs/zh/s04-subagent.md` -- Subagent design pattern, code examples, tool configuration
- CLAUDE.md -- Project constraints, anti-patterns, tech stack

### Secondary (MEDIUM confidence)
- learn-claude-code README -- Architecture philosophy, 12-session structure

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new crates needed; all patterns already established in codebase
- Architecture: HIGH -- Tool trait integration point is clear from rig-core source and existing tool implementations
- Pitfalls: HIGH -- Reference implementation documents these explicitly; codebase already handles most via existing patterns

**Research date:** 2026-03-28
**Valid until:** 2026-04-28 (stable -- no fast-moving dependencies)
