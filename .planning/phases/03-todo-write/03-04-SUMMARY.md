---
phase: 03-todo-write
plan: 04
subsystem: llm
tags: [todo, prompt-hook, provider, tool-registration, direct-detection]
requires:
  - phase: 03-01
    provides: TodoManager core state
  - phase: 03-02
    provides: TodoTool implementation
  - phase: 03-03
    provides: TodoUsageHook struct, Session todo fields
provides:
  - TodoTool registration in provider chain
  - PromptHook trait implementation for TodoUsageHook
  - chat_with_history_and_hook method on LlmProvider
  - agent_loop with hook parameter support
affects: [src/llm/provider.rs, src/agent/loop_.rs, src/cli/session.rs]
tech_stack:
  added: []
  patterns: [PromptHook trait, direct tool usage detection, Arc<Mutex> shared state]
key_files:
  created: []
  modified:
    - src/llm/provider.rs
    - src/agent/loop_.rs
    - src/cli/session.rs
key_decisions:
  - Use PromptHook::on_tool_call() for direct tool usage detection (not state tracking)
  - NO updated/generation fields in TodoManager (per review feedback)
  - NO was_updated()/clear_updated() methods (per review feedback)
  - TodoTool registered in both Anthropic and OpenAI provider branches
metrics:
  duration: 7 min
  tasks_completed: 5
  files_created: 0
  files_modified: 3
  tests_added: 0
  completed_date: 2026-03-23
---

# Phase 03 Plan 04: Provider Wiring Summary

## One-liner

Wired TodoTool into provider creation chain and implemented PromptHook trait for direct tool usage detection, matching Python's dispatch-based approach.

## What Was Done

### Task 1: Update provider.rs to accept and register TodoTool

Updated `src/llm/provider.rs`:
- Added `todo_manager: Arc<Mutex<TodoManager>>` parameter to `create_provider` function
- Created `TodoTool` instance in both Anthropic and OpenAI branches
- Registered TodoTool via `.tool(todo_tool)` in AgentBuilder chain
- Updated SYSTEM_PROMPT to include todo tool in available tools list

### Task 2: Implement PromptHook for TodoUsageHook

Added `#[derive(Clone)]` to TodoUsageHook and implemented `PromptHook<M>` trait:
- `on_tool_call()` method detects when "todo" tool is called
- Sets `used_todo` AtomicBool flag to true on detection
- Returns `ToolCallHookAction::cont()` to continue execution

### Task 3: Update agent_loop to use PromptRequest with hook

Updated `agent_loop` function signature to accept generic hook parameter:
- Generic bound: `H: PromptHook<AnthropicModel> + PromptHook<OpenAIModel> + Clone + Send + Sync + 'static`
- Uses `chat_with_history_and_hook()` method for hook-enabled chat

### Task 4: Add chat_with_history_and_hook to LlmProvider

Added new method to LlmProvider:
- `chat_with_history_and_hook()` accepts prompt, history, and hook
- Uses `PromptRequest::from_agent()` + `.with_hook()` for hook-enabled execution
- Implemented for both Anthropic and OpenAI variants

### Task 5: Update session.rs to pass todo_manager and use hook

Updated Session::run():
- Passes `self.todo_manager.clone()` to `create_provider()`
- Creates `TodoUsageHook` via `TodoUsageHook::new()`
- Checks `used_todo_flag.load(Ordering::SeqCst)` after agent_loop
- Resets `rounds_since_todo = 0` when todo was used
- Injects nag reminder into response when `rounds_since_todo >= 3`

## Deviations from Plan

None - plan executed exactly as written, following review feedback for PromptHook approach.

## Verification Results

- `cargo check` completes without errors
- All acceptance criteria met:
  - TodoTool registered in both provider branches
  - SYSTEM_PROMPT includes todo tool
  - TodoUsageHook implements PromptHook trait
  - agent_loop accepts hook parameter
  - Session passes todo_manager to create_provider

## Key Files

| File | Purpose | Changes |
|------|---------|---------|
| src/llm/provider.rs | Provider creation with TodoTool | +todo_manager param, TodoTool registration |
| src/agent/loop_.rs | Hook-enabled agent_loop | PromptHook impl, generic hook parameter |
| src/cli/session.rs | Session integration | Hook creation, flag checking, nag reminder |

## Next Steps

Phase 03 complete. All TodoWrite functionality implemented:
- TodoManager core state management (03-01)
- TodoTool with shared state (03-02)
- Session integration with nag reminder (03-03)
- Provider wiring with PromptHook (03-04)

Ready for phase verification.
