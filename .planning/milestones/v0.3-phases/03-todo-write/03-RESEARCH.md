# Phase 3: s03 - TodoWrite - Research

**Researched:** 2026-03-23
**Domain:** Task tracking with nag reminders for agent self-management
**Confidence:** HIGH

## Summary

TodoWrite implements a task list tool that allows the agent to track its own progress on multi-step tasks. The key innovation is the "nag reminder" mechanism - if the agent goes 3+ turns without updating its todo list, a reminder is injected to prompt it to use the tool. This prevents "drift" where the agent loses track of what it was doing.

The implementation requires three components:
1. **TodoManager** - State management for todo items with constraints (max 20, single in_progress)
2. **TodoTool** - Tool implementation using the existing Tool trait pattern
3. **Nag Integration** - Round counting and reminder injection in Session

**Primary recommendation:** Follow the established tool pattern from ReadTool/BashTool, integrate TodoManager into Session struct, and inject nag reminders before agent responses.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01: Memory storage, Session-internal management**
- Todos stored in `Session` struct as `TodoManager` field
- No persistence to file (deferred to Phase 7)
- Consistent with existing `messages: Vec<Message>` pattern

**D-02: Minimal field set**
| Field | Type | Description |
|-------|------|-------------|
| id | u32 | Unique identifier, auto-increment |
| text | String | Task description |
| status | TodoStatus | pending/in_progress/completed |

**NOT included:** owner (single-agent scenario), metadata (Phase 7), dependencies (Phase 7)

**D-03: Constraint validation at update() time**
1. Max 20 items: `items.len() <= 20`
2. Only one in_progress: count check on `status == InProgress`
3. Validation failure: return error, no modification

**D-04: Render format with markers + ID + text**
```
[ ] #1: Setup project structure
[>] #2: Implement TodoTool
[x] #3: Add tests
```
- `[ ]` = pending
- `[>]` = in_progress
- `[x]` = completed

**D-05: Session round counting + reminder injection**
1. `Session` maintains `rounds_since_todo: u32`
2. Increment after each user input
3. Reset to 0 when todo tool called
4. When `rounds_since_todo >= 3`, inject reminder before agent response

**D-06: Single operation interface**
```rust
pub struct TodoArgs {
    pub items: Vec<TodoItemInput>,
}
```
Behavior: Each call replaces entire todo list (matches Python implementation)

### Claude's Discretion

- Specific error type design
- TodoManager internal implementation details
- Test case organization

### Deferred Ideas (OUT OF SCOPE)

None - discussion stayed within phase scope.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PLAN-01 | TodoWrite with nag reminders - task list that reminds agent of pending work | TodoManager pattern (D-01/D-02/D-03), TodoTool interface (D-06), Nag mechanism (D-05), Render format (D-04) |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rig-core | 0.31 | Tool trait implementation | Already in use, defines Tool trait with NAME, Args, Error, Output |
| thiserror | 1.0 | Error type derivation | Project standard for library errors |
| serde | 1.0 | Serialization for Args | Required by Tool trait |
| schemars | 0.8 | JSON Schema generation | Required for ToolDefinition parameters |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1.44 | Async runtime | Tool::call is async |
| tracing | 0.1 | Logging | Info/debug logging for tool calls |

**No new dependencies required** - all needed crates are already in Cargo.toml.

## Architecture Patterns

### Recommended Project Structure

```
src/
├── planning/           # NEW module for planning-related types
│   ├── mod.rs         # Exports TodoManager, TodoItem, TodoStatus
│   └── todo.rs        # TodoManager implementation
├── tools/
│   ├── mod.rs         # Add: mod todo; pub use todo::TodoTool;
│   └── todo.rs        # NEW: TodoTool implementation
├── cli/
│   └── session.rs     # Add: rounds_since_todo, todo_manager fields
└── llm/
    └── provider.rs    # Add: .tool(TodoTool) to AgentBuilder chains
```

### Pattern 1: Tool Trait Implementation

**What:** Follow existing ReadTool pattern for TodoTool
**When to use:** All tools must implement rig::tool::Tool trait

**Example:**
```rust
// Source: src/tools/read.rs (existing pattern)
use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TodoArgs {
    /// List of todo items (replaces entire list)
    pub items: Vec<TodoItemInput>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TodoItemInput {
    /// Task ID (1-20)
    pub id: u32,
    /// Task description
    pub text: String,
    /// Task status: pending, in_progress, completed
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoTool;

impl Tool for TodoTool {
    const NAME: &'static str = "todo";

    type Error = TodoError;
    type Args = TodoArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "todo".to_string(),
            description: "Update task list. Track progress on multi-step tasks.".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(TodoArgs))
                .expect("Failed to generate schema"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Convert input to validated TodoItem instances
        // Call TodoManager::update()
        // Return rendered output
    }
}
```

### Pattern 2: TodoManager State

**What:** In-memory task management with constraint validation
**When to use:** Session-scoped task tracking

**Example:**
```rust
// Source: CONTEXT.md D-01/D-03
pub struct TodoManager {
    items: Vec<TodoItem>,
}

pub struct TodoItem {
    pub id: u32,
    pub text: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl TodoManager {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn update(&mut self, items: Vec<TodoItem>) -> Result<String, TodoError> {
        // Validate constraints
        if items.len() > 20 {
            return Err(TodoError::TooManyItems(items.len()));
        }
        let in_progress_count = items.iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .count();
        if in_progress_count > 1 {
            return Err(TodoError::MultipleInProgress(in_progress_count));
        }
        self.items = items;
        Ok(self.render())
    }

    pub fn render(&self) -> String {
        if self.items.is_empty() {
            return "No todos.".to_string();
        }
        let mut lines: Vec<String> = self.items.iter().map(|item| {
            let marker = match item.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[>]",
                TodoStatus::Completed => "[x]",
            };
            format!("{} #{}: {}", marker, item.id, item.text)
        }).collect();
        let done = self.items.iter().filter(|i| i.status == TodoStatus::Completed).count();
        lines.push(format!("\n({}/{})", done, self.items.len()));
        lines.join("\n")
    }
}
```

### Pattern 3: Nag Reminder Integration

**What:** Round counting in Session, reminder injection before agent response
**When to use:** Every agent turn in Session::run()

**Example:**
```rust
// Source: CONTEXT.md D-05
pub struct Session {
    messages: Vec<Message>,
    turn_count: u32,
    todo_manager: TodoManager,        // NEW
    rounds_since_todo: u32,           // NEW
}

// In Session::run() main loop:
// After agent_loop returns, before printing response:
if self.rounds_since_todo >= 3 && !self.todo_manager.is_empty() {
    print!("\n<reminder>You have pending todos. Use the 'todo' tool to update your task list.</reminder>\n\n");
}
self.rounds_since_todo += 1;

// When TodoTool is called (via callback or state check):
self.rounds_since_todo = 0;
```

### Anti-Patterns to Avoid

- **Storing todos in messages[]** - TodoManager is separate state, not part of conversation history
- **Multiple in_progress tasks** - Constraint must be enforced, not advisory
- **Silent constraint failures** - Return errors, don't auto-fix invalid input
- **Nag reminder in system prompt** - Must be injected dynamically based on round count

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Tool schema generation | Manual JSON schema | schemars::schema_for! | Consistent with existing tools, type-safe |
| Error types | String errors | thiserror derive | Project standard, proper Error trait |
| Status parsing | String matching | FromStr impl for TodoStatus | Type-safe conversion with clear errors |

## Common Pitfalls

### Pitfall 1: TodoTool needs TodoManager access

**What goes wrong:** Tool trait's `call()` method takes only `self` and `args` - no access to Session state.

**Why it happens:** rig-core's Tool trait is designed for stateless tools. TodoTool needs mutable access to TodoManager.

**How to avoid:** Two approaches:
1. **Preferred:** TodoTool stores `Arc<Mutex<TodoManager>>` internally - tool and session share reference
2. **Alternative:** Return structured result from TodoTool, let Session parse and update TodoManager

**Recommendation:** Use `Arc<Mutex<TodoManager>>` pattern - simpler integration, matches Python reference.

**Warning signs:** Tool call succeeds but todos aren't persisted in Session.

### Pitfall 2: Nag reminder injection timing

**What goes wrong:** Reminder printed at wrong time (before user input, after tool call, etc.)

**Why it happens:** Unclear when "round" increments - is it per user turn or per tool call loop?

**How to avoid:** Follow Python reference exactly:
- Increment `rounds_since_todo` AFTER each agent response (user turn completes)
- Reset to 0 when todo tool is called
- Check and inject BEFORE printing agent response

**Warning signs:** Agent gets nag reminders immediately after using todo tool.

### Pitfall 3: Status string parsing

**What goes wrong:** LLM returns invalid status string ("in-progress", "IN_PROGRESS", etc.)

**Why it happens:** JSON schema enum validation is advisory, not enforced by all LLMs.

**How to avoid:** Implement robust FromStr for TodoStatus with case-insensitive matching and clear error messages.

**Warning signs:** TodoTool errors on otherwise valid-looking input.

### Pitfall 4: Tool registration order

**What goes wrong:** TodoTool not available to agent after implementation.

**Why it happens:** Forgot to add tool to AgentBuilder chain in provider.rs for both Anthropic and OpenAI.

**How to avoid:** Update BOTH provider branches in `create_provider()`:
```rust
.tool(BashTool)
.tool(ReadTool)
// ... other tools ...
.tool(TodoTool)  // Add after existing tools
```

**Warning signs:** Agent says "I don't have access to a todo tool" when asked to track tasks.

## Code Examples

### Complete TodoError type

```rust
// Source: Project pattern (thiserror)
#[derive(Debug, thiserror::Error)]
pub enum TodoError {
    #[error("Too many todos: {0} (max 20)")]
    TooManyItems(usize),

    #[error("Multiple tasks in progress: {0} (only one allowed)")]
    MultipleInProgress(usize),

    #[error("Invalid status '{0}' (expected: pending, in_progress, completed)")]
    InvalidStatus(String),

    #[error("Item #{0}: text is required")]
    MissingText(u32),
}
```

### TodoStatus FromStr implementation

```rust
// Source: Robust parsing pattern
impl std::str::FromStr for TodoStatus {
    type Err = TodoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(TodoStatus::Pending),
            "in_progress" | "in-progress" => Ok(TodoStatus::InProgress),
            "completed" => Ok(TodoStatus::Completed),
            _ => Err(TodoError::InvalidStatus(s.to_string())),
        }
    }
}
```

### TodoItemInput to TodoItem conversion

```rust
// Source: Validation pattern from Python reference
impl TodoItemInput {
    pub fn validate(self) -> Result<TodoItem, TodoError> {
        let text = self.text.trim();
        if text.is_empty() {
            return Err(TodoError::MissingText(self.id));
        }
        let status = self.status.parse::<TodoStatus>()?;
        Ok(TodoItem {
            id: self.id,
            text: text.to_string(),
            status,
        })
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual task tracking in prose | Structured todo tool | s03 curriculum | Agent can track progress programmatically |
| No drift prevention | Nag reminders | s03 curriculum | Forces agent to maintain task awareness |

**Deprecated/outdated:**
- None for this phase - TodoWrite is the baseline pattern

## Open Questions

1. **TodoManager sharing mechanism**
   - What we know: Tool trait is stateless, need shared mutable state
   - What's unclear: Best pattern for Arc<Mutex<>> vs callback approach
   - Recommendation: Use Arc<Mutex<TodoManager>> - simpler, matches Python's global TODO object

2. **Nag reminder format**
   - What we know: Python uses `<reminder>` text block
   - What's unclear: Exact wording and whether to include pending count
   - Recommendation: Match Python format exactly initially, can iterate later

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rig-core | Tool trait | Yes | 0.31 | - |
| thiserror | Error types | Yes | 1.0 | - |
| serde/schemars | Args schema | Yes | 1.0/0.8 | - |
| tokio | Async runtime | Yes | 1.44 | - |

**Missing dependencies with no fallback:**
- None - all dependencies available

**Missing dependencies with fallback:**
- None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Built-in Rust tests + tokio::test |
| Config file | None - tests inline in modules |
| Quick run command | `cargo test todo --lib` |
| Full suite command | `cargo test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PLAN-01 | TodoManager update with constraints | unit | `cargo test todo_manager --lib` | Wave 0 |
| PLAN-01 | TodoTool call returns rendered output | unit | `cargo test todo_tool --lib` | Wave 0 |
| PLAN-01 | Nag reminder after 3 rounds | unit | `cargo test nag_reminder --lib` | Wave 0 |
| PLAN-01 | Status parsing (case-insensitive) | unit | `cargo test todo_status --lib` | Wave 0 |
| PLAN-01 | Constraint errors (20 items, 1 in_progress) | unit | `cargo test todo_constraints --lib` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test todo --lib`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/planning/todo.rs` - TodoManager implementation with tests
- [ ] `src/tools/todo.rs` - TodoTool implementation with tests
- [ ] `src/planning/mod.rs` - Module file for planning types
- [ ] Update `src/tools/mod.rs` - Add todo module export
- [ ] Update `src/cli/session.rs` - Add todo_manager and rounds_since_todo fields
- [ ] Update `src/llm/provider.rs` - Add TodoTool to AgentBuilder chains

## Sources

### Primary (HIGH confidence)
- CONTEXT.md - User decisions for implementation details
- src/tools/read.rs - Existing tool implementation pattern
- src/llm/provider.rs - Tool registration pattern
- Python reference: https://raw.githubusercontent.com/shareAI-lab/learn-claude-code/main/agents/s03_todo_write.py

### Secondary (MEDIUM confidence)
- rig-core 0.31 Tool trait docs: https://docs.rs/rig-core/0.31/rig/tool/trait.Tool.html

### Tertiary (LOW confidence)
- None - all findings verified against project code or official docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All dependencies already in project
- Architecture: HIGH - Pattern established by existing tools
- Pitfalls: HIGH - Based on direct analysis of Tool trait constraints

**Research date:** 2026-03-23
**Valid until:** 30 days (stable Rust patterns, rig-core 0.31 API)
