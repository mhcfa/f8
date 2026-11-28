---
name: review-implementation
description: >-
  Report-only implementation review for code changes and draft specs.
  Use after implementation, before folding durable docs, or on demand for direct-execution changes;
  do not use to edit code or author skills.
---

# Implementation Review Workflow

## Core rules

- Report-only: Read, run, probe. Never edit tracked files, `docs/specs/drafts/`, or git state.
- Review target: The draft spec in `docs/specs/drafts/` when one exists; otherwise the user request that prompted the change.
- Evidence-bound: Every finding cites `<file:line>`, concrete proof, and a concrete fix. Rank behavioral claims by proof strength: asserted (insufficient) < `file:line` trace < executed probe/test against real code < live artifact reproduction. Drop speculative "could break if..." findings not backed by code proof or a run.
- One finding per defect: Report each defect under the single most specific checklist item.
- Severity & verdicts:
  - `Blocker` (blocks approval): Soundness defect, hard-rule breach (`AGENTS.md`), contract/phase invariant breach, unmet or untested acceptance criterion, scope creep, or verification gate failure.
  - `Minor` (non-blocking): Hygiene, memory, or style issue with no correctness impact.
  - `Flag` (non-blocking): Architectural trade-off, ambiguity, or docs impact requiring Lead Orchestrator or user decision.
  - Verdicts: `Changes required` (any `Blocker`), `Approve with flags` (no `Blocker`, 1+ `Flag`), or `Approve` (no `Blocker`, no `Flag`).

## Review steps

1. Verification gate:
   - Run the `AGENTS.md` verification suite. Any failure is a `Blocker`; test output feeds step 2.
2. Diff & spec conformance:
   - Inspect `git status --porcelain -uall` and `git diff HEAD` (staged and unstaged changes) against the review target and `AGENTS.md`.
   - Map every acceptance criterion to its implementing code and covering test (or `met (inspection)` for documentation, template prose, and skill criteria). Missing tests for executable code, weak assertions (tests that pass even if logic breaks), or unspecified scope are `Blocker`.
3. Checklist audit:
   - Audit every changed file against the checklist below, `AGENTS.md`, and `references/repo-checks.md` when present in this skill directory.
4. Blast radius & targeted probes (when warranted):
   - Look past direct symbol callers for non-grep breakage: serialized JSON/wire/schema shapes, cross-language boundaries (Rust/PyO3/Python stubs, TypeScript/Zod contracts), ordering or lifecycle assumptions, and implicit downstream consumers.
   - When the diff introduces complex edge cases or unverified safety claims ("X already handles Y"), run a temporary probe script outside the worktree (e.g., agent's scratch directory) importing the real code rather than trusting a prose trace. Never leave scratch files in the repository. If a check passes unexpectedly easily, confirm the test or probe actually exercised the changed path.

## Review checklist

Audit every changed file against `AGENTS.md` (single source of truth for rules) and classify findings as follows:

- [ ] `AGENTS.md` `Hard rules` (`Blocker`): Suppressions, shims, origin-only fixes, error/panic hygiene, placeholders/debug macros/glob imports, exhaustive enum matching, draft-spec custody.
- [ ] `AGENTS.md` `Core principles` (`Blocker` unless split below): Single source of truth, clean boundaries, canonical contracts & IR, behavioral symmetry, pure data flow, deterministic output, typed semantics.
  - Bool toggles (`Blocker` in public or IR signatures, `Minor` in private helpers).
  - Consistent conventions (`Blocker` in public API/CLI signatures, `Minor` internally).
  - Minimal surface (`Minor`).
- [ ] `AGENTS.md` `Code standards` for touched languages:
  - Rust: Panic-free paths & `thiserror` propagation (`Blocker`); concrete internal signatures, borrowed slices/`Cow`, import order (`Minor`).
  - Python / PyO3: Strict typing, exception hygiene, `_core.pyi`/`py.typed` parity, GIL release & buffer protocols (`Blocker`); immutability & no mutable defaults (`Minor`).
  - TypeScript & React: Type & lint hygiene, `zod` boundary validation, regenerated artifacts (`Blocker`); React 19 `ref`, `cva`/`cn`, semantic tokens (`Minor`).
- [ ] Repository invariants (`Blocker`): Domain invariants in `docs/architecture.md` and `references/repo-checks.md` (when present).
- [ ] Structural fit (`Flag`): Change conforms to the request or spec but relies on awkward adapters, parallel state, or cross-layer seams; name the deeper structural mismatch for Lead Orchestrator or user consideration.
- [ ] Reference docs impact (`Flag`): Change alters architecture, schemas, DSL, or public behavior; name the section of `docs/architecture.md` or packaged skills/templates to update in the fold step.

## Report template

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
