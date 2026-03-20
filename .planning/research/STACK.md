# Stack Research

**Domain:** AI Agent Harness (High-Performance Rust)
**Researched:** 2026-03-20
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **tokio** | 1.44+ | Async runtime | Industry standard, best ecosystem, proven at scale. Only viable choice for production async Rust in 2025. |
| **rig-core** | 0.18+ | LLM framework | Most mature Rust LLM framework with multi-provider support (Anthropic, OpenAI, Ollama, 15+ providers). Ergonomic agent abstractions, built-in streaming, vector store integrations. |
| **rmcp** | 1.2+ | MCP protocol | Official Rust SDK for Model Context Protocol from modelcontextprotocol/rust-sdk. Only authoritative MCP implementation in Rust. |
| **clap** | 4.5+ | CLI argument parsing | De facto standard for Rust CLIs. Derive macros for type-safe argument definitions, excellent error messages, subcommand support. |
| **serde** | 1.0+ | Serialization framework | Foundation for all Rust serialization. Required by nearly every crate in the ecosystem. |
| **serde_json** | 1.0+ | JSON handling | JSON Lines (JSONL) is the standard format for agent communication, logging, and multi-agent mailboxes. Essential for this project. |

### Async I/O & Networking

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **tokio::process** | (in tokio) | Async command execution | Spawning and managing subprocesses (tools like bash, git, etc.) |
| **tokio::sync::mpsc** | (in tokio) | Multi-producer single-consumer channels | Task coordination, message passing between agents |
| **tokio::sync::broadcast** | (in tokio) | Multi-producer multi-consumer channels | Broadcasting events to multiple subscribers |
| **tokio::sync::oneshot** | (in tokio) | Single-use channels | One-time responses, futures completion |
| **tokio-stream** | 0.1+ | Stream utilities | Processing async sequences of values, SSE streaming |
| **reqwest** | 0.12+ | HTTP client | De facto standard for async HTTP in Rust. Built on tokio, supports streaming responses. Required for LLM API calls not covered by rig. |
| **async-stream** | 0.3+ | Stream macros | Creating async streams with `stream!` and `try_stream!` macros |

### Error Handling

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **thiserror** | 2.0+ | Custom error types | Library code, domain-specific errors that callers need to match on |
| **anyhow** | 1.0+ | Application errors | Binary application code, error propagation with context, CLI tools |

**Pattern:** Use `thiserror` for the harness library crate (structured errors for consumers), `anyhow` for the CLI binary (ergonomic error handling with context).

### Observability

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **tracing** | 0.1+ | Structured logging & tracing | Required for async applications. Replaces `log` crate. Spans survive async boundaries correctly. |
| **tracing-subscriber** | 0.3+ | Tracing subscriber | Configuring log output, filters, formatters |
| **tracing-appender** | 0.2+ | Log file rotation | Writing traces to files with rotation |

### Utilities

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **uuid** | 1.11+ | UUID generation | Agent IDs, session IDs, trace IDs. Use v4 for random, v7 for time-ordered. |
| **chrono** | 0.4+ | Date/time handling | Timestamps, duration calculations. Most widely used time library in Rust. |
| **regex** | 1.11+ | Regular expressions | Pattern matching in tool outputs, parsing. Linear-time matching guarantee. |
| **directories** | 5.0+ | Platform-specific paths | Config directories, data directories, cache directories. Cross-platform (Linux, macOS, Windows). |
| **once_cell** | 1.20+ | Lazy static initialization | Once-off initialization, global configuration |
| **parking_lot** | 0.12+ | High-performance synchronization | Alternative to `std::sync` primitives. Smaller memory footprint (1 byte mutex), no lock poisoning. Use for high-contention scenarios. |

### Serialization & Schema

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **schemars** | 0.8+ | JSON Schema generation | Required by rmcp for tool schema generation. Derives `JsonSchema` trait. |
| **toml** | 0.8+ | TOML parsing | Configuration files (Cargo.toml style) |

### Process Isolation (Sandboxing)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **tokio::process::Command** | (in tokio) | Process spawning | Basic async subprocess management |
| **nix** | 0.29+ | Unix system calls | seccomp-bpf filters, chroot, namespace isolation (Linux only) |

**Note:** For production-grade sandboxing, consider:
- **Linux seccomp** via `nix` crate for syscall filtering
- **User namespace isolation** for filesystem sandboxing
- The `microsandbox` ecosystem (hardware-level isolation)

## Installation

```toml
# Cargo.toml

[dependencies]
# Core async runtime
tokio = { version = "1.44", features = ["full"] }

# LLM framework
rig-core = "0.18"

# MCP protocol
rmcp = { version = "1.2", features = ["server"] }

# CLI
clap = { version = "4.5", features = ["derive", "env"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Async utilities
tokio-stream = "0.1"
async-stream = "0.3"
reqwest = { version = "0.12", features = ["json", "stream"] }

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utilities
uuid = { version = "1.11", features = ["v4", "v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
regex = "1.11"
directories = "5.0"
once_cell = "1.20"
parking_lot = "0.12"

# Schema generation (for MCP tools)
schemars = "0.8"

# Configuration
toml = "0.8"

# Sandboxing (Linux)
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "0.29", features = ["process", "signal"] }

[dev-dependencies]
tokio-test = "0.4"
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| **tokio** | async-std | **Don't.** async-std is officially discontinued as of 2025. Use `smol` only for library development. |
| **tokio** | smol | Library development where you want runtime-agnostic code. Not recommended for applications. |
| **rig-core** | async-openai | Only if you need OpenAI-specific features not in rig. rig wraps async-openai internally. |
| **rig-core** | genai | If you want a lighter-weight multi-provider client without agent abstractions. |
| **rig-core** | kalosm | Local-first AI with quantized models (Llama, Mistral). Use if running local models primarily. |
| **rig-core** | AutoAgents | Multi-agent orchestration with Ractor. Consider if building complex multi-agent teams. |
| **thiserror** | eyre | If you prefer `eyre::Report` error chain format over `anyhow::Error`. |
| **thiserror** | miette | If you want fancy diagnostic reports with source code snippets. |
| **chrono** | time | If you want a more modern API with compile-time format checking. Smaller but less ecosystem support. |
| **parking_lot** | std::sync | Default choice. Use std unless you have specific performance requirements. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **async-std** | Officially discontinued in 2025. No longer maintained. | tokio (applications) or smol (libraries) |
| **log crate** | Doesn't handle async contexts correctly. Spans don't survive `.await`. | tracing |
| **std::sync::mpsc** | Blocking channels don't work in async contexts. | tokio::sync::mpsc |
| **crossbeam-channel** | Synchronous, designed for multi-threaded blocking code. Use in async will block the executor. | tokio::sync channels for async, crossbeam only for blocking threads |
| **reqwest::blocking** | Blocks the async runtime. | reqwest async API |
| **unwrap() in library code** | Panics are not recoverable. Library consumers can't handle them. | Proper error types with thiserror |
| **panic! for error conditions** | Same reason. Use Result propagation. | anyhow for apps, thiserror for libraries |

## Stack Patterns by Variant

**If targeting only Anthropic Claude:**
- Use `rig-core` with `rig::providers::anthropic` module
- API key via `ANTHROPIC_API_KEY` environment variable
- Streaming built-in

**If targeting local models (Ollama):**
- Use `rig-core` with `rig::providers::ollama` module
- Requires Ollama running locally
- Supports Llama, Mistral, Phi, etc.

**If multi-provider is critical:**
- Use `rig-core` abstraction layer
- Switch providers by changing client initialization
- Consider `genai` crate for lighter-weight multi-provider support

**If building complex multi-agent teams:**
- Consider `AutoAgents` framework (built on Ractor actor model)
- Or build on top of rig-core with custom orchestration
- Use tokio::sync::mpsc channels for inter-agent communication

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| rig-core 0.18 | tokio 1.x | rig requires tokio runtime |
| rmcp 1.2 | tokio 1.x | rmcp is tokio-native |
| reqwest 0.12 | tokio 1.x | reqwest uses tokio |
| tracing 0.1 | tokio 1.x | tracing has tokio feature for async support |
| schemars 0.8 | serde 1.0 | schemars derives from serde types |
| rmcp 1.2 | schemars 0.8 | rmcp uses schemars for tool schema generation |

## Key Architecture Decisions

### Why rig-core over alternatives

1. **Multi-provider abstraction**: Single API for Anthropic, OpenAI, Ollama, and 15+ providers
2. **Agent primitives**: Built-in `Agent` type with RAG support, tool integration
3. **Streaming support**: Native streaming completion API
4. **Vector store integrations**: Companion crates for MongoDB, LanceDB, Qdrant, SQLite
5. **Active development**: v0.18 released 2025, active community
6. **Production-ready**: Used in production systems

### Why rmcp for MCP

1. **Official SDK**: Maintained by modelcontextprotocol organization
2. **Complete implementation**: Tools, resources, prompts, sampling, logging
3. **Tokio-native**: Built for async from the ground up
4. **Macro support**: `#[tool]` macro for declarative tool definitions
5. **Active ecosystem**: 3.2k+ GitHub stars, 478 forks, 149+ contributors

### Why tokio (not smol/async-std)

1. **Ecosystem dominance**: Most crates support tokio first
2. **Performance**: Proven at scale (Discord, Cloudflare)
3. **Feature completeness**: Full-featured runtime with I/O, time, signals, process
4. **Documentation**: Official tokio.rs tutorial is excellent
5. **Community support**: Largest async Rust community

## Sources

- [rig-core crates.io](https://crates.io/crates/rig-core) — Rig framework (MEDIUM confidence for version, verified)
- [rig.rs documentation](https://docs.rig.rs/) — Official Rig docs (HIGH confidence)
- [rmcp GitHub](https://github.com/modelcontextprotocol/rust-sdk) — Official MCP Rust SDK (HIGH confidence)
- [Tokio documentation](https://tokio.rs/tokio/tutorial) — Official Tokio tutorial (HIGH confidence)
- [Reddit: async-std discontinued](https://www.reddit.com/r/rust/comments/1jc6gis/psa_asyncstd_has_been_officially_discontinued_use/) — Community announcement (HIGH confidence)
- [tracing vs log](https://tokio.rs/tokio/topics/tracing) — Official Tokio tracing guide (HIGH confidence)
- [thiserror vs anyhow](https://medium.com/@Murtza/error-handling-best-practices-in-rust-a-comprehensive-guide) — Error handling patterns (MEDIUM confidence)
- [parking_lot vs std::sync](https://www.reddit.com/r/rust/comments/1ok2vv2/inside_rusts_std_and_parking_lot_mutexes_who_wins/) — Performance comparison (MEDIUM confidence)
- [clap documentation](https://docs.rs/clap/latest/clap/) — CLI framework docs (HIGH confidence)
- [Rust LLM ecosystem overview](https://hackmd.io/@Hamze/Hy5LiRV1gg) — Community-maintained list (MEDIUM confidence)
- [genai crate](https://lib.rs/crates/genai) — Multi-provider alternative (MEDIUM confidence)
- [AutoAgents documentation](https://liquidos-ai.github.io/AutoAgents/) — Multi-agent framework (MEDIUM confidence)

---
*Stack research for: Rust Agent Harness*
*Researched: 2026-03-20*
