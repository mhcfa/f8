---
name: review-implementation
description: >-
  Report-only implementation review for code changes and draft specs.
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
  - `Blocker` (blocks approval): Soundness defect, hard-rule breach (`AGENTS.md`), contract/phase invariant breach, unmet or untested acceptance criterion, scope creep, or verification gate failure.
  - `Minor` (non-blocking): Hygiene, memory, or style issue with no correctness impact.
  - `Flag` (non-blocking): Architectural trade-off, ambiguity, or docs impact requiring Lead Orchestrator or user decision.
  - Verdicts: `Changes required` (any `Blocker`), `Approve with flags` (no `Blocker`, 1+ `Flag`), or `Approve` (no `Blocker`, no `Flag`).

## Review Steps

1. Verification Gate:
   - Run the `AGENTS.md` verification suite. Any failure is a `Blocker`; test output feeds step 2.
2. Diff & Spec Conformance:
   - Inspect `git status --porcelain -uall` and `git diff` (uncommitted changes) against the review target and `AGENTS.md`.
   - Map every acceptance criterion to its implementing code and covering test (or `met (inspection)` for documentation, template prose, and skill criteria). Missing tests for executable code, weak assertions (tests that pass even if logic breaks), or unspecified scope are `Blocker`.
3. Checklist Audit:
   - Audit all changed files against the applicable sections of the checklist below, plus `references/repo-checks.md` when present in this skill directory.
4. Targeted Probes (when warranted):
   - When the diff introduces complex edge cases not covered by tests, run a temporary script outside the worktree (e.g., agent's scratch directory). Never leave scratch files in the repository.

## Review Checklist

### 1. Architecture & Invariants

- [ ] Single source of truth (`Blocker`): Domain models, identifiers, schemas, coordinate mappings, and errors have one authoritative definition; no duplicated domain logic across modules/passes. Incidental code similarity is not a finding.
- [ ] Clean boundaries (`Blocker`): Passes and modules consume only upstream outputs; higher-level presets stay out of low-level primitives; no unexpected I/O in pure core modules.
- [ ] Canonical contracts & IR (`Blocker`): One representation per concept; no implicit fallbacks, untyped dicts, or syntactic sugar in core; ergonomics confined to outer adapters.
- [ ] Typed semantics (`Blocker`): Explicit enums, `Literal` types, interned IDs, newtypes, and typed structs/models; no string inspection to infer semantics outside parsing/ingest.
- [ ] Pure data flow (`Blocker`): Explicit inputs and outputs; no hidden or shared mutable state.
- [ ] Root-cause correctness (`Blocker`): Defects fixed at origin (parser, ingest, IR, schema, solver); no downstream band-aids, silent workarounds, or test-specific branches.
- [ ] Deterministic output (`Blocker`): No wall-clock time, randomness, or hash-order dependence in evaluation, layout, emitted artifacts, or bytes; `IndexMap` / `IndexSet` (or deterministic ordering) wherever order matters.
- [ ] Repository-specific invariants (`Blocker`): Satisfies all domain invariants in `AGENTS.md` and `references/repo-checks.md` (when present).

### 2. Hard Rules & Hygiene

- [ ] No unapproved suppressions (`Blocker`): New `#[allow(...)]`, `# type: ignore`, `# pyright: ignore`, `# noqa`, `eslint-disable`, `oxlint-disable`, `@ts-ignore`, or `@ts-expect-error` lacks user approval or an inline reason.
- [ ] Exhaustive matching (`Blocker`): No wildcard arms (`_ =>`) on AST, IR, or domain enums in compiler passes, emitters, or PyO3 bridges.
- [ ] No shims (`Blocker`): No `#[deprecated]`, `@deprecated`, transition wrappers, old-name re-exports, or placeholder stubs. Public `__all__` deprecations only when the user or spec requires them.
- [ ] Bool toggles (`Blocker` in public or IR signatures, `Minor` in private helpers): 2-variant enums/unions instead of `bool` behavior flags.
- [ ] Minimal surface (`Minor`): No commented-out tombstones, unused functions/imports, or single-use trait/generic abstractions.

### 3. Rust Standards (2024 Edition, when the diff touches Rust crates)

- [ ] Panic-free production paths (`Blocker`): No `.unwrap()` / `.expect()` outside tests; no unchecked indexing where bounds are not guaranteed.
- [ ] Structured errors (`Blocker`): `thiserror` diagnostics propagated via `?`; no stringly-typed errors or context swallowed via `.ok()`.
- [ ] Concrete internals (`Minor`): No `impl Into` / `impl AsRef` in internal pass signatures; generic conversions only at public facades.
- [ ] Memory & style (`Minor`): Borrowed slices (`&str`, `&[T]`) or `Cow` over unneeded `.clone()` / `.to_string()`; guard clauses over deep nesting; import order std, external, crate-local.

### 4. Python Standards (3.12+, when the diff touches Python or PyO3 bindings)

- [ ] Strict typing (`Blocker`): `pyright` strict clean, explicit `-> None`, `TypedDict` / frozen dataclasses / Pydantic models over `dict[str, Any]`, no bare `Any`.
- [ ] Native stub parity (`Blocker`, PyO3 repos): PyO3 crate exports and exceptions match `_core.pyi`; `py.typed` marker present.
- [ ] Exception hygiene (`Blocker`): No bare `except:` or swallowed `Exception`; structured error attributes; causes preserved via `raise ... from err`.
- [ ] GIL release (`Blocker`, PyO3 repos): CPU-bound PyO3 calls release the GIL (`py.allow_threads` / `py.detach`); bulk data uses buffer protocols (`memoryview` / `bytes`).
- [ ] Immutability (`Minor`): No mutable default arguments; frozen dataclasses or immutable models for value objects.

### 5. TypeScript & React Standards (when the diff touches TypeScript or React packages)

- [ ] Strict module & promise hygiene (`Blocker`): `import type` for type-only imports (`verbatimModuleSyntax`), zero unused locals/parameters, and zero floating or misused promises (`oxlint` type-aware).
- [ ] Boundary validation (`Blocker`): External inputs, search params, and network payloads parsed via `zod` schemas rather than unverified type assertions (`as`) or `any`.
- [ ] Generated artifacts (`Blocker`): Generated files (e.g., `routeTree.gen.ts`) regenerated via workspace scripts, never manually edited.
- [ ] React 19 & styling idioms (`Minor`): `ref` passed directly as a prop (`React.RefAttributes<T>`) without `React.forwardRef`; variants composed with `cva`, classes merged with `cn`, and semantic theme tokens used over hardcoded values.

### 6. Docs Impact

- [ ] Reference docs (`Flag`): Change alters architecture, schemas, DSL, or public behavior; name the section of `docs/architecture.md` or packaged skills/templates to update in the fold step.

## Report Template

```markdown
# Implementation Review: <Spec Title | Change Summary>

## Verdict: `<Changes required | Approve with flags | Approve>`

<One-line rationale.>

## Verification & Spec Conformance

- Verification gate: `<Pass | Fail (with failing command & output)>`
- Acceptance criteria:
  - `<Scenario 1>`: `met` (`<impl_file:line>`, `<test_file:line | inspection>`)
  - `<Scenario 2>`: `unmet / untested` (<reason>)

## Findings

- [Blocker | Minor | Flag] `<file:line>` — <concise issue & evidence>. Fix: <actionable remediation>.
```
