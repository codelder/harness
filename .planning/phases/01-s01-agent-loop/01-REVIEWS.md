---
phase: 1
reviewers: [codex]
reviewed_at: 2026-03-22T14:07:24Z
plans_reviewed:
  - 01-01-PLAN.md
  - 01-02-PLAN.md
  - 01-03-PLAN.md
  - 01-04-PLAN.md
---

# Plan Review - Phase 1

## Codex Review

### Summary

The Phase 1 plan set has a good overall wave structure and a clear separation between foundation, provider setup, agent loop, and CLI work. The main risk is not sequencing but plan correctness: several interfaces change shape across plans, one required phase deliverable is never actually planned, and the verification strategy does not test the most failure-prone parts of the design. As written, these plans were likely to force mid-execution replanning.

### Strengths

- The four-wave structure is sensible and keeps foundational work ahead of downstream implementation.
- The plans consistently tie tasks back to roadmap requirements and expected artifacts.
- Error handling, retry behavior, and observability are treated as first-class concerns instead of afterthoughts.
- The review context and research capture enough background for an executor to understand the intended architecture.

### Concerns

- HIGH: Phase 1 requires Bash tool execution, but none of the four plans allocates work for a Bash tool or tool registration. The phase contract explicitly includes Bash tool support in [.planning/ROADMAP.md](../../ROADMAP.md), lines 57-63, and [.planning/phases/01-s01-agent-loop/01-CONTEXT.md](./01-CONTEXT.md), lines 12-18 and 40-46. The plan objectives in [01-03-PLAN.md](./01-03-PLAN.md) and [01-04-PLAN.md](./01-04-PLAN.md) only cover provider access, agent loop, and CLI, so Phase 1 can complete "successfully" while still missing a required user-visible capability.
- HIGH: The provider API is internally inconsistent across plans. [01-02-PLAN.md](./01-02-PLAN.md), lines 212-226 defines `LlmProvider = Box<dyn Chat>` and `create_provider(provider_type, model)`, but its own acceptance criteria in lines 286-288 require `create_provider(provider_type, model, base_url)` and enum dispatch instead of `Box<dyn Chat>`. [01-04-PLAN.md](./01-04-PLAN.md), lines 353-355 then calls the two-argument version in `Session::run`, while lines 461-464 and 552 require `base_url` plumbing. This is a design contract mismatch, not just an implementation detail.
- HIGH: The Ollama path is specified as an API-key-backed OpenRouter client instead of a local provider flow. [01-02-PLAN.md](./01-02-PLAN.md), lines 238-242 maps Ollama through `rig::providers::openrouter::Client::from_env()` and converts failures into `MissingApiKey("ollama: ...")`, while the phase context only requires API keys for Anthropic and OpenAI and treats Ollama as a local backend. That would misconfigure one of the three advertised providers and push the implementation toward the wrong operational model.
- MEDIUM: The module naming instructions for the agent loop are self-contradictory. [01-03-PLAN.md](./01-03-PLAN.md), lines 187-193 declares `mod loop_rs;`, but line 272 says the file "MUST be named `loop.rs` (not `loop_rs.rs`)". Without an explicit `#[path = "loop.rs"]` attribute, those instructions cannot both be true.
- MEDIUM: The test plan does not verify the most important Phase 1 behavior. [01-03-PLAN.md](./01-03-PLAN.md), lines 483-488 says Task 3 should test `EndTurn`, `ToolUse`, `MaxTokens`, and streaming behavior, but Task 4 in lines 637-689 explicitly avoids mocking providers and only compiles against the `agent_loop` signature. That leaves stop-reason branching, streaming semantics, and message-history mutation effectively untested despite being core phase claims.
- MEDIUM: The streaming implementation requires a dependency change that is not actually planned. [01-03-PLAN.md](./01-03-PLAN.md), lines 492 and 604 require `futures::StreamExt` and even note "Add futures crate dependency if not already present", but the task's writable files only include `src/agent/loop.rs` and the Phase 1 dependency plan in [01-01-PLAN.md](./01-01-PLAN.md), lines 119-133 does not add `futures`. That creates a hidden cross-plan edit that the executor is not formally authorized to make.

### Suggestions

- Add a dedicated Phase 1 task for Bash tool definition, registration, and the minimal `tool_use -> execute -> append result` loop, or explicitly move that roadmap criterion out of Phase 1.
- Normalize the provider contract once, then propagate it everywhere: choose either enum dispatch or trait objects, and decide whether `base_url` is part of `create_provider` before execution begins.
- Rewrite the Ollama section around a local endpoint configuration instead of an API-key error path.
- Fix the agent loop module naming so the file path, module declaration, and acceptance criteria all agree.
- Introduce a mock or fake provider abstraction for Phase 1 tests so stop reasons and streamed deltas can be tested deterministically.
- If streaming needs `futures`, add that dependency in the plan explicitly; otherwise switch the implementation and examples to `tokio_stream` or the exact trait already in the dependency set.

### Risk Assessment

HIGH. The overall architecture is viable, but the current plans contain enough contract drift that a straightforward executor would likely either implement the wrong interfaces or need to improvise around contradictory instructions. The largest risk is silent scope miss: the plans can all "pass" while still failing the Bash-tool requirement that defines the phase boundary.

---

## Consensus Summary

Only one reviewer was used for this pass, so there is no true cross-reviewer consensus. The dominant concerns from this review are:

### Agreed Strengths

- Phase decomposition and wave ordering are appropriate.
- Requirements, artifacts, and verification intent are documented clearly.

### Agreed Concerns

- The phase scope includes Bash tool execution, but the plan set does not schedule it.
- Provider interfaces drift across plans and would force rework.
- Core stop-reason and streaming behavior is under-tested.

### Divergent Views

- Not applicable for a single-reviewer pass.
