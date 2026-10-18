---
name: review-implementation
description: >-
  Report-only implementation review for the f8 compiler, runtime, and language workspace.
  Verification gate, spec conformance, review checklist, targeted probing, and decisive verdicts.
  Use after implementation, before spec cleanup.
---

# Implementation Review Workflow

## Core Rules

- Report-only: Read, run, probe. Never edit tracked files, `docs/specs/drafts/`, or git state.
- Evidence-bound: Every finding must cite `<file:line>`, concrete evidence (code, test output, or probe result), and a concrete fix.
- Severity & Verdicts:
  - `Blocker` (blocks approval): Soundness defect, production panic (`unwrap`/`expect`), compiler/phase invariant breach, unmet or untested spec criterion, scope creep, typing/stub/GIL violation, silent workaround, or verification gate failure.
  - `Minor` (non-blocking): Dead code, unneeded memory cloning, minor duplication, or style/doc polish.
  - `Flag` (non-blocking): Architectural trade-off or ambiguity requiring Lead Orchestrator or user decision.
  - Verdicts: `Changes required` (any `Blocker`), `Approve with flags` (zero `Blocker`, 1+ `Flag`), or `Approve` (zero `Blocker`, zero `Flag`).

## Review Steps

1. Diff & Spec Conformance:
   - Inspect `git status --porcelain -uall` and `git diff` against the target draft spec in `docs/specs/drafts/` and `AGENTS.md`.
   - Map every spec acceptance criterion to its implementing code and covering test. Flag missing tests, weak assertions (tests that pass even if logic breaks), or unspecified scope creep as `Blocker`.
2. Verification Gate:
   - Run the `AGENTS.md` verification suite (Rust always; Python when touching `crates/f8-python` or `python/`). Any failure is a `Blocker`.
3. Checklist Audit:
   - Audit all changed files against the checklist below.
4. Targeted Probes (when warranted):
   - When the diff introduces complex edge cases not covered by unit tests, run a temporary script outside the worktree (e.g., agent's scratch directory). Never leave scratch files in the repository.

## Review Checklist

### 1. Compiler Invariants & Architecture

- [ ] Single source of truth (`Blocker`): Domain models, identifiers, coordinate mappings, and errors have one authoritative definition; zero duplication across passes.
- [ ] Clean phase boundaries (`Blocker`): Passes remain decoupled; higher-level domain logic never leaks into low-level IR primitives.
- [ ] Predictable core IR (`Blocker`): Strictly one canonical representation per concept; zero magic fallbacks or core syntactic sugar; isolate ergonomics and convenience to outer adapters.
- [ ] Strong typing over string heuristics (`Blocker`): Explicit enums, interned IDs, and typed AST/IR fields; zero regex/string parsing to infer semantics.
- [ ] Pure data flow (`Blocker`): Explicit inputs and outputs; zero hidden or shared mutable state.
- [ ] Root-cause correctness (`Blocker`): Defects fixed at origin (parser, IR, schema); zero downstream band-aids or test-specific branches.

### 2. Greenfield Rigor

- [ ] Zero compatibility shims or stubs (`Blocker`): No `#[deprecated]`, legacy aliases, transition wrappers, `todo!()`, `unimplemented!()`, or silent workarounds. Callers updated in-place.
- [ ] Zero dead or speculative code (`Minor`): No commented-out tombstones, unused functions/imports, or single-use premature trait/generic abstractions.

### 3. Rust Standards (2024 Edition)

- [ ] Zero production panics (`Blocker`): No `.unwrap()` or `.expect()` outside tests; collection indexing uses `.get()` or pattern matching.
- [ ] Structured error handling (`Blocker`): `thiserror` diagnostics propagated via `?`; zero stringly-typed errors or swallowed context via `.ok()`.
- [ ] Deterministic ordering (`Blocker`): `IndexMap` / `IndexSet` over `HashMap` / `HashSet` wherever iteration, layout, IR, or serialization order matters.
- [ ] Memory & style (`Minor`): Borrowed slices (`&str`, `&[T]`) or `Cow` over unneeded `.clone()`/`.to_string()`; immutable `let` by default; guard clauses over deep nesting.

### 4. Python Standards (3.12+, when touching `crates/f8-python` or `python/`)

- [ ] Strict typing & contracts (`Blocker`): 100% typed (`pyright` strict, explicit `-> None`, `TypedDict` over `dict[str, Any]`); zero bare `Any` or unjustified `# type: ignore`.
- [ ] Native stub parity (`Blocker`): `crates/f8-python` exports and exceptions match `python/f8/_core.pyi` and `python/f8/py.typed`.
- [ ] Exception hygiene (`Blocker`): Zero bare `except:` or swallowed `Exception`; errors inherit from `F8Error` with structured attributes (`raise ... from err`).
- [ ] GIL & immutability (`Blocker`): CPU-bound PyO3 tasks release GIL (`py.allow_threads`) and use buffer protocols (`memoryview`/`bytes`); zero mutable default args; frozen dataclasses where applicable.

## Report Template

```markdown
# Implementation Review: <Spec Title>

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
