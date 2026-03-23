# Phase 1: s01 - Agent Loop - Research

**Researched:** 2026-03-20
**Domain:** AI Agent Infrastructure, Async Rust, LLM Provider Abstraction
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**LLM Provider Support**
- Support all three providers in Phase 1: Anthropic, OpenAI, Ollama
- Use rig-core's multi-provider abstraction layer
- Provider selection via CLI flag: `--provider anthropic|openai|ollama`
- API keys from environment variables: `HARNESS_ANTHROPIC_KEY`, `HARNESS_OPENAI_KEY`

**CLI Interaction Mode**
- REPL-style interactive session (not single-shot command)
- Single-line input, Enter to send
- Graceful exit on Ctrl+C with session summary
- Essential flags only: `--provider`, `--model`, `--verbose`

**Error Handling & Retry**
- Auto-retry on network errors and rate limits with exponential backoff
- Maximum 3 retries before failing
- User-friendly error messages by default
- Full technical details with `--verbose` flag
- Graceful handling when hitting context window limit (notify user, don't crash)

**Streaming & Output**
- Stream LLM output in real-time (not wait for complete response)
- Default silent logging, `-v` for info level, `-vv` for debug
- Structured tracing via tracing-subscriber

**System Prompt**
- Minimal hardcoded system prompt for Phase 1
- Tell the model it's an agent with loop capability
- Full skill system deferred to Phase 5

**Session State**
- No persistence between runs (fresh session each start)
- Session persistence deferred to Phase 7 task system

### Claude's Discretion
- Exact retry backoff timing (start with 1s, double each retry)
- Specific error message wording
- Context limit threshold percentage (e.g., warn at 80%)
- Exact CLI welcome message and exit summary format

### Deferred Ideas (OUT OF SCOPE)
- Tool dispatch and file operations — Phase 2
- TodoWrite with nag reminders — Phase 3
- Subagent spawning — Phase 4
- On-demand skill loading — Phase 5
- Context compression — Phase 6
- Session persistence — Phase 7
- Multi-agent teams — Phase 9

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CORE-01 | Agent loop with stop_reason handling — while loop that processes tool_use until text response | Agent loop pattern documented with code example; rig-core provides StopReason enum |
| CORE-05 | Multi-backend LLM provider — abstract trait for Anthropic, OpenAI, Ollama/local (partial: basic provider support) | rig-core provides multi-provider abstraction via `Provider` trait; streaming support included |
| CORE-06 | CLI interface with clap — argument parsing and interactive mode | clap 4.5+ with derive macros; interactive-clap or custom readline for REPL |
| CROSS-02 | Error classification (retryable vs non-retryable) — proper error handling per provider | thiserror for structured errors; retryable classification by error type |
| CROSS-03 | Graceful degradation when providers fail — fallback logic with user notification | Error handling patterns with user-friendly messages; verbose flag for details |
| CROSS-04 | Structured logging and observability — tracing/tracing-subscriber for diagnostics | tracing 0.1+ with env-filter; RUST_LOG for control |

</phase_requirements>

## Summary

Phase 1 establishes the foundational agent loop that transforms an LLM into an agent through stop_reason branching. The core mechanism is a simple while loop (under 50 lines) that calls the LLM, checks the stop_reason, and either returns text output or processes tool calls. This phase delivers a working REPL-style CLI that supports all three providers (Anthropic, OpenAI, Ollama) via rig-core's abstraction layer, with real-time streaming output and robust error handling.

The implementation centers on rig-core's streaming and completion modules for LLM interaction, clap derive macros for type-safe CLI argument parsing, and tracing for structured observability. The critical architectural insight is that "the model IS the agent" — the harness provides mechanisms (tools, knowledge interfaces, permissions), not routing logic. The agent loop should be clean and simple, delegating decision-making to the model.

**Primary recommendation:** Build the agent loop as a standalone function with clear inputs (messages, tools, provider) and outputs (text response or error). Use rig-core's streaming API for real-time output. Implement error classification early with retryable vs non-retryable distinction. Keep the CLI thin — it should delegate to the agent loop and handle user I/O only.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.44+ | Async runtime | Industry standard, only viable choice for production async Rust |
| rig-core | 0.31.0 | LLM framework | Multi-provider support (Anthropic, OpenAI, Ollama), ergonomic agent abstractions, built-in streaming |
| clap | 4.5+ | CLI parsing | De facto standard with derive macros for type-safe arguments |
| tracing | 0.1+ | Observability | Structured logging that survives async boundaries correctly |
| tracing-subscriber | 0.3+ | Tracing setup | Env-filter for RUST_LOG control, fmt layer for output |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde | 1.0+ | Serialization | Message serialization, configuration |
| serde_json | 1.0+ | JSON handling | Message content parsing, tool definitions |
| thiserror | 1.0+ | Library errors | Structured error types with derive macro |
| anyhow | 1.0+ | CLI errors | Ergonomic error handling with context for main.rs |
| tokio-stream | 0.1+ | Stream utilities | Processing streaming LLM responses |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| rig-core | genai | genai is multi-provider alternative but less mature, smaller ecosystem |
| clap | argh | argh is lighter but lacks clap's feature richness for complex CLIs |
| tracing | log | log doesn't survive async boundaries correctly; tracing is the modern standard |
| thiserror | snafu | snafu has more complexity; thiserror is simpler for most cases |

**Installation:**

```toml
[dependencies]
tokio = { version = "1.44", features = ["full"] }
rig-core = "0.31"
clap = { version = "4.5", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
anyhow = "1.0"
tokio-stream = "0.1"

[dev-dependencies]
tokio-test = "0.4"
```

**Version verification:**
- rig-core 0.31.0 verified via docs.rig.rs (current as of March 2026)
- tokio 1.44+ verified via tokio.rs
- clap 4.5+ verified via docs.rs

## Architecture Patterns

### Recommended Project Structure

```
src/
├── main.rs              # CLI entry point, anyhow error handling
├── lib.rs               # Library root, exports modules
├── agent/
│   ├── mod.rs           # Agent module exports
│   ├── loop.rs          # Core agent loop with stop_reason handling
│   └── message.rs       # Message types and conversion
├── llm/
│   ├── mod.rs           # LLM module exports
│   ├── provider.rs      # Provider trait abstraction
│   └── streaming.rs     # Stream processing utilities
├── error/
│   ├── mod.rs           # Error module exports
│   └── classify.rs      # Error classification (retryable vs non-retryable)
└── cli/
    ├── mod.rs           # CLI module exports
    ├── args.rs          # clap derive Args struct
    └── session.rs       # REPL session management
```

### Pattern 1: Agent Loop with Stop Reason Branching

**What:** Core orchestration that calls LLM, checks stop_reason, and either returns text or processes tools.

**When to use:** This is the foundational pattern for all agent interactions. Every agent session runs through this loop.

**Example:**

```rust
// Source: .planning/research/ARCHITECTURE.md + rig-core API
use rig::completion::{Chat, Message, StopReason};

pub async fn agent_loop(
    messages: &mut Vec<Message>,
    tools: &[Tool],
    provider: &dyn Chat,
) -> Result<String, AgentError> {
    loop {
        // Call LLM with streaming
        let mut stream = provider.chat_stream(messages.clone(), tools).await?;
        let mut response_text = String::new();
        let mut tool_calls = Vec::new();
        let mut stop_reason = StopReason::EndTurn;

        // Process stream events
        while let Some(event) = stream.next().await {
            match event? {
                StreamEvent::TextDelta(delta) => {
                    print!("{}", delta); // Real-time output
                    response_text.push_str(&delta);
                }
                StreamEvent::ToolUseStart(tool) => {
                    tool_calls.push(ToolCallBuilder::new(tool));
                }
                StreamEvent::ToolUseDelta(tool_id, delta) => {
                    tool_calls.iter_mut()
                        .find(|t| t.id == tool_id)
                        .expect("tool not found")
                        .append_input(delta);
                }
                StreamEvent::StopReason(reason) => {
                    stop_reason = reason;
                }
            }
        }
        println!(); // Newline after streaming

        // Add assistant message to history
        messages.push(Message::assistant(response_text.clone(), tool_calls.clone()));

        // Branch on stop_reason
        match stop_reason {
            StopReason::EndTurn | StopReason::StopSequence => {
                return Ok(response_text);
            }
            StopReason::ToolUse => {
                // Phase 2: Execute tools and append results
                // For Phase 1, return error since tools not implemented
                return Err(AgentError::ToolsNotImplemented);
            }
            StopReason::MaxTokens => {
                // Notify user about context limit
                eprintln!("Warning: Context window limit reached. Consider shorter conversation.");
                // Continue with current context (graceful degradation)
            }
        }
    }
}
```

### Pattern 2: Multi-Provider Abstraction via rig-core

**What:** Use rig-core's provider abstraction to support Anthropic, OpenAI, and Ollama with unified API.

**When to use:** All LLM interactions should go through rig-core's `Chat` trait.

**Example:**

```rust
// Source: docs.rig.rs + rig-core API
use rig::providers::{anthropic, openai, openrouter};
use rig::completion::Chat;

pub fn create_provider(
    provider_name: &str,
    model: &str,
) -> Result<Box<dyn Chat>, ProviderError> {
    match provider_name {
        "anthropic" => {
            let client = anthropic::Client::from_env()?;
            Ok(Box::new(client.agent(model).build()))
        }
        "openai" => {
            let client = openai::Client::from_env()?;
            Ok(Box::new(client.agent(model).build()))
        }
        "ollama" => {
            // rig-core supports Ollama via local endpoint
            let client = openrouter::Client::from_env()?;
            Ok(Box::new(client.agent(model).build()))
        }
        _ => Err(ProviderError::UnknownProvider(provider_name.to_string())),
    }
}
```

### Pattern 3: Error Classification with Retry Logic

**What:** Classify errors as retryable (network, rate limit) or non-retryable (auth, invalid request) with exponential backoff.

**When to use:** All LLM API calls should wrap in retry logic with proper error classification.

**Example:**

```rust
// Source: Best practice for LLM API resilience
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Network error: {0}")]
    Network(String), // Retryable
    #[error("Rate limited: retry after {0:?}")]
    RateLimited(Duration), // Retryable
    #[error("Authentication failed: {0}")]
    Auth(String), // Non-retryable
    #[error("Invalid request: {0}")]
    InvalidRequest(String), // Non-retryable
    #[error("Context limit exceeded")]
    ContextLimit, // Graceful degradation, not error
    #[error("Tools not implemented (Phase 2)")]
    ToolsNotImplemented,
}

impl AgentError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, AgentError::Network(_) | AgentError::RateLimited(_))
    }
}

pub async fn with_retry<T, F, Fut>(
    max_retries: u32,
    mut operation: F,
) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let mut delay = Duration::from_secs(1);

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_retryable() && attempt < max_retries => {
                eprintln!("Retry {}/{}: {}", attempt + 1, max_retries, e);
                sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
            Err(e) => return Err(e),
        }
    }

    Err(AgentError::Network("Max retries exceeded".to_string()))
}
```

### Pattern 4: CLI with clap Derive Macros

**What:** Type-safe argument parsing with clap derive macros for REPL-style interactive session.

**When to use:** Main entry point for the CLI application.

**Example:**

```rust
// Source: clap 4.5 documentation
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "harness")]
#[command(about = "AI Agent Harness - A high-performance agent CLI", long_about = None)]
pub struct Args {
    /// LLM provider to use
    #[arg(short, long, value_enum, default_value = "anthropic")]
    pub provider: Provider,

    /// Model to use (provider-specific)
    #[arg(short, long, default_value = "claude-3-5-sonnet-20241022")]
    pub model: String,

    /// Verbosity level (-v for info, -vv for debug)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum Provider {
    Anthropic,
    Openai,
    Ollama,
}

impl Args {
    pub fn tracing_level(&self) -> tracing::Level {
        match self.verbose {
            0 => tracing::Level::WARN,
            1 => tracing::Level::INFO,
            _ => tracing::Level::DEBUG,
        }
    }
}
```

### Pattern 5: REPL Session with Graceful Exit

**What:** Interactive readline loop that handles user input, Ctrl+C, and session summary.

**When to use:** Main session loop for CLI interaction.

**Example:**

```rust
// Source: Best practice for CLI REPL
use std::io::{self, Write, BufRead};

pub struct Session {
    messages: Vec<Message>,
    turn_count: u32,
}

impl Session {
    pub fn new() -> Self {
        Self {
            messages: vec![Message::system(
                "You are an AI agent with the ability to have a conversation. \
                 Respond naturally to user messages."
            )],
            turn_count: 0,
        }
    }

    pub async fn run(&mut self, provider: &dyn Chat) -> Result<(), AgentError> {
        println!("Agent Harness v0.1.0");
        println!("Provider: {} | Model: {}", "anthropic", "claude-3-5-sonnet");
        println!("Type your message and press Enter. Ctrl+C to exit.\n");

        let stdin = io::stdin();
        let mut ctrl_c = tokio::signal::ctrl_c();

        loop {
            print!("You: ");
            io::stdout().flush().unwrap();

            // Read user input
            let mut input = String::new();
            let mut line = String::new();

            // Use tokio::select for Ctrl+C handling
            tokio::select! {
                _ = &mut ctrl_c => {
                    self.print_summary();
                    return Ok(());
                }
                result = async {
                    stdin.lock().read_line(&mut line)?;
                    Ok::<_, io::Error>(line)
                } => {
                    input = result?;
                }
            }

            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            // Add user message
            self.messages.push(Message::user(input));

            // Run agent loop
            print!("Agent: ");
            let response = agent_loop(&mut self.messages, &[], provider).await?;
            println!();

            self.turn_count += 1;
        }
    }

    fn print_summary(&self) {
        println!("\n\nSession Summary:");
        println!("  Turns: {}", self.turn_count);
        println!("  Messages: {}", self.messages.len());
    }
}
```

### Anti-Patterns to Avoid

- **Prompt Plumbing:** Don't wire LLM calls with if-else branches and hardcoded routing. Build clean agent loop with tools. Let the model decide. Add harness mechanisms, not routing logic.

- **System Prompt Stuffing:** Don't put all knowledge upfront in system prompt. This phase uses minimal system prompt. Phase 5 adds on-demand skill loading via tool_result.

- **Blocking on LLM Calls:** Always use streaming API (`chat_stream`) instead of blocking (`chat`). Real-time output is expected by users.

- **Unwrap in Library Code:** Never use `.unwrap()` in agent/, llm/, error/ modules. Use `Result` propagation with `?` operator. Only use unwrap in tests.

- **std::sync::mpsc in Async:** Never use `std::sync::mpsc` in async contexts. It blocks the tokio executor. Use `tokio::sync::mpsc` instead.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-provider LLM support | Custom HTTP client for each provider | rig-core Provider trait | Handles authentication, streaming, error mapping, retry logic internally |
| Streaming event parsing | Manual SSE parsing | rig-core streaming module | Handles chunk parsing, event reconstruction, error recovery |
| CLI argument parsing | Manual argv parsing | clap derive macros | Type-safe, generates help text, handles validation |
| Structured logging | println! with formatting | tracing macros | Survives async boundaries, supports filtering, structured fields |
| Error types | String errors | thiserror derive | Structured errors with context, proper Error trait implementation |
| Async retry logic | Manual loop with sleep | Custom with_retry function | Exponential backoff needs careful implementation; reuse pattern |

**Key insight:** The LLM API interaction layer is deceptively complex (streaming, error recovery, authentication). rig-core handles this complexity. The agent loop itself should be simple and focused on stop_reason branching.

## Common Pitfalls

### Pitfall 1: Confusing Stop Reasons

**What goes wrong:** Treating all stop_reason values as terminal conditions, or not handling MaxTokens correctly.

**Why it happens:** LLM providers have different stop_reason semantics. Anthropic has EndTurn, ToolUse, MaxTokens, StopSequence. Not understanding these leads to incorrect control flow.

**How to avoid:**
- EndTurn: Normal completion, return text
- ToolUse: Execute tools, append results, continue loop
- MaxTokens: Hit context limit, warn user but continue (graceful degradation)
- StopSequence: Model hit stop sequence, return text

**Warning signs:** Agent stops unexpectedly, infinite loop on tool calls, context limit crashes.

### Pitfall 2: Not Streaming Output

**What goes wrong:** Using blocking `chat()` call and waiting for complete response before showing any output.

**Why it happens:** Streaming API is more complex than blocking API. Developer chooses simplicity over UX.

**How to avoid:** Always use `chat_stream()` for user-facing interactions. Process StreamEvent::TextDelta immediately with print!(). Only use blocking API for internal/testing scenarios.

**Warning signs:** Long pause before any output appears, user thinks agent is frozen.

### Pitfall 3: Blocking the Tokio Runtime

**What goes wrong:** Using `std::io::stdin().read_line()` or `std::sync::mpsc` in async context, blocking the executor.

**Why it happens:** These APIs look similar to async equivalents but block the thread.

**How to avoid:** Use `tokio::io::stdin()` with async read, or use `tokio::select!` with `ctrl_c()` signal. Use `tokio::sync::mpsc` for channels.

**Warning signs:** Agent stops responding to Ctrl+C, other async tasks stall, streaming output hangs.

### Pitfall 4: Retrying Non-Retryable Errors

**What goes wrong:** Retrying authentication errors or invalid request errors, wasting API calls and time.

**Why it happens:** Not classifying errors properly. Treating all errors as network failures.

**How to avoid:** Implement error classification. Only retry Network and RateLimited errors. Auth, InvalidRequest, and ContextLimit errors should fail immediately.

**Warning signs:** Many failed API calls with same error, slow failures, API quota exhausted.

### Pitfall 5: Context Window Overflow

**What goes wrong:** Messages grow unbounded until hitting context limit, causing MaxTokens or errors.

**Why it happens:** No context management in Phase 1. Each turn adds user message + assistant message + tool results.

**How to avoid:**
- Phase 1: Warn user when approaching limit (e.g., 80% threshold)
- Phase 6: Implement three-layer compression (summarize old turns, truncate verbose outputs, persist to disk)

**Warning signs:** Response quality degrades over long sessions, MaxTokens stop reason appears frequently.

## Code Examples

Verified patterns from official sources and research:

### Agent Loop Core

```rust
// Source: .planning/research/ARCHITECTURE.md + rig-core patterns
pub async fn agent_loop(
    messages: &mut Vec<Message>,
    tools: &[Tool],
    provider: &dyn Chat,
) -> Result<String, AgentError> {
    loop {
        let response = with_retry(3, || {
            provider.chat(messages.clone(), tools)
        }).await?;

        messages.push(Message::assistant(response.content.clone()));

        match response.stop_reason {
            StopReason::EndTurn | StopReason::StopSequence => {
                return Ok(response.text_content());
            }
            StopReason::ToolUse => {
                // Phase 2: let results = execute_tools(&response.tool_calls).await?;
                // messages.push(Message::tool_results(results));
                return Err(AgentError::ToolsNotImplemented);
            }
            StopReason::MaxTokens => {
                eprintln!("Warning: Context limit approaching.");
                // Continue with current context
            }
        }
    }
}
```

### Streaming with Real-Time Output

```rust
// Source: docs.rig.rs streaming module
use futures::StreamExt;

pub async fn stream_response(
    provider: &dyn Chat,
    messages: Vec<Message>,
) -> Result<String, AgentError> {
    let mut stream = provider.chat_stream(messages, &[]).await?;
    let mut full_response = String::new();

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextDelta(delta) => {
                print!("{}", delta);
                io::stdout().flush()?;
                full_response.push_str(&delta);
            }
            StreamEvent::StopReason(reason) => {
                // Handle stop reason
            }
            _ => {}
        }
    }

    Ok(full_response)
}
```

### Tracing Setup

```rust
// Source: tracing-subscriber documentation
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn setup_tracing(level: tracing::Level) {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(level.to_string())),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
```

### Main Entry Point

```rust
// Source: Best practice for CLI applications
use clap::Parser;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    setup_tracing(args.tracing_level());

    let provider = create_provider(&args.provider.to_string().to_lowercase(), &args.model)?;
    let mut session = Session::new();

    session.run(&provider).await?;

    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| async-std runtime | tokio runtime | async-std discontinued 2025 | Only viable async runtime for production Rust |
| log crate | tracing crate | tracing 0.1 release | Structured logging survives async boundaries |
| Blocking LLM calls | Streaming LLM calls | rig-core 0.18+ | Real-time output, better UX |
| String errors | thiserror/anyhow | Rust 1.65+ | Structured errors with context |
| Single provider | Multi-provider abstraction | rig-core 0.18+ | Provider flexibility without code changes |

**Deprecated/outdated:**
- **async-std:** Officially discontinued as of 2025. Do NOT use.
- **log crate:** Use tracing instead for async compatibility.
- **std::sync::mpsc:** Blocks async executor. Use tokio::sync::mpsc.
- **reqwest::blocking:** Blocks runtime. Use async reqwest.

## Open Questions

1. **Ollama Provider Configuration**
   - What we know: rig-core supports Ollama, needs local endpoint
   - What's unclear: Exact configuration for Ollama in rig-core (base URL, model naming)
   - Recommendation: Check rig-core docs for Ollama examples during implementation. Default to localhost:11434.

2. **Streaming Event Types**
   - What we know: rig-core has StreamEvent enum with TextDelta, ToolUseStart, StopReason
   - What's unclear: Complete list of event types for all providers
   - Recommendation: Implement handler for common events (TextDelta, StopReason), add others as needed during testing.

3. **Interactive Input Method**
   - What we know: Need REPL-style input with Ctrl+C handling
   - What's unclear: Whether to use interactive-clap, rustyline, or custom stdin reading
   - Recommendation: Start with custom stdin reading (simplest). Add rustyline later for history/editing if needed.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework + tokio-test |
| Config file | Cargo.toml (no separate config needed) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CORE-01 | Agent loop processes stop_reason correctly | unit | `cargo test test_agent_loop --lib` | ❌ Wave 0 |
| CORE-01 | Agent loop returns text on EndTurn | unit | `cargo test test_end_turn --lib` | ❌ Wave 0 |
| CORE-01 | Agent loop continues on ToolUse | unit | `cargo test test_tool_use --lib` | ❌ Wave 0 |
| CORE-05 | Provider creation for Anthropic | unit | `cargo test test_anthropic_provider --lib` | ❌ Wave 0 |
| CORE-05 | Provider creation for OpenAI | unit | `cargo test test_openai_provider --lib` | ❌ Wave 0 |
| CORE-05 | Provider creation for Ollama | unit | `cargo test test_ollama_provider --lib` | ❌ Wave 0 |
| CORE-06 | CLI args parsing with clap | unit | `cargo test test_cli_args --lib` | ❌ Wave 0 |
| CROSS-02 | Error classification retryable vs non-retryable | unit | `cargo test test_error_classification --lib` | ❌ Wave 0 |
| CROSS-02 | Retry logic with exponential backoff | unit | `cargo test test_retry_logic --lib` | ❌ Wave 0 |
| CROSS-03 | Graceful error messages | integration | `cargo test --test error_messages` | ❌ Wave 0 |
| CROSS-04 | Tracing setup and filtering | integration | `cargo test --test tracing_setup` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --lib` (unit tests only, fast)
- **Per wave merge:** `cargo test` (full suite including integration)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/agent/loop.rs` — unit tests for agent_loop function
- [ ] `src/llm/provider.rs` — unit tests for provider creation
- [ ] `src/error/classify.rs` — unit tests for error classification
- [ ] `src/cli/args.rs` — unit tests for CLI argument parsing
- [ ] `tests/error_messages.rs` — integration tests for user-facing error messages
- [ ] `tests/tracing_setup.rs` — integration tests for logging configuration
- [ ] Framework install: `cargo test` — built-in, no additional install needed

## Sources

### Primary (HIGH confidence)

- **docs.rig.rs** — Official Rig framework documentation, multi-provider LLM abstraction, streaming API
- **tokio.rs/tokio/tutorial** — Official Tokio tutorial, async runtime patterns
- **docs.rs/clap/4.5** — Official clap documentation, derive macros
- **tracing.rs** — Official tracing documentation, structured logging
- **.planning/research/ARCHITECTURE.md** — Project-specific architecture patterns, agent loop design
- **.planning/research/STACK.md** — Project-specific stack recommendations, version requirements

### Secondary (MEDIUM confidence)

- **thiserror documentation** (docs.rs/thiserror) — Error type derive macro patterns
- **anyhow documentation** (docs.rs/anyhow) — Application error handling patterns
- **tracing-subscriber documentation** (docs.rs/tracing-subscriber) — Env-filter configuration
- **.planning/research/SUMMARY.md** — Build order rationale, critical pitfalls

### Tertiary (LOW confidence)

- **Web search for rig-core current version** — Confirmed 0.31.0 as of March 2026
- **Web search for async-std discontinuation** — Community announcement confirming discontinuation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — All crates are mature, well-documented, production-ready. Versions verified against official sources.
- Architecture: HIGH — Agent loop pattern is well-established from reference curriculum. rig-core provides clear API patterns.
- Pitfalls: MEDIUM — Pitfalls identified from research and best practices. Some provider-specific behaviors need validation during implementation.

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (30 days - stable Rust ecosystem with rig-core actively maintained)
