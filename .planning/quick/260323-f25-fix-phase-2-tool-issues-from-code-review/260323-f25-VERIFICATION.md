---
phase: quick-260323-f25
verified: 2026-03-23T03:15:00Z
status: gaps_found
score: 0/4 must-haves verified
gaps:
  - truth: "BashTool timeout parameter is enforced via tokio::time::timeout"
    status: failed
    reason: "Code uses std::process::Command (synchronous) instead of tokio::process::Command with timeout wrapper"
    artifacts:
      - path: "src/tools/bash.rs"
        issue: "Line 5: uses std::process::Command; Lines 93-97: blocking .output() call without timeout wrapper; timeout parameter declared but never used"
    missing:
      - "Change import from std::process::Command to tokio::process::Command"
      - "Wrap cmd.output() with tokio::time::timeout(Duration::from_secs(args.timeout), ...)"
      - "Handle timeout case returning BashError::Timeout"

  - truth: "GlobTool surfaces GlobError instead of silently discarding it"
    status: failed
    reason: "Code uses filter_map(|entry| entry.ok()) which silently drops all GlobError values"
    artifacts:
      - path: "src/tools/glob.rs"
        issue: "Lines 57-60: .filter_map(|entry| entry.ok()) discards errors without surfacing them"
    missing:
      - "Replace filter_map with .map(|entry| entry.map(|p| p.display().to_string()))"
      - "Use collect::<Result<Vec<_>, _>>()? to propagate first error"

  - truth: "GrepTool documentation accurately reflects single-file-only behavior"
    status: failed
    reason: "Struct field doc still says 'File or directory path' which is misleading since implementation only reads single file"
    artifacts:
      - path: "src/tools/grep.rs"
        issue: "Line 14: comment says 'File or directory path to search' but implementation only handles single file"
    missing:
      - "Update line 14 comment to: 'Single file path to search (defaults to \".\", reads as file)'"

  - truth: "Dead error variants are removed from ReadError and BashError"
    status: failed
    reason: "Both dead variants still exist in the code"
    artifacts:
      - path: "src/tools/bash.rs"
        issue: "Lines 33-34: BashError::InvalidUtf8 variant exists but is never constructed (code uses from_utf8_lossy)"
      - path: "src/tools/read.rs"
        issue: "Lines 27-28: ReadError::InvalidPath variant exists but is never constructed"
    missing:
      - "Remove BashError::InvalidUtf8 variant from src/tools/bash.rs"
      - "Remove ReadError::InvalidPath variant from src/tools/read.rs"
---

# Quick Task 260323-f25: Fix Phase 2 Tool Issues from Code Review Verification Report

**Phase Goal:** Fix Phase 2 tool issues from code review
**Verified:** 2026-03-23T03:15:00Z
**Status:** gaps_found
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| #   | Truth | Status | Evidence |
| --- | ----- | ------ | -------- |
| 1 | BashTool timeout parameter is enforced via tokio::time::timeout | FAILED | Uses std::process::Command with blocking .output(), no timeout wrapper found |
| 2 | GlobTool surfaces GlobError instead of silently discarding it | FAILED | filter_map(|entry| entry.ok()) silently drops errors at line 58 |
| 3 | GrepTool documentation accurately reflects single-file-only behavior | FAILED | Line 14 still says "File or directory path to search" |
| 4 | Dead error variants are removed from ReadError and BashError | FAILED | InvalidUtf8 (bash.rs:34) and InvalidPath (read.rs:28) still exist |

**Score:** 0/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| src/tools/bash.rs | Async execution with timeout | STUB | Uses sync std::process::Command, timeout param unused |
| src/tools/glob.rs | Error-propagating glob matching | STUB | Silently discards GlobError via filter_map |
| src/tools/grep.rs | Accurate single-file docs | STUB | Docs claim "file or directory" but impl is single-file only |
| src/tools/read.rs | No dead error variant | STUB | InvalidPath variant exists but never constructed |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| src/tools/bash.rs | tokio::time::timeout | async spawn with timeout wrapper | NOT_WIRED | Pattern not found - no timeout wrapper exists |
| src/tools/glob.rs | GlobError | error propagation via ? | NOT_WIRED | Uses filter_map which discards errors |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| src/tools/bash.rs | 14 | Unused parameter `timeout` | Blocker | API contract violated - timeout declared but not enforced |
| src/tools/bash.rs | 5,93-97 | Blocking I/O in async context | Blocker | std::process::Command blocks tokio executor |
| src/tools/glob.rs | 58 | Silent error discarding | Blocker | Users get incomplete results without knowing |
| src/tools/grep.rs | 14 | API/docs mismatch | Warning | Misleading documentation |
| src/tools/bash.rs | 33-34 | Dead code (InvalidUtf8) | Info | Unused variant |
| src/tools/read.rs | 27-28 | Dead code (InvalidPath) | Info | Unused variant |

### Critical Finding: Fix Commits Not on Current Branch

The SUMMARY.md claims fixes were committed (69ece8f, 38f61eb, c48dd01), but investigation reveals:

- These commits exist on branch `worktree-agent-adea8a97`
- They are **NOT** on the current branch `gsd/v0.2-tool-use`
- The SUMMARY was written as if fixes were merged, but they were never integrated

This is a workflow gap - the task was "completed" in a worktree but the changes were never merged to the target branch.

### Human Verification Required

None - all issues are programmatically verifiable.

### Gaps Summary

**0 of 4 must-haves verified.** All four code quality fixes from the plan are missing from the actual codebase:

1. **BashTool timeout** - Still uses blocking std::process::Command, timeout parameter is ignored
2. **GlobTool error propagation** - Still silently discards errors via filter_map
3. **GrepTool docs** - Still claims "file or directory" when only single-file is supported
4. **Dead error variants** - Both InvalidUtf8 and InvalidPath still exist

The root cause appears to be a workflow issue: fixes were implemented on a worktree branch but never merged to the target branch. The SUMMARY was written prematurely.

---

_Verified: 2026-03-23T03:15:00Z_
_Verifier: Claude (gsd-verifier)_
