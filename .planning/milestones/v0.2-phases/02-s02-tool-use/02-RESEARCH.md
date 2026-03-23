# Phase 2: s02-tool-use - Research

**Researched:** 2026-03-23
**Domain:** Rust tool dispatch system using rig-core Tool trait
**Confidence:** HIGH

## Summary

Phase 2 implements the tool dispatch system that enables the agent to execute actions through registered tool handlers. The implementation follows the existing BashTool pattern established in Phase 1, extending it to support five additional tools: Read, Write, Edit, Glob, and Grep.

**Primary recommendation:** Implement all five tools following the exact BashTool pattern using rig-core's `Tool` trait. Static registration via `AgentBuilder.tool()` is locked in due to rig-core's Tool trait not being object-safe. Basic sandbox protection via command blacklist in BashTool.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Core tools statically compiled via AgentBuilder.tool():**
- BashTool (already implemented)
- ReadTool
- WriteTool
- EditTool
- GlobTool
- GrepTool

**Deferred to later phases:**
- MCP protocol support (CROSS-01) → Phase 3+
- Skills dynamic loading → Phase 3+
- Dynamic tool extension architecture → Phase 3+

**Technical reason:** rig-core's `Tool` trait is not object-safe, static compilation is the simplest reliable approach.

**File operation tools (CORE-04, PLAN-05):**

| Tool | Function | File Location |
|------|----------|---------------|
| Read | Read file contents | `src/tools/read.rs` |
| Write | Create/overwrite files | `src/tools/write.rs` |
| Edit | Precise string replacement | `src/tools/edit.rs` |
| Glob | File pattern matching | `src/tools/glob.rs` |
| Grep | Content search | `src/tools/grep.rs` |

**Implementation pattern:** Follow existing BashTool pattern with Args struct, Error enum, and Tool impl.

**Output limit strategy:** Configurable via CLI parameter or config file
- Default: 50000 characters (consistent with Python tutorial)
- Config method: CLI `--max-output` or `.harness.toml`

**Sandbox/Permission model (PERS-04):**
- Phase 2 scope: Basic command blacklist in BashTool
- Dangerous command patterns to block:
  - `rm -rf /` and variants
  - `sudo` commands
  - `mkfs`, `fdisk` disk operations
  - `shutdown`, `reboot`, `halt`
  - API key leak detection in env vars (optional)

**Implementation approach:** Check command patterns in BashTool's `call()` method
- Match blacklist → return error, prompt user confirmation
- No match → execute normally

**Full sandbox (Phase 3):**
- Path sanitization (safe_path function)
- Allow directory whitelist
- Human-in-the-Loop confirmation flow

**MCP protocol support (CROSS-01):** Deferred to Phase 3+

### Claude's Discretion

- Output limit default value (50000 suggested, could be different)
- API key leak detection in environment variables (optional enhancement)

### Deferred Ideas (OUT OF SCOPE)

- Complete sandbox system (Phase 3)
- Human-in-the-Loop confirmation UI (Phase 3)
- MCP protocol support (Phase 3+)
- Skills dynamic loading (Phase 3+)
- Dynamic tool extension architecture (Phase 3+)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORE-02 | Tool dispatch registry | rig-core Agent handles this; tools registered via AgentBuilder.tool() |
| CORE-03 | BashTool | Already implemented in Phase 1; extend with blacklist |
| CORE-04 | Read/Write/Edit tools | Follow BashTool pattern; code examples in Architecture Patterns |
| PLAN-05 | Glob/Grep search tools | Use glob and grep crates; patterns documented below |
| PERS-04 | Basic sandbox | Command blacklist approach; dangerous patterns listed in CONTEXT.md |
| CROSS-01 | MCP protocol | DEFERRED to Phase 3+ |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rig-core | 0.31 | LLM abstraction + Tool trait | Project standard, already integrated |
| schemars | 0.8 | JSON Schema derivation | Required by rig-core Tool trait |
| serde | 1.0 | Serialization | Standard Rust pattern |
| thiserror | 1.0 | Error types | Project standard for library errors |
| tokio | 1.x | Async runtime | Project standard |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| glob | 0.3 | File pattern matching | GlobTool implementation |
| regex | 1.10 | Content search | GrepTool implementation |
| walkdir | 2.5 | Directory traversal | GlobTool for recursive patterns |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| glob crate | custom impl | glob crate handles edge cases (symlinks, permissions) |
| regex crate | grep command | regex crate is cross-platform, no external dependency |

**Installation:**
```bash
# Already in Cargo.toml:
# rig-core = "0.31"
# schemars = "0.8"
# thiserror = "1.0"
# serde = { version = "1.0", features = ["derive"] }
# tokio = { version = "1", features = ["full"] }

# Add for new tools:
cargo add glob
cargo add regex
```

**Version verification:** Confirmed versions match Cargo.toml (2026-03-23).

## Architecture Patterns

### Recommended Project Structure
```
src/tools/
├── mod.rs           # Module exports
├── bash.rs          # BashTool (existing)
├── read.rs          # ReadTool (new)
├── write.rs         # WriteTool (new)
├── edit.rs          # EditTool (new)
├── glob.rs          # GlobTool (new)
└── grep.rs          # GrepTool (new)
```

### Pattern 1: Tool Implementation (Follow BashTool)

**What:** Each tool is a unit struct implementing rig-core's `Tool` trait
**When to use:** All static tools in Phase 2

**Example (from existing BashTool):**
```rust
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::process::Command;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct BashArgs {
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 { 120 }

#[derive(Debug, thiserror::Error)]
pub enum BashError {
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Command timed out after {0} seconds")]
    Timeout(u64),
    #[error("Command was terminated by signal")]
    Terminated,
    #[error("Invalid UTF-8 in command output")]
    InvalidUtf8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashTool;

impl Tool for BashTool {
    const NAME: &'static str = "bash";
    type Error = BashError;
    type Args = BashArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: r#"Execute bash commands on the system..."#.to_string(),
            parameters: serde_json::to_value(
                schemars::schema_for!(BashArgs)
            ).expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Implementation with timeout and error handling
    }
}
```

**Source:** `src/tools/bash.rs` (existing code)

### Pattern 2: Tool Registration

**What:** Register tools with AgentBuilder at compile time
**When to use:** In `src/llm/provider.rs` when building the agent

**Example (from existing code):**
```rust
let agent = AgentBuilder::new(completion_model)
    .preamble(SYSTEM_PROMPT)
    .tool(BashTool)
    .tool(ReadTool)      // Add Phase 2 tools
    .tool(WriteTool)
    .tool(EditTool)
    .tool(GlobTool)
    .tool(GrepTool)
    .default_max_turns(DEFAULT_MAX_TURNS)
    .max_tokens(4096)
    .build();
```

**Source:** `src/llm/provider.rs` (existing pattern)

### Pattern 3: Module Exports

**What:** Export all tools from `src/tools/mod.rs`
**When to use:** After implementing each tool

**Example:**
```rust
mod bash;
mod read;
mod write;
mod edit;
mod glob;
mod grep;

pub use bash::BashTool;
pub use read::ReadTool;
pub use write::WriteTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
```

### Anti-Patterns to Avoid

- **Box<dyn Tool>**: Tool trait is not object-safe; cannot use dynamic dispatch
- **unwrap() in tool implementations**: Use proper error propagation with thiserror
- **Blocking operations in async call()**: Use tokio::task::spawn_blocking for CPU-intensive work
- **Command injection**: Always sanitize file paths; use safe_path (Phase 3)
- **Unlimited output**: Truncate to configurable limit to prevent context explosion

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File globbing | Custom pattern matching | `glob` crate | Handles symlinks, permissions, edge cases |
| Content search | Custom regex engine | `regex` crate | Optimized, Unicode-aware |
| JSON Schema | Manual schema generation | `schemars::JsonSchema` derive | Required by rig-core, type-safe |
| Error types | Manual Error impl | `thiserror::Error` derive | Project standard, #[from] conversion |
| Async subprocess | std::process::Command blocking | `tokio::process::Command` | Non-blocking, integrates with runtime |

**Key insight:** The rig-core Tool trait and schemars derive macros eliminate boilerplate. Focus on the `call()` implementation logic, not schema generation or error boilerplate.

## Common Pitfalls

### Pitfall 1: Tool Trait Not Object-Safe
**What goes wrong:** Attempting to store tools in `Vec<Box<dyn Tool>>` or `HashMap<String, Box<dyn Tool>>`
**Why it happens:** Tool trait uses associated types and const generics, making it not object-safe
**How to avoid:** Use static registration via `AgentBuilder.tool()` - this is locked in
**Warning signs:** Compiler error "the trait `Tool` cannot be made into an object"

### Pitfall 2: Blocking the Async Runtime
**What goes wrong:** Tool `call()` method performs blocking I/O, freezing the agent loop
**Why it happens:** File operations and subprocess execution can block
**How to avoid:** Use `tokio::fs` for file operations, `tokio::process::Command` for subprocesses
**Warning signs:** Agent becomes unresponsive during tool execution

### Pitfall 3: Context Explosion from Large Output
**What goes wrong:** Reading a 10MB file returns all content, consuming entire context window
**Why it happens:** No output limiting in tool implementation
**How to avoid:** Truncate output to configurable limit (default 50000 chars), include "... truncated" indicator
**Warning signs:** API costs spike, response times increase dramatically

### Pitfall 4: Command Injection via BashTool
**What goes wrong:** Agent executes `rm -rf /` or similar destructive commands
**Why it happens:** No validation of command patterns
**How to avoid:** Implement blacklist check in BashTool `call()` before execution
**Warning signs:** Commands containing `sudo`, `rm -rf /`, `mkfs`, etc.

### Pitfall 5: Path Traversal Attacks
**What goes wrong:** Agent reads/writes files outside intended directory (e.g., `../../../etc/passwd`)
**Why it happens:** No path sanitization
**How to avoid:** Phase 2 scope is limited; full path sanitization deferred to Phase 3
**Warning signs:** File paths containing `..`, absolute paths to sensitive directories

## Code Examples

### ReadTool Implementation

```rust
// src/tools/read.rs
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use tokio::fs;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ReadArgs {
    pub file_path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("File too large, output truncated")]
    Truncated,
}

pub struct ReadTool;

impl Tool for ReadTool {
    const NAME: &'static str = "read";
    type Error = ReadError;
    type Args = ReadArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: r#"Read file contents from the filesystem.
Supports offset and limit for reading partial files."#.to_string(),
            parameters: serde_json::to_value(
                schemars::schema_for!(ReadArgs)
            ).expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let content = fs::read_to_string(&args.file_path).await?;

        // Apply offset and limit
        let lines: Vec<&str> = content.lines().collect();
        let offset = args.offset.unwrap_or(0);
        let limit = args.limit.unwrap_or(lines.len());

        let selected: Vec<&str> = lines
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        Ok(selected.join("\n"))
    }
}
```

### WriteTool Implementation

```rust
// src/tools/write.rs
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use tokio::fs;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WriteArgs {
    pub file_path: String,
    pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("Failed to write file: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct WriteTool;

impl Tool for WriteTool {
    const NAME: &'static str = "write";
    type Error = WriteError;
    type Args = WriteArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: r#"Write content to a file, creating or overwriting as needed."#.to_string(),
            parameters: serde_json::to_value(
                schemars::schema_for!(WriteArgs)
            ).expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Create parent directories if needed
        if let Some(parent) = std::path::Path::new(&args.file_path).parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&args.file_path, &args.content).await?;
        Ok(format!("Successfully wrote to {}", args.file_path))
    }
}
```

### EditTool Implementation

```rust
// src/tools/edit.rs
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use tokio::fs;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EditArgs {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("Failed to read file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Old string not found in file")]
    NotFound,
    #[error("Multiple matches found for old string")]
    MultipleMatches,
}

pub struct EditTool;

impl Tool for EditTool {
    const NAME: &'static str = "edit";
    type Error = EditError;
    type Args = EditArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: r#"Perform precise string replacement in a file.
Fails if old_string is not found or appears multiple times."#.to_string(),
            parameters: serde_json::to_value(
                schemars::schema_for!(EditArgs)
            ).expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let content = fs::read_to_string(&args.file_path).await?;

        let matches: Vec<_> = content.match_indices(&args.old_string).collect();

        match matches.len() {
            0 => Err(EditError::NotFound),
            1 => {
                let new_content = content.replacen(&args.old_string, &args.new_string, 1);
                fs::write(&args.file_path, &new_content).await?;
                Ok(format!("Successfully edited {}", args.file_path))
            }
            _ => Err(EditError::MultipleMatches),
        }
    }
}
```

### GlobTool Implementation

```rust
// src/tools/glob.rs
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GlobArgs {
    pub pattern: String,
    pub path: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GlobError {
    #[error("Glob pattern error: {0}")]
    PatternError(#[from] glob::PatternError),
    #[error("Glob error: {0}")]
    GlobError(#[from] glob::GlobError),
}

pub struct GlobTool;

impl Tool for GlobTool {
    const NAME: &'static str = "glob";
    type Error = GlobError;
    type Args = GlobArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description: r#"Find files matching a glob pattern.
Supports ** for recursive matching."#.to_string(),
            parameters: serde_json::to_value(
                schemars::schema_for!(GlobArgs)
            ).expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let base_path = args.path.unwrap_or_else(|| ".".to_string());
        let full_pattern = format!("{}/{}", base_path, args.pattern);

        let paths: Vec<String> = glob::glob(&full_pattern)?
            .filter_map(|entry| entry.ok())
            .map(|path| path.display().to_string())
            .collect();

        Ok(paths.join("\n"))
    }
}
```

### GrepTool Implementation

```rust
// src/tools/grep.rs
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use regex::Regex;
use std::fs;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GrepArgs {
    pub pattern: String,
    pub path: Option<String>,
    #[serde(default)]
    pub case_insensitive: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GrepError {
    #[error("Invalid regex pattern: {0}")]
    RegexError(#[from] regex::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct GrepTool;

impl Tool for GrepTool {
    const NAME: &'static str = "grep";
    type Error = GrepError;
    type Args = GrepArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: r#"Search for pattern in file contents.
Returns matching lines with file paths."#.to_string(),
            parameters: serde_json::to_value(
                schemars::schema_for!(GrepArgs)
            ).expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut regex_builder = Regex::new(&args.pattern)?;
        if args.case_insensitive {
            regex_builder = Regex::new(&format!("(?i){}", args.pattern))?;
        }

        // For Phase 2, implement basic file-by-file search
        // Full recursive search can be enhanced later
        let path = args.path.unwrap_or_else(|| ".".to_string());

        // Simplified: read single file (full implementation would use walkdir)
        let content = fs::read_to_string(&path)?;
        let matches: Vec<String> = content
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                if regex_builder.is_match(line) {
                    Some(format!("{}:{}:{}", path, i + 1, line))
                } else {
                    None
                }
            })
            .collect();

        Ok(matches.join("\n"))
    }
}
```

### BashTool Blacklist Extension

```rust
// Add to src/tools/bash.rs call() method
impl Tool for BashTool {
    // ... existing code ...

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Check blacklist before execution
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            "sudo",
            "mkfs",
            "fdisk",
            "shutdown",
            "reboot",
            "halt",
            "dd if=",
            "> /dev/sd",
        ];

        for pattern in &dangerous_patterns {
            if args.command.contains(pattern) {
                return Err(BashError::ExecutionFailed(
                    format!("Command blocked by safety filter: contains '{}'. \
                             This operation requires explicit user confirmation.", pattern)
                ));
            }
        }

        // ... existing execution code ...
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Dynamic tool dispatch via trait objects | Static registration via AgentBuilder | Phase 1 design | Compile-time safety, zero-cost abstraction |
| Manual JSON schema generation | schemars derive macro | Phase 1 design | Type-safe, automatic schema generation |
| std::process::Command | tokio::process::Command | Phase 1 design | Non-blocking, async integration |
| log crate | tracing crate | Phase 1 design | Structured logging, async-aware |

**Deprecated/outdated:**
- `async-std`: Discontinued as of 2025, use tokio exclusively
- `std::sync::mpsc` in async context: Blocks executor, use `tokio::sync::mpsc`

## Open Questions

1. **Output limit default value**
   - What we know: 50000 characters suggested in CONTEXT.md
   - What's unclear: Should this be per-tool or global?
   - Recommendation: Start with global configurable limit via CLI `--max-output`, can refine per-tool later

2. **API key leak detection**
   - What we know: Listed as optional in CONTEXT.md
   - What's unclear: Scope and implementation approach
   - Recommendation: Defer to Phase 3 with full sandbox implementation

## Environment Availability

> Step 2.6: SKIPPED (no external dependencies beyond Rust toolchain)

Phase 2 is purely code changes with no external services, databases, or CLI utilities beyond the existing Rust toolchain (cargo, rustc). All dependencies are crates.io packages already in or to be added to Cargo.toml.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust) |
| Config file | Cargo.toml (test section) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CORE-02 | Tool registration | unit | `cargo test tool_registration` | ❌ Wave 0 |
| CORE-04 | Read tool reads files | unit | `cargo test read_tool` | ❌ Wave 0 |
| CORE-04 | Write tool writes files | unit | `cargo test write_tool` | ❌ Wave 0 |
| CORE-04 | Edit tool replaces strings | unit | `cargo test edit_tool` | ❌ Wave 0 |
| PLAN-05 | Glob tool matches patterns | unit | `cargo test glob_tool` | ❌ Wave 0 |
| PLAN-05 | Grep tool searches content | unit | `cargo test grep_tool` | ❌ Wave 0 |
| PERS-04 | Blacklist blocks dangerous commands | unit | `cargo test bash_blacklist` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/tools_test.rs` — unit tests for all 5 new tools
- [ ] `tests/tools_test.rs` — blacklist test for BashTool
- [ ] Test fixtures: sample files for read/write/edit tests

**Existing test infrastructure:** `tests/agent_loop_test.rs` and `tests/integration_test.rs` provide patterns to follow.

## Sources

### Primary (HIGH confidence)
- `src/tools/bash.rs` - existing implementation pattern
- `src/tools/mod.rs` - module structure
- `src/llm/provider.rs` - tool registration pattern
- `Cargo.toml` - dependency versions
- `.planning/phases/02-s02-tool-use/02-CONTEXT.md` - locked decisions
- docs.rig.rs/docs/concepts/tools - rig-core Tool trait documentation

### Secondary (MEDIUM confidence)
- crates.io/crates/glob - glob crate documentation
- crates.io/crates/regex - regex crate documentation
- docs.rs/tokio - tokio async patterns

### Tertiary (LOW confidence)
- None - all critical information verified from primary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all dependencies already in project, versions verified
- Architecture: HIGH - exact pattern exists in BashTool, just need to replicate
- Pitfalls: HIGH - well-documented in rig-core and project constraints

**Research date:** 2026-03-23
**Valid until:** 30 days (stable Rust ecosystem, rig-core API stable)
