---
name: review-implementation
description: >-
  Report-only implementation review for the f8 compiler workspace.
  Verification gate, spec conformance, review checklist, targeted probing, and decisive verdicts.
  Use after implementation, before spec cleanup, or on demand for direct-execution changes.
---

# Implementation Review Workflow

## Core Rules

- Report-only: Read, run, probe. Never edit tracked files, `docs/specs/drafts/`, or git state.
- Review target: The draft spec in `docs/specs/drafts/` when one exists; otherwise the user request that prompted the change.
- Evidence-bound: Every finding cites `<file:line>`, concrete evidence (code, test output, or probe result), and a concrete fix.
- One finding per defect: Report each defect under the single most specific checklist item.
- Severity & Verdicts:
  - `Blocker` (blocks approval): Soundness defect, hard-rule breach (`AGENTS.md`), compiler/phase invariant breach, unmet or untested acceptance criterion, scope creep, or verification gate failure.
  - `Minor` (non-blocking): Hygiene, memory, or style issue with no correctness impact.
  - `Flag` (non-blocking): Architectural trade-off, ambiguity, or docs impact requiring Lead Orchestrator or user decision.
  - Verdicts: `Changes required` (any `Blocker`), `Approve with flags` (no `Blocker`, 1+ `Flag`), or `Approve` (no `Blocker`, no `Flag`).

## Review Steps

1. Verification Gate:
   - Run the `AGENTS.md` verification suite. Any failure is a `Blocker`; test output feeds step 2.
2. Diff & Spec Conformance:
   - Inspect `git status --porcelain -uall` and `git diff` (uncommitted changes) against the review target and `AGENTS.md`.
   - Map every acceptance criterion to its implementing code and covering test. Missing tests, weak assertions (tests that pass even if logic breaks), or unspecified scope are `Blocker`.
3. Checklist Audit:
   - Audit all changed files against the checklist below.
4. Targeted Probes (when warranted):
   - When the diff introduces complex edge cases not covered by tests, run a temporary script outside the worktree (e.g., agent's scratch directory). Never leave scratch files in the repository.

## Review Checklist

### 1. Compiler Invariants & Architecture

- [ ] Single source of truth (`Blocker`): Domain models, identifiers, coordinate mappings, and errors have one authoritative definition; no duplicated domain logic across passes. Incidental code similarity is not a finding.
- [ ] Clean phase boundaries (`Blocker`): Passes consume only upstream outputs; higher-level domain logic stays out of low-level IR primitives; no filesystem I/O in `f8-core`.
- [ ] Canonical core IR (`Blocker`): One representation per concept; no implicit fallbacks or syntactic sugar in core; ergonomics confined to outer adapters.
- [ ] Typed semantics (`Blocker`): Explicit enums, interned IDs, and typed AST/IR fields; no string inspection to infer semantics outside the lexer/parser.
- [ ] Pure data flow (`Blocker`): Explicit inputs and outputs; no hidden or shared mutable state.
- [ ] Root-cause correctness (`Blocker`): Defects fixed at origin (parser, IR, schema); no downstream band-aids, silent workarounds, or test-specific branches.
- [ ] Deterministic output (`Blocker`): No time, randomness, or hash-order dependence in evaluation, layout, emitted formulas, or bytes; `IndexMap` / `IndexSet` wherever order matters.
- [ ] Excel parity (`Blocker`): New or changed functions and operators match Excel semantics (type coercion, error propagation, comparison ranking); tests encode Excel-verified expectations.

### 2. Hard Rules & Hygiene

- [ ] No unapproved suppressions (`Blocker`): New `#[allow(...)]`, `# type: ignore`, `# pyright: ignore`, or `# noqa` lacks user approval or an inline reason.
- [ ] Exhaustive matching (`Blocker`): No wildcard arms (`_ =>`) on AST, IR, or domain enums in compiler passes or PyO3 bridges.
- [ ] No shims (`Blocker`): No `#[deprecated]`, transition wrappers, or old-name re-exports in `f8-core` or `f8-python`. Public `f8.__all__` deprecations only when the user or spec requires them.
- [ ] Bool toggles (`Blocker` in public or IR signatures, `Minor` in private helpers): 2-variant enums instead of `bool` behavior flags.
- [ ] Minimal surface (`Minor`): No commented-out tombstones, unused functions/imports, or single-use trait/generic abstractions.

### 3. Rust Standards (2024 Edition)

- [ ] Panic-free production paths (`Blocker`): No `.unwrap()` / `.expect()` outside tests; no unchecked indexing where bounds are not guaranteed.
- [ ] Structured errors (`Blocker`): `thiserror` diagnostics propagated via `?`; no stringly-typed errors or context swallowed via `.ok()`.
- [ ] Concrete internals (`Minor`): No `impl Into` / `impl AsRef` in internal pass signatures; generic conversions only at public facades.
- [ ] Memory & style (`Minor`): Borrowed slices (`&str`, `&[T]`) or `Cow` over unneeded `.clone()` / `.to_string()`; guard clauses over deep nesting; import order std, external, crate-local.

### 4. Python Standards (3.12+, when the diff touches `crates/f8-python` or `python/`)

- [ ] Strict typing (`Blocker`): `pyright` strict clean, explicit `-> None`, `TypedDict` over `dict[str, Any]`, no bare `Any`.
- [ ] Native stub parity (`Blocker`): `crates/f8-python` exports and exceptions match `python/f8/_core.pyi`; `python/f8/py.typed` marker present.
- [ ] Exception hygiene (`Blocker`): No bare `except:` or swallowed `Exception`; errors inherit from `F8Error` with structured attributes; native errors wrapped via `raise ... from err`.
- [ ] GIL release (`Blocker`): CPU-bound PyO3 calls release the GIL (`py.allow_threads`); bulk data uses buffer protocols (`memoryview` / `bytes`).
- [ ] Immutability (`Minor`): Frozen dataclasses for value objects.

### 5. Docs Impact

- [ ] Reference docs (`Flag`): Change alters architecture, DSL, or function behavior; name the section of `docs/architecture.md` or `python/f8/skills/f8-modeling/` to update in the fold step.

## Report Template

```markdown
# Implementation Review: <Spec Title | Change Summary>

## Verdict: `<Changes required | Approve with flags | Approve>`

<One-line rationale.>

## Verification & Spec Conformance

- Verification gate: `<Pass | Fail (with failing command & output)>`
- Acceptance criteria:
  - `<Scenario 1>`: `met` (`<impl_file:line>`, `<test_file:line>`)
  - `<Scenario 2>`: `unmet / untested` (<reason>)

## Findings

- [Blocker | Minor | Flag] `<file:line>` — <concise issue & evidence>. Fix: <actionable remediation>.
```
