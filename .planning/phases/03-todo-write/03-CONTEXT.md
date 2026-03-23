# Phase 3: s03 - TodoWrite - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

实现 TodoWrite 工具，让 agent 能够追踪多步骤任务进度，防止 drift。

**Scope:**
- TodoManager 状态管理（pending/in_progress/completed）
- todo 工具注册到 tool dispatch
- Nag reminder 机制（3+ 轮未调用 todo 时注入提醒）
- 约束：一次只能有一个 in_progress，最大 20 条

**Out of Scope:**
- 任务持久化到文件（Phase 7: s07 - Tasks）
- 复杂任务依赖关系（Phase 7）
- 后台任务执行（Phase 8）

</domain>

<decisions>
## Implementation Decisions

### 存储架构

**D-01: 内存存储，Session 内管理**

- Todos 存储在 `Session` 结构体中，作为 `TodoManager` 字段
- 不持久化到文件（延后到 Phase 7）
- 与现有 `messages: Vec<Message>` 模式一致

```rust
// src/planning/todo.rs
pub struct TodoManager {
    items: Vec<TodoItem>,
    rounds_since_update: u32,
}

pub struct TodoItem {
    pub id: u32,
    pub text: String,
    pub status: TodoStatus,
}

pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}
```

### Todo 结构

**D-02: 最小字段集**

| 字段 | 类型 | 说明 |
|------|------|------|
| id | u32 | 唯一标识符，自增 |
| text | String | 任务描述 |
| status | TodoStatus | pending/in_progress/completed |

**不包含：**
- owner（单 agent 场景不需要）
- metadata（Phase 7 需要时再加）
- dependencies（Phase 7）

### 约束验证

**D-03: update() 时验证约束**

1. **最大 20 条**: `items.len() <= 20`
2. **只有一个 in_progress**: 遍历检查 `status == InProgress` 的数量
3. **验证失败**: 返回错误信息，不做任何修改

```rust
pub fn update(&mut self, items: Vec<TodoItem>) -> Result<String, TodoError> {
    // 验证约束
    if items.len() > 20 {
        return Err(TodoError::TooManyItems(items.len()));
    }
    let in_progress_count = items.iter()
        .filter(|i| i.status == TodoStatus::InProgress)
        .count();
    if in_progress_count > 1 {
        return Err(TodoError::MultipleInProgress(in_progress_count));
    }
    // 更新并返回渲染结果
    self.items = items;
    Ok(self.render())
}
```

### 渲染格式

**D-04: 标记符号 + ID + 文本**

```
[ ] #1: Setup project structure
[>] #2: Implement TodoTool
[x] #3: Add tests
```

- `[ ]` = pending
- `[>]` = in_progress
- `[x]` = completed

### Nag Reminder 机制

**D-05: Session 轮次计数 + 提醒注入**

1. `Session` 维护 `rounds_since_todo: u32`
2. 每次用户输入后递增
3. 调用 todo 工具后重置为 0
4. `rounds_since_todo >= 3` 时，在下次 agent 响应前注入提醒

**注入方式：** 在 agent_loop 返回的响应前插入 `<reminder>` 文本

```rust
// In Session::run()
if self.rounds_since_todo >= 3 {
    print!("\n<reminder>You have pending todos. Use the 'todo' tool to update your task list.</reminder>\n\n");
}
self.rounds_since_todo += 1;
```

### TodoTool 接口

**D-06: 单一操作接口**

```rust
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
```

**行为：** 每次调用替换整个 todo 列表（与 Python 实现一致）

### Claude's Discretion

- 具体错误类型设计
- TodoManager 内部实现细节
- 测试用例组织

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Reference Implementation
- `https://github.com/shareAI-lab/learn-claude-code/blob/main/agents/s03_todo_write.py` — Python 参考实现
- `https://github.com/shareAI-lab/learn-claude-code/blob/main/docs/en/s03-todo-write.md` — 设计文档

### Existing Code Patterns
- `src/tools/read.rs` — 工具实现模式参考
- `src/cli/session.rs` — Session 结构和 agent_loop 调用
- `src/agent/message.rs` — 消息类型定义

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ReadTool` 模式：Tool trait + Args struct + Error enum
- `Session` 结构：已有 messages, turn_count 字段，可添加 todo_manager
- `thiserror` 依赖：用于定义 TodoError

### Established Patterns
- 工具注册：通过 `pub use` 导出，在 main.rs 中通过 AgentBuilder.tool() 注册
- 错误处理：使用 thiserror 定义错误类型，Result<T, Error> 传播
- 测试模式：使用 tempfile 创建临时文件，tokio::test 异步测试

### Integration Points
- `src/tools/mod.rs` — 添加 `mod todo;` 和 `pub use todo::TodoTool;`
- `src/cli/session.rs` — 添加 `todo_manager: TodoManager` 字段
- `src/lib.rs` 或新建 `src/planning/mod.rs` — TodoManager 定义
- `src/main.rs` — 在 AgentBuilder 链中添加 `.tool(TodoTool)`

</code_context>

<specifics>
## Specific Ideas

**参照 learn-claude-code Python 实现：**

1. **TodoManager 类** — 提供 `update()` 和 `render()` 方法
2. **状态枚举** — pending/in_progress/completed 三种状态
3. **Nag 注入** — 作为 `<reminder>` 标签插入到 agent 输出中
4. **工具名** — `todo`（不是 `todo_write`）

**与 Python 实现的差异：**
- Rust 强类型：status 使用 enum 而非字符串
- 错误处理：使用 Result 而非异常
- 内存安全：无需担心类型错误

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 03-todo-write*
*Context gathered: 2026-03-23*
