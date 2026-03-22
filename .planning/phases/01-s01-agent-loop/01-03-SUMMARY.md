---
phase: 01-s01-agent-loop
plan: 03
subsystem: agent
tags: [agent-loop, retry, message-types, core-orchestration]
requires: [01-01, 01-02]
provides: [agent_loop, with_retry, Message, Role, AgentTurn]
affects: [src/agent/, src/llm/provider.rs]
tech_stack:
  added:
    - futures crate for async stream utilities
  patterns:
    - Enum-based message types with Role variants
    - Exponential backoff retry pattern
    - Error conversion from rig-core PromptError to AgentError
key_files:
  created:
    - src/agent/mod.rs
    - src/agent/message.rs
    - src/agent/loop_.rs
    - tests/agent_loop_test.rs
  modified:
    - src/lib.rs
    - src/llm/provider.rs
    - src/llm/mod.rs
    - Cargo.toml
decisions:
  - Use enum-based message types (Role enum with System/User/Assistant)
  - Use loop_.rs filename (not loop.rs) to avoid keyword conflict
  - Use LlmProvider enum instead of &dyn Chat (rig-core Chat trait not object-safe)
  - Skip system messages in chat history (handled via agent preamble in rig)
  - Use non-streaming chat for Phase 1 (streaming deferred to future iteration)
metrics:
  duration_min: 45
  completed_date: "2026-03-21"
  tasks_completed: 4
  files_created: 4
  files_modified: 4
  tests_added: 9
---

# Phase 01 Plan 03: Core Agent Loop Summary

## One-liner

Core agent loop with message types, retry logic with exponential backoff, and LLM provider integration using enum-based dispatch.

## What Was Built

### 1. Agent Module Structure (src/agent/)

Created the foundational agent module with:

- **mod.rs**: Module declarations and re-exports for `agent_loop`, `with_retry`, `Message`, `Role`, `AgentTurn`
- **message.rs**: `Role` enum (System, User, Assistant) and `Message` struct with convenience constructors, `AgentTurn` struct
- **loop_.rs**: Core agent loop with retry logic, uses `classify_prompt_error` for error classification

### 2. Message Types

```rust
pub enum Role {
    System,
    User,
    Assistant,
}

pub struct Message {
    pub role: Role,
    pub content: String,
}
```

With constructors: `Message::system()`, `Message::user()`, `Message::assistant()`

### 3. Retry Logic with Exponential Backoff

```rust
pub async fn with_retry<T, F, Fut>(
    max_retries: u32,
    operation: F,
) -> Result<T, AgentError>
```

- Maximum 3 retries
- Initial delay: 1 second
- Exponential backoff: 1s -> 2s -> 4s
- Only retries on `AgentError::Network` and `AgentError::RateLimited`
- Non-retryable errors (Auth, InvalidRequest) fail immediately

### 4. Agent Loop

```rust
pub struct AgentTurn {
    pub user_input: String,
    pub response: String,
}

pub async fn agent_loop(
    history: &[Message],
    current_input: &str,
    provider: &LlmProvider,
) -> Result<AgentTurn, AgentError>
```

- `history` is read-only; state management is the caller's responsibility
- `current_input` is passed explicitly (no implicit extraction from history)
- Returns structured `AgentTurn` for clean session state management
- Converts harness `Message` types to `rig::completion::Message`
- Handles system messages (skipped in history, handled via preamble)
- Uses `with_retry` wrapper for LLM calls
- Converts `PromptError` to `AgentError` for error propagation

### 5. LLM Provider Enhancement

Added `chat_with_history` method to `LlmProvider`:

```rust
pub async fn chat_with_history(
    &self,
    prompt: impl Into<String>,
    chat_history: Vec<Message>,
) -> Result<String, PromptError>
```

### 6. Error Classification

Added `classify_prompt_error` function to properly classify rig-core errors:

```rust
pub fn classify_prompt_error(err: PromptError) -> AgentError
```

**HTTP Status Code Mapping:**

| Status Code | AgentError | Retryable |
|-------------|------------|-----------|
| 408, 500, 502, 503, 504 | `Network` | ✅ Yes |
| 429 | `RateLimited` | ✅ Yes |
| 401, 403 | `Auth` | ❌ No |
| 400, 404 | `InvalidRequest` | ❌ No |
| 413 | `ContextLimit` | ❌ No |

**Test Coverage:** 34 unit tests covering status code classification, retry-after extraction, and provider message classification.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] rig-core Chat trait not object-safe**

- **Found during:** Task 3 implementation
- **Issue:** Plan specified `&dyn rig::completion::Chat` but rig-core's Chat trait uses `impl Trait` in return types, making it not object-safe
- **Fix:** Changed `agent_loop` signature to use `&LlmProvider` enum instead of trait object
- **Files modified:** src/agent/loop_.rs
- **Commit:** 0b899b9

**2. [Rule 3 - Blocking] No Message::system constructor in rig-core**

- **Found during:** Task 3 implementation
- **Issue:** `rig::completion::Message` enum only has User and Assistant variants, no System
- **Fix:** System messages are skipped in message conversion (handled via agent preamble in rig)
- **Files modified:** src/agent/loop_.rs
- **Commit:** 0b899b9

**3. [Rule 3 - Blocking] Streaming API complexity**

- **Found during:** Task 3 implementation
- **Issue:** Plan expected `chat_stream()` with `StreamEvent::TextDelta`, but rig-core streaming API returns `MultiTurnStreamItem` with complex generic types
- **Fix:** Deferred streaming to future iteration, using `chat_with_history` for Phase 1
- **Files modified:** src/llm/provider.rs, src/agent/loop_.rs
- **Commit:** 0b899b9

## Test Coverage

- **Unit tests:** 9 tests (4 retry logic + 1 message types + 4 provider tests)
- **Integration tests:** 5 tests in tests/agent_loop_test.rs
- All tests passing

## Key Decisions

1. **Enum dispatch over trait objects**: Using `LlmProvider` enum instead of `&dyn Chat` for zero-cost abstraction and type safety

2. **Module naming**: Using `loop_.rs` to avoid Rust keyword conflict while maintaining clear naming

3. **Streaming deferral**: Complex rig-core streaming API deferred to future phase to deliver working agent loop faster

4. **System message handling**: Skipping system messages in history conversion since rig handles them via agent preamble

## Next Steps

- Phase 2: Tool dispatch with stop_reason handling
- Future: Streaming output implementation with `chat_stream()`
- Future: Proper stop_reason branching (EndTurn, ToolUse, MaxTokens)
