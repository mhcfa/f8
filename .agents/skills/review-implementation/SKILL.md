---
name: review-implementation
description: >-
  Report-only implementation review for the f8 compiler, runtime, and language workspace.
  Verification gate, spec conformance, greenfield rigor, single source of truth,
  Rust 2024 & Python standards, adversarial probing, and decisive verdicts.
  Use after implementation, before living spec reconciliation and archive.
---

# Implementation Review Workflow

Use this skill to conduct rigorous, objective, report-only code reviews for implementations, refactors, and feature increments in the `f8` codebase.

---

## Core Philosophy

- **Report-only**: Read, run, probe. Zero edits to tracked files, `specs/living-specs/`, `specs/drafts/`, or git state. All remediation flows back through the Lead Orchestrator.
- **Independent context**: Executed in a fresh Reviewer Subagent context (or standalone review). Free from author confirmation bias; judge work strictly against specs and repository standards.
- **Evidence-bound**: Every finding cites `file:line`, concrete evidence (code excerpt, test output, or probe result), and a suggested fix. No location, no finding.
- **Root-cause correctness**: Reject downstream band-aids, temporary shims, silent workarounds, or test-specific hacks.
- **Decisive verdicts**: Clear classification (`Changes required`, `Approve with flags`, `Approve`) based on explicit severity criteria.

---

## Workflow

Context -> Verification Gate -> Spec Conformance -> Checklist Pass -> Adversarial Probes -> Report

### 1. Context & Diff

- **Truth & specs**: Read target draft spec in `specs/drafts/` and referenced living specs in `specs/living-specs/<domain>/`.
- **Standards**: Re-read `AGENTS.md`.
- **Diff inspection**:
  - `git status --porcelain -uall`
  - `git diff <base>` (base vs. working tree, including uncommitted changes)
  - `git log --oneline <base>..HEAD`
  - Cross-check actual changed files against spec `Impacted components` / `Affected files`.

### 2. Verification Gate

- **Rust verification**:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
- **Python verification** (when touching `crates/f8-python` or `python/`):
  - `uv run ruff check .`
  - `uv run ruff format --check .`
  - `uv run maturin develop` (if native extension bindings changed)
  - `uv run pyright`
  - `uv run pytest`
- **Gate rule**: Any failure is a `Blocker` resulting in verdict `Changes required`. Continue review to report all findings.

### 3. Spec Conformance

- **Criteria mapping**: Map each acceptance criterion from the spec to implementing code and covering tests.
- **Test quality**: Verify tests assert behavioral outcomes and fail if implementation logic is broken or reverted.
- **Boundary audit**: Flag missing tests for specified criteria (`Major`); flag unspecified behavior or scope creep (`Major`); flag unticked tasks in draft spec (`Minor`).

### 4. Checklist Pass

- **Load**: `.agents/skills/review-implementation/references/checklist.md`.
- **Evaluate**: Apply every relevant category (Compiler Invariants, Greenfield Rigor, Rust Standards, Python Standards). Record skipped categories with justification.

### 5. Adversarial Probes

- **Location**: Agent scratch directory outside repository working tree (e.g. `<appDataDir>/brain/<conversation-id>/scratch/`). Never commit probe scripts into repository.
- **Target failure points**:
  - Zero-dimension ranges, empty coordinate matrices, single-cell tables.
  - Circular dependency chains and topological cycle detection.
  - Multi-tier column header spanning and misalignment.
  - PyO3 boundary type mismatches, null values, and GIL release during CPU-bound tasks.
  - Formatted value cache coherence after formula re-evaluation.

### 6. Report Delivery

- Deliver in-conversation following the Report Schema. Write no permanent review file into git tree.

---

## Severity Classifications

| Severity | Definition | Blocks Approval |
| :--- | :--- | :--- |
| `Blocker` | Soundness defect, production panic (`unwrap`/`expect`), compiler invariant breach, phase boundary violation, memory safety/GIL defect, stub desynchronization, silent workaround, or verification gate failure. | Yes |
| `Major` | Acceptance criterion untested or unmet, scope creep, silent error handling / swallowed error context, typing escape hatch (`Any`, unjustified ignore), or stringly-typed semantics. | Yes |
| `Minor` | Dead code, commented-out code, unused imports, duplication, docs drift, style deviation, unneeded memory cloning. | No |
| `Nit` | Optional polish, minor phrasing in comments. | No |
| `Flag` | Architectural observation or trade-off requiring Lead Orchestrator or user decision. | No |

---

## Verdicts

- **`Changes required`**: Any `Blocker` or `Major` finding. Lead Orchestrator will re-dispatch Implementer Subagent with findings table and remediation steps.
- **`Approve with flags`**: Zero `Blocker` and zero `Major` findings; one or more `Flag` items surfaced for user attention.
- **`Approve`**: Clean review meeting all standards. Proceed to living spec reconciliation and archive.

---

## Report Schema

```markdown
# Implementation Review Report: <Spec Title>

## Verdict: `<Changes required | Approve with flags | Approve>`
<One-line rationale for the verdict.>

## Verification gate

| Command | Status | Details |
| :--- | :--- | :--- |
| `cargo check` | Pass/Fail | <details> |
| `cargo test` | Pass/Fail | <counts> |
| `cargo clippy --all-targets -- -D warnings` | Pass/Fail | <warnings> |
| `cargo fmt --check` | Pass/Fail | <status> |
| `uv run ruff check .` | Pass/Fail/Skipped | <counts> |
| `uv run ruff format --check .` | Pass/Fail/Skipped | <status> |
| `uv run maturin develop` | Pass/Fail/Skipped | <status> |
| `uv run pyright` | Pass/Fail/Skipped | <counts> |
| `uv run pytest` | Pass/Fail/Skipped | <counts> |

## Spec conformance

| Acceptance criterion | Implementation | Test | Status |
| :--- | :--- | :--- | :--- |
| <Scenario 1> | `<file:line>` | `<test_file:line>` | `met` |
| <Scenario 2> | `<file:line>` | `<test_file:line>` | `untested` |

## Findings

| ID | Severity | Category | Location | Evidence | Fix | Confidence |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| R1 | `Blocker` | Compiler Invariants | `<file:line>` | `<evidence>` | `<fix>` | `high` |
| R2 | `Major` | Python Standards | `<file:line>` | `<evidence>` | `<fix>` | `high` |

## Coverage

- **Reviewed categories**: <list of checklist categories evaluated>
- **Skipped categories**: <list of skipped categories with rationale>
```
