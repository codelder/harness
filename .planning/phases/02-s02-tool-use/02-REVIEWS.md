---
phase: 02
reviewers: [codex]
reviewed_at: "2026-03-23T09:15:00Z"
plans_reviewed:
  - 02-01-PLAN.md
  - 02-02-PLAN.md
  - 02-03-PLAN.md
---

# Cross-AI Plan Review — Phase 2

## Codex Review

### Plan 02-01: File Tools

#### Summary
This plan covers the core file-manipulation surface needed for Phase 2 and is appropriately scoped, but it is underspecified around atomicity, path safety, encoding behavior, and integration with the sandbox model. As written, it likely produces working tools, but not yet a defensible implementation for a long-running agent harness where file correctness and predictable failure modes matter.

#### Strengths
- Targets the minimum useful file tool set: read, write, and precise edit.
- `EditTool` explicitly defines two important failure cases: no match and multiple matches.
- Read truncation requirement is called out up front instead of being left implicit.
- Scope is narrow enough to fit the phase without drifting into full patch/diff editing.

#### Concerns
- **HIGH**: "Write files atomically" is in the phase success criteria, but the plan only says "create/overwrite files." Atomic write semantics are not specified.
- **HIGH**: No path validation or sandbox boundary checks are mentioned. A file tool without path constraints weakens the stated sandbox-first posture.
- **MEDIUM**: `ReadTool` says offset/limit, but the plan does not define behavior for invalid offsets, huge files, non-UTF-8 content, or binary files.
- **MEDIUM**: `WriteTool` does not specify what happens on partial failures when creating parent directories or replacing existing files.
- **MEDIUM**: `EditTool` only supports single string replacement; that is fine for scope, but the plan does not define whether replacement is literal, line-oriented, encoding-aware, or newline-preserving.
- **LOW**: No mention of tests for corner cases such as empty files, symlinks, permissions errors, or concurrent modifications.

#### Suggestions
- Specify atomic write behavior explicitly: write to a temp file in the same directory, `fsync` if needed, then rename.
- Define a shared path-policy layer for all file tools: canonicalization, allowed roots, symlink handling, and rejection behavior.
- Document binary/non-UTF-8 handling for `ReadTool` and `EditTool` rather than leaving it to implementation drift.
- Add explicit error taxonomy for file tools so the agent gets structured, recoverable failures instead of opaque IO errors.
- Add tests for truncation boundaries, nonexistent parents, permission denied, duplicate match replacement, and exact newline preservation.

#### Risk Assessment
**MEDIUM** — The plan is directionally correct, but it currently underspecifies the file-safety and atomicity details that matter most for this phase.

---

### Plan 02-02: Search Tools + Sandbox

#### Summary
This plan usefully groups code search and `BashTool` hardening, but the sandbox portion is much weaker than the rest of the phase requires. The search tools are likely implementable with low complexity, yet the blacklist-based shell safety model is brittle and could create false confidence unless its limitations are made explicit.

#### Strengths
- Combines related developer-facing tools in one wave, which is sensible for implementation flow.
- `GlobTool` and `GrepTool` directly support the phase requirement to search the codebase.
- Truncation is called out for grep output, which is necessary for agent context control.
- Scope avoids premature MCP work and stays aligned to the current phase boundary.

#### Concerns
- **HIGH**: A command blacklist is not a robust sandbox. It is easy to bypass with shell composition, quoting, alternate binaries, env tricks, or indirect execution.
- **HIGH**: The phase requirement says destructive operations require interactive confirmation, but this plan replaces that with blocking some commands. Those are not equivalent controls.
- **MEDIUM**: `GlobTool` does not specify whether it respects ignore rules, hidden files, symlinks, or workspace boundaries.
- **MEDIUM**: `GrepTool` does not define behavior for binary files, large repositories, invalid regexes, or multiline matching.
- **MEDIUM**: Depending on implementation, recursive grep over the full tree may be slow or memory-heavy if results are buffered before truncation.
- **LOW**: Adding "glob and regex dependencies" may be unnecessary if standard ecosystem crates already exist elsewhere in the project; dependency duplication/version drift could appear.

#### Suggestions
- Reframe the `BashTool` change as "basic guardrails" rather than "sandbox," and document that real approval/isolation remains future work.
- Add an explicit confirmation hook or policy interface for destructive shell commands, even if the first version is minimal.
- Define search scope rules: workspace root only, whether `.gitignore` is honored, and how symlinks are treated.
- Stream grep results and stop once output cap is reached rather than collecting full results first.
- Validate regex input and return structured compile errors.
- Consider using fast filesystem/search crates that align with Rust async expectations, but avoid overengineering if repo sizes are modest.

#### Risk Assessment
**HIGH** — The search tools are low-risk, but the blacklist approach materially underdelivers on the safety goal and could be misrepresented as stronger protection than it is.

---

### Plan 02-03: Tool Registration

#### Summary
This is the right final integration step and correctly depends on the implementation waves before it, but it is too thin for a plan whose job is to prove the phase works end-to-end. Registration alone will not validate tool schemas, naming consistency, backend compatibility, or actual agent-loop behavior.

#### Strengths
- Correctly sequenced after tool implementation.
- Keeps integration centralized instead of scattering registrations.
- Includes an explicit full-suite verification step.

#### Concerns
- **MEDIUM**: Registering tools in `src/llm/provider.rs` may be the wrong abstraction boundary if tool availability belongs to agent construction rather than provider selection.
- **MEDIUM**: No explicit check for tool name/schema alignment between handler registration and model-facing tool definitions.
- **MEDIUM**: "Run full test suite and verify integration" is too vague to ensure the actual phase goals are covered.
- **LOW**: If multi-backend support is already complete, touching provider code may create avoidable regression surface in an unrelated area.

#### Suggestions
- Verify whether tool registration belongs in a provider layer or in a higher-level agent builder/module. Provider code should ideally not own tool composition unless architecture already dictates that.
- Add integration tests that exercise the actual dispatch path: model emits tool call -> registry resolves handler -> tool executes -> result returns in expected format.
- Add backend-agnostic tests ensuring all registered tools are exposed consistently regardless of Anthropic/OpenAI/Ollama selection.
- Include negative-path integration tests for blocked shell commands and file tool failures, not only happy-path registration.

#### Risk Assessment
**MEDIUM** — The dependency ordering is sound, but the plan is too shallow to guarantee end-to-end correctness without stronger integration verification.

---

## Consensus Summary

### Overall Assessment

The phase plan is broadly well-scoped and mostly aligned with the Phase 2 goal: give the agent actionable tools without prematurely pulling in deferred systems. The main weakness is that the safety story is currently inconsistent. File tools are close to adequate if atomicity and path-policy details are added, but the `BashTool` blacklist does not satisfy the stated destructive-operation confirmation requirement and should be treated as an interim guardrail, not a sandbox.

### Agreed Strengths
- Clear wave ordering with a sensible separation between implementation and integration.
- Scope discipline is good; deferred MCP work stays deferred.
- The selected tool set maps well to the stated requirements and immediate agent needs.
- Truncation is called out for read/grep output (context control).

### Agreed Concerns (HIGH Priority)
1. **Safety model inconsistency**: File tools, shell tools, and confirmation policy are not unified.
2. **"Atomic" and "interactive confirmation"** appear in goals but are not concretely planned.
3. **Blacklist is not a sandbox**: Easy to bypass with shell composition, quoting, alternate binaries, env tricks.

### Divergent Views
None significant - review focused on implementation gaps rather than architectural disagreements.

### Recommended Actions Before Execution

| Priority | Action | Plan Affected |
|----------|--------|---------------|
| HIGH | Reframe BashTool blacklist as "basic guardrails" not "sandbox" | 02-02 |
| MEDIUM | Add integration test for tool dispatch path | 02-03 |
| MEDIUM | Document what is intentionally deferred (Phase 3) | All |
| LOW | Add edge case tests (empty files, symlinks, permissions) | 02-01 |

---

## Verdict

**⚠️ PROCEED WITH ACKNOWLEDGED GAPS**

The plans are implementable but have documented safety/correctness gaps. Key points:

1. **Blacklist = Guardrails, not Sandbox** - This is explicitly Phase 3 work per CONTEXT.md
2. **Atomic writes** - Not required for Phase 2 MVP, can enhance later
3. **Integration tests** - Should be added during 02-03 execution

**Recommendation:** Execute with current plans, document limitations clearly, enhance in Phase 3.

---

*Review completed: 2026-03-23*
*Reviewer: OpenAI Codex (gpt-5.4)*
