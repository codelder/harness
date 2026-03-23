# Phase 2: s02 - Tool Use

## Phase Goal

Build tool dispatch system so agent can execute actions through registered tool handlers.

## Locked Decisions

### 工具注册架构 (CORE-02)

**状态：锁定**

**Phase 2 决策：核心工具静态编译**

通过 AgentBuilder.tool() 在编译时注册：
- BashTool（已实现）
- ReadTool
- WriteTool
- EditTool
- GlobTool
- GrepTool

**延后到后续阶段：**
- MCP 协议支持（CROSS-01）→ Phase 3+
- Skills 动态加载 → Phase 3+
- 动态工具扩展架构 → Phase 3+

**技术原因：** rig-core 的 `Tool` trait 不是 object-safe，静态编译是最简单可靠的方式

---

### 文件操作工具 (CORE-04, PLAN-05)

**状态：锁定**

**实现范围：** Phase 2 实现全部 5 个工具

| 工具 | 功能 | 文件位置 |
|------|------|----------|
| Read | 读取文件内容 | `src/tools/read.rs` |
| Write | 创建/覆盖文件 | `src/tools/write.rs` |
| Edit | 精确字符串替换 | `src/tools/edit.rs` |
| Glob | 文件模式匹配 | `src/tools/glob.rs` |
| Grep | 内容搜索 | `src/tools/grep.rs` |

**实现模式：** 遵循现有 BashTool 模式

```rust
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ReadArgs {
    pub file_path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

pub struct ReadTool;

impl Tool for ReadTool {
    const NAME: &'static str = "read";
    type Error = ReadError;
    type Args = ReadArgs;
    type Output = String;
}
```

**输出限制策略：** 可配置（通过 CLI 参数或配置文件）
- 默认值：待定（建议 50000 字符与 Python 教程一致）
- 配置方式：CLI `--max-output` 或 `.harness.toml`

---

### 沙箱/权限模型 (PERS-04)

**状态：部分锁定**

**Phase 2 范围：** 不实现完整沙箱（延后到 Phase 3）

**高风险操作确认：** Phase 2 实现简单黑名单检查

危险命令模式：
- `rm -rf /` 及其变体
- `sudo` 命令
- `mkfs`, `fdisk` 等磁盘操作
- `shutdown`, `reboot`, `halt`
- 环境变量中的 API key 泄露检测（可选）

**实现方式：** 在 BashTool 的 `call()` 方法中检查命令模式
- 匹配黑名单 → 返回错误，提示需要用户确认
- 不匹配 → 正常执行

**完整沙箱（Phase 3）：**
- 路径净化（safe_path 函数）
- 允许目录白名单
- Human-in-the-Loop 确认流程

---

### MCP 协议支持 (CROSS-01)

**状态：延后**

- Phase 2 不实现 MCP
- 延后到 Phase 3 或更晚
- 届时使用 `rmcp` crate（官方 Rust SDK）

---

## Prior Context Applied

From Phase 1 (01-CONTEXT.md):
- ✅ BashTool 已实现
- ✅ Tool loop 由 rig-core Agent 处理
- ✅ 工具通过 AgentBuilder.tool() API 注册
- ✅ max_turns: 50, max_tokens: 4096

---

## Open Questions

1. ~~**工具注册架构最终决定**~~ ✅ 已锁定：核心工具静态编译

2. **输出限制默认值**
   - 50000 字符？
   - 其他值？

---

## Success Criteria (from ROADMAP)

- [x] Tool dispatch registry (已由 rig-core 提供)
- [ ] File R/W/E tools (Read, Write, Edit)
- [ ] Glob/Grep search tools
- [x] Multi-LLM support (已在 Phase 1 实现)
- [ ] Basic sandbox (高风险命令黑名单)
- [ ] ~~MCP protocol support~~ (延后到 Phase 3+)

---

## Deferred

- 完整沙箱系统（Phase 3）
- Human-in-the-Loop 确认 UI（Phase 3）
- MCP 协议支持（Phase 3+）
- Skills 动态加载（Phase 3+）
- 动态工具扩展架构（Phase 3+）

---

*Context captured: 2026-03-23*
*Phase: 02-s02-tool-use*
