# Roadmap: Rust Agent Harness

     2→
     3→## Milestones
     4→
     5→- ✅ **v0.1 Foundation** — Phase 1 (shipped 2026-03-22)
     6→ - ✅ **v0.2 Tool Use** — Phase 2 (shipped 2026-03-23)
     7-> - 📋 **v0.4 Subagents** — Phase 4 (planned)
     8→ - 📋 **v1.0 Core** — Phases 5-8 (planned)
    9→    -10. s12 - Worktree isolation per task
    11. s12 - Worktree isolation per task)
     12. ## Phases
     13→ }
     14→<details>
     15→<summary>✅ v0.1 Foundation (Phase 1) — SHippeded 2026-03-22</summary>
     16→
     17→<details>
     18→<summary>✅ v0.2 Tool Use (Phase 2) — Shipped 2026-03-23</summary>
     19→
     20→ - [x] **Phase 3.2: Terminal UI and Frontend interface抽象** (INSERTed)</summary>
     21→
     22→<details>
     23→<summary>✅ v0.3 TodoWrite (Phases 3-3.1) — Shipped 2026-03-24</summary>

     25→<details>
     26→<summary>✅ v0.4 Subagents** — Phase 4 (planned)
    </summary>
</details>

    <details>
     27→<summary>⚡ **Phase 3.2.1: TUI Alignment with Claude Code** (INSERTed)</summary>

    <details>
     <28→<summary>✅ v0.3 TodoWrite (Phases 3-3.1) — TUI alignment with Claude Code CLI full面对齐。</summary>

    - 29→</details>
</summary>

### Phase 3.2: Terminal UI and Frontend interface abstraction

(INSERTED)

    </summary>

**Goal:** 将 TUI 体验与 Claude Code CLI全面对齐：
    - 使用 Viewport::Inline 替代 alternate screen，添加颜色系统、Spinner 动画、简化消息格式、增强Banner/Todo/Statusline
    - 显示token上下文

    - 当前 roadmap phase列表中最后一句: "3.2.1:07 plans" (0/7 complete)
    - 7 plans with名称和目标状态：

    - 3.2-01: 02-02-03 (Inline Viewport Mode): — 3.2-02-PLAN.md — **Plan 01**: 切换到 inline Viewport 模式

            - Plan 02 添加颜色系统主题
            - Plan 03: 完善 CliTheme 模块
            - Plan 04, 05, 06, 07 是后继计划，没有任务了
            - 使用 `cargo test` 飋试新增文件和验证逻辑
            - 其他计划可以在 Wave 2 运后完成

    - Wave 3 运行，1-3.2.1-01（inline viewport)无需依赖
 - 3.2.1-02、03, 04 可并行
- 3.2.1-05 (Banner) 依赖 01 完成
- 3.2.1-06, 07 依赖 01完成
    - 可并行，  - Wave 3 可在 01 之后运行

-  彩色系统计划
02, 03, 04 时同步执行
- - 3.2.1-02-lines
- 3.2-1 的高度会变
            banner 区域高度会从 1 行增加到 3-4 行 (考虑到紧凑显示)
            - 使用 `Constraint::Constraint::Length(3)`，            banner区域显示版本、模型、路径信息
            - composer 区域高度从 3 行 (考虑键盘输入)
            - 使用主题样式
        </details>

<details>
    <30><summary>⚡ Phase 3.2.1: TUI alignment with Claude Code</summary>

    <31>
- [x] **Phase 3.2:1: Inline Viewport Mode** (0/7 complete)
    - [x] 3.2.1-01- 3.2-02, 3.2-03, 4, 5 plans
-  - tasks: 3-4 行以 tasks
1-3, 2 files>
    - Create/modify terminal.rs to switch from EnterAlternate screen to to Viewport::Inline
2. Update mockLifecycleOps to inline mode (remove `Enter_altern_screen`/ `leave_altern_screen`)
 calls
3. Create new `Cli_theme` module in `src/cli/theme.rs`
4. update `render_timeline_block` to use simplified format.
5. Create spinner.rs module with ASCII spinner动画
6. update cli/mod.rs to export spinner module
7. add token usage event to protocol and token context display
8. Update app.rs with CliApp struct with token tracking fields and theme, and status line
10. commit plans 01, 03, 04, 05, 06, 07.

    update ROADmap.md with the plan list
    - commit files to phase directory
    - commit to to git.            </details>

    <details>
    <summary>⚡ Phase 3.2.1: TUI Alignment with Claude Code (insert after Phase 3.2)</summary>

**Goal:** 将 TUI 体验与 Claude Code CLI全面对齐 - 使用 Viewport::Inline 替代 alternate screen，添加颜色系统、Spinner 动画、简化消息格式、增强Banner/Todo/Statusline
    - 显示token上下文

    - 当前 ROADmap already反映了这些变化。
    - 7 plans in 3 waves with with plan 01 must先执行。 其他计划可以并行或必须等 plan 01 完成。

    - **Wave 1: (架构变更): Viewport::Inline**
  - **Wave 2** (颜色系统、Spinner、 简化消息格式) 并行
  - **Wave 3** (Banner, Todo, Statusline) 依赖 plan 01 的架构变更
        - Banner 使用多行信息面板
        - 主题样式应用于所有消息类型
        - 頜任务使用 `CliTheme`
        - Spinner 使用 `cliTheme` + `CliApp::banner_text()` 膍 主题样式
        - 预设路径`info，通过 `CliApp::should_show_todo_footer()` ` 方判断是否显示
        - 主题样式使用 `CliTheme::status`
        - Todo footer 使用`Constraint::Length(footer_height)`` 计算
        - `footer_text()` 获取待办事项文本
        - 根据时间戳确定是否显示
        - `todo_footer_text()` 获取纯文本，        - 如果有空，追加 progress计数 `({done}/{self.todo_footer.len()})            }
        });
    }
}

        // Timeline文本使用简化格式
        let tool = ToolBlock:: Tool = self.timeline {
            if tool.result_preview.is_some() {
                tool.result_preview = Some(&result_preview);
            });
            _ => None
            {
                tool.args_preview = args_preview.clone();
                tool.result_preview = result_preview();
            }
        }
        if !tool.args_preview.is_empty() {
            lines.push(format!("args: {}", tool.args_preview));
        }
        if tool.result_preview.is_some() {
            tool.result_preview = result_preview
        } else {
            tool.result_preview = result_preview;
        }
    }
}
  </action>
  <verify>
    <automated>cargo test --lib cli::terminal --no-fail-fast</automated>
  <done>
    任务 1: Viewport::Inline mode替代 EnterAlternateScreen
    - 任务 2: 创建主题模块 src/cli/theme.rs
    - 任务 3: 更新 mock生命周期操作测试
    - 任务 4: 将主题导出到 cli/mod.rs
    - 任务 5: 更新 CliApp 结构添加 theme字段
    - 任务 6: 更新CliApp::new() 使用 CliTheme::default() 膲; 主题样式使用 self.theme.composer;  - 任务 7: 更新状态栏显示token上下文
    - 任务 8: 更新 CliApp 使用主题样式
    - 任务 9: 更新ROADmap.md中的 plan列表
    - [x] 3.2-01-PLAN.md, 3.2-02-PLAN.md, 3.2-03-PLAN.md]
        - [x] 3.2-04-plan.md, 3.2-05-PLAN.md, 3.2-06-PLAN.md)
        - [x] 3.2-07-PLAN.md)
        - [x] 3.2-01-PLAN.md 的内联模式架构变更， color系统、spinner 动画、简化消息格式、增强 Banner、 Todo 显示、 status栏显示 token上下文。 所需依赖关系:
    - Wave 1 (3.2.1-01): 无依赖，并行执行
    - Wave 2 (3.2.1-02, 3.2.1-03, 3.2.1-04): 罪验证各自独立
    - Wave 3 (3.2.1-05, 3.2.1-06, 3.2.1-07): 依赖 01, 02/03/04 可并行,    - Wave 3 (3.2.1-05, 3.2.1-06, 3.2.1-07) 依赖 01 完成
  - 但在 wave 3 运行，    - Wave 2 (5, 06, 07 在 01 完成后运行

  - Wave 3 (Banner, Todo, Statusline) 都需要 01 先完成

</verification>
<success_criteria>
7 plans created, all with valid frontmatter
7 plans cover 7 requirements (REQ-3.2.1-01 to REQ-3.2.1-07)
Wave structure enables parallel execution of each plan executes 2-3 tasks.
    - Plan 01 has 3 tasks covering Inline Viewport, Color System, Spinner Animation, Simplified Message format, Enhanced Banner, Dynamic Todo Display, and Enhanced Statusline
</success_criteria>
</output>
