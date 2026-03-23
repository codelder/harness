---
phase: 02
reviewers: [codex]
reviewed_at: "2026-03-23T10:30:00Z"
plans_reviewed:
  - 02-01-PLAN.md
  - 02-02-PLAN.md
  - 02-03-PLAN.md
---

# Cross-AI Plan Review — Phase 2

## Codex Review (Updated)

*Review completed: 2026-03-23 10:30*
*Reviewer: OpenAI Codex (gpt-5.4)*

### Plan 02-01: File Tools

#### Summary
The plan is directionally sound and aligned with the phase goals, especially the choice to keep the API small and use `tokio::fs`. The main gap is that "atomic file operations" is listed as a requirement, but the current write/edit descriptions do not yet define an actually atomic update strategy, failure semantics, or concurrency behavior.

#### Strengths
- Small tool surface area that matches the stated phase scope.
- `ReadTool` with `offset` and `limit` is practical for large files and agent iteration.
- `EditTool` limited to single replacement reduces ambiguity and keeps behavior predictable.
- `tokio::fs` is the correct async choice for this project.

#### Concerns
- **HIGH**: "Atomic" writes are underspecified. A plain `tokio::fs::write` is not atomic for updates.
- **HIGH**: `EditTool` needs explicit behavior when `old_string` matches zero times or multiple times. Silent ambiguity will cause agent failures and bad edits.
- **MEDIUM**: Character-based truncation can break UTF-8 boundaries if implemented naïvely over bytes.
- **MEDIUM**: Auto-creating parent directories is convenient, but without policy checks it expands write scope in ways the model may not intend.
- **MEDIUM**: No path validation is mentioned. Relative traversal (`../`) and symlink handling can undermine sandbox expectations.
- **LOW**: Large file reads with offset/limit need a clear unit definition: bytes, chars, or lines.

#### Suggestions
- Define atomic write/edit as: write to temp file in same directory, `fsync` if needed, then rename.
- Make `EditTool` return structured outcomes: `not_found`, `multiple_matches`, `success`.
- Define read slicing in bytes or lines, then implement safely and document it.
- Normalize and validate paths before operations; decide whether symlinks are allowed.
- Consider optional file size caps in addition to output truncation.
- Return metadata with results: truncated flag, bytes read/written, created flag.

#### Risk Assessment
**MEDIUM** overall. It will work for happy paths, but atomicity and path-safety need tightening before implementation.

---

### Plan 02-02: Search + Sandbox

#### Summary
This plan covers the required functionality, but the sandbox approach is the weakest part of the phase. `GlobTool` and `GrepTool` are straightforward, but a blacklist-only `BashTool` is fragile and easy to bypass unless the scope is explicitly limited and the threat model is kept intentionally narrow.

#### Strengths
- Clear mapping to requirements: glob, grep, bash, basic sandboxing.
- Using dedicated search tools instead of shelling out for everything is the right design.
- Output truncation keeps tool responses bounded.

#### Concerns
- **HIGH**: Blacklist-based command filtering is not a meaningful security boundary. Shell escaping, chaining, indirection, and alternate binaries can bypass simple pattern checks.
- **HIGH**: `BashTool` threat model is unclear. If destructive operations require interactive confirmation, the plan should define how confirmation is requested and enforced in the tool flow.
- **MEDIUM**: `GlobTool` needs rules for hidden files, symlink traversal, ignored directories, and recursion depth.
- **MEDIUM**: `GrepTool` needs regex engine constraints. Unbounded regex over large trees can become slow or surprising.
- **MEDIUM**: Search tools need explicit file/binary handling; grepping binaries or huge generated artifacts will degrade performance.
- **LOW**: Truncation alone is not enough; result ordering and match caps also matter for usefulness.

#### Suggestions
- Treat the blacklist as a temporary UX guard, not real sandboxing; document that explicitly.
- Add a command execution policy layer:
  - explicit allow/deny result
  - reason code
  - confirmation-required state for risky commands
- Prefer argument-aware parsing over raw string matching where possible, even in this basic phase.
- Bound grep execution with max files, max matches, binary-file skip, and ignored-path defaults (`target`, `.git`, etc.).
- Decide whether search tools follow `.gitignore`; if not, define project-local exclusions explicitly.
- Return structured grep results: file path, line number, line text, truncated flag.

#### Risk Assessment
**HIGH** overall, driven almost entirely by the blacklist-only bash design. Search tools are low-to-medium risk; bash is the main exposure.

---

### Plan 02-03: Integration

#### Summary
The integration plan is necessary but currently too thin. Exporting tools, registering them, and updating the system prompt are all required, but this plan should also define tool schema consistency, registration tests, and failure handling so the agent loop remains reliable once tools are added.

#### Strengths
- Keeps integration concerns separated from tool implementation.
- Static registration via `AgentBuilder.tool()` matches the project decision and reduces runtime complexity.
- Prompt updates acknowledge that tool quality depends partly on model guidance.

#### Concerns
- **MEDIUM**: No mention of integration tests verifying name-to-handler dispatch.
- **MEDIUM**: No schema/versioning discipline is mentioned; inconsistent tool names or parameter shapes will cause hard-to-debug failures.
- **MEDIUM**: System prompt updates can become prompt-plumbing if they over-specify tool routing logic.
- **LOW**: `mod.rs` exports alone do not guarantee clean ownership boundaries or maintainability.

#### Suggestions
- Add an integration test that verifies every registered tool:
  - has a unique name
  - round-trips through dispatch
  - returns structured errors correctly
- Define a consistent tool result envelope now: `ok`, `error`, `truncated`, optional metadata.
- Keep the system prompt minimal: describe tool capabilities and safety expectations, not if/else routing rules.
- Add negative-path tests for unknown tool names, malformed args, and confirmation-required bash commands.

#### Risk Assessment
**MEDIUM** overall. The integration work is not conceptually hard, but thin planning here often causes avoidable reliability issues.

---

## Overall Assessment

The phase plan is mostly aligned with the goals and sensibly scoped for an early tool-use milestone. The strongest parts are the dedicated file/search tools and static registration model. The biggest issue is that two requirements are only partially specified: "atomic file operations" and "destructive operations require interactive confirmation." Right now, file atomicity is not concrete enough, and bash safety is materially underdesigned.

### Cross-cutting Suggestions
- Define structured tool I/O contracts before implementation.
- Specify path normalization and workspace-boundary behavior for all filesystem tools.
- Add integration tests for dispatch, truncation, and failure cases.
- Explicitly document that blacklist-only bash protection is temporary and not a real sandbox.

---

## Consensus Summary

### Agreed Strengths
- Clear wave ordering with a sensible separation between implementation and integration.
- Scope discipline is good; deferred MCP work stays deferred.
- The selected tool set maps well to the stated requirements and immediate agent needs.
- Truncation is called out for read/grep output (context control).
- `tokio::fs` is the correct async choice.
- `EditTool` single replacement pattern is predictable.

### Agreed Concerns (HIGH Priority)
1. **Safety model inconsistency**: File tools, shell tools, and confirmation policy are not unified.
2. **"Atomic" and "interactive confirmation"** appear in goals but are not concretely planned.
3. **Blacklist is not a sandbox**: Easy to bypass with shell composition, quoting, alternate binaries, env tricks.
4. **EditTool ambiguity**: Needs explicit behavior for zero/multiple matches.

### Recommended Actions Before Execution

| Priority | Action | Plan Affected |
|----------|--------|---------------|
| HIGH | Reframe BashTool blacklist as "basic guardrails" not "sandbox" | 02-02 |
| HIGH | Add structured outcomes to EditTool (not_found, multiple_matches, success) | 02-01 |
| MEDIUM | Add integration test for tool dispatch path | 02-03 |
| MEDIUM | Document what is intentionally deferred (Phase 3) | All |
| MEDIUM | Define path normalization and workspace-boundary behavior | 02-01 |
| LOW | Add edge case tests (empty files, symlinks, permissions) | 02-01 |

---

## Verdict

**⚠️ PROCEED WITH ACKNOWLEDGED GAPS**

The plans are implementable but have documented safety/correctness gaps. Key points:

1. **Blacklist = Guardrails, not Sandbox** - This is explicitly Phase 3 work per CONTEXT.md
2. **Atomic writes** - Not required for Phase 2 MVP, can enhance later
3. **Integration tests** - Should be added during 02-03 execution
4. **Structured EditTool outcomes** - Already planned (NotFound, MultipleMatches errors)

**Recommendation:** Execute with current plans, document limitations clearly, enhance in Phase 3.

**Overall Risk Assessment:** **MEDIUM-HIGH** — The phase is implementable, but without tightening atomic write semantics and bash safety/confirmation flow, it is likely to meet the demo goal while leaving correctness and security gaps.

---

*Review completed: 2026-03-23 10:30*
*Reviewer: OpenAI Codex (gpt-5.4)*
