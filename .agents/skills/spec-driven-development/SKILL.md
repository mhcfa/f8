---
name: spec-driven-development
description: >-
  Tri-agent spec-driven development (SDD) workflow.
  Author ephemeral draft specs, orchestrate Implementer and Reviewer subagents,
  enforce verification gates, fold durable knowledge into reference docs, and clean up specs.
---

# Spec-Driven Development Workflow

## Core rules

- Truth hierarchy (current behavior):
  1. Code, types & tests: Authoritative for behavior, data structures, and edge cases.
  2. Reference docs: `docs/architecture.md` and packaged skills/docs listed in `docs/architecture.md` or `AGENTS.md`.
  3. Draft specs (`docs/specs/drafts/`): Intended delta for active work. Conflicts with current code are expected; conflicts with reference architecture invariants require user escalation.
- Autonomous increments: One draft spec per independently verifiable increment. Decompose multi-phase work into sequential specs.
- Spec & commit custody: Only the Lead Orchestrator authors, amends, or deletes draft specs (keeping them in `docs/specs/drafts/` through review and deleting on commit per `AGENTS.md`), and leaves all changes uncommitted until the user requests a commit. Subagents never edit, move, or delete specs, and never commit.

## Tri-Agent Workflow

### 1. Explore & align (Lead Orchestrator)

- Ground designs in existing modules, schemas, AST/IR types, and reference docs listed in `AGENTS.md`.
- Surface candidate approaches with explicit trade-offs.

### 2. Specify (Lead Orchestrator)

- Create `docs/specs/drafts/YYYY-MM-DD-<feature-name>.md` using the template below.
- Define data structures, signatures, invariants, and 1:1 testable `Given / When / Then` acceptance criteria without repeating file lists across sections; structure tasks as vertical slices rather than deferring tests to a trailing task.
- Get user approval of the spec before step 3.

### 3. Implement (Implementer Subagent)

- Dispatch a fresh-context subagent in the same workspace instructing it to:
  - Implement the target draft spec under `AGENTS.md` with no extra scope, without touching the spec file.
  - Cover every executable `Acceptance Criteria` scenario with falsifiable tests (reproducing bug fixes with a failing test first, per `AGENTS.md`).
  - Stop and report to the Lead Orchestrator on spec gaps, contradictions, or infeasibility instead of improvising. Lead amends the spec (re-aligning with the user if scope or design changes) and re-dispatches.
  - Pass the `AGENTS.md` verification suite, remove scratch files, leave changes uncommitted, and return a concise summary of changed files and verification results.

### 4. Review (Reviewer Subagent)

- Dispatch a fresh-context subagent in the same workspace instructing it to run `review-implementation` against the target draft spec and uncommitted diff.
- Verdict handling:
  - `Changes required`: Re-dispatch Implementer with findings, then re-run review with a fresh Reviewer. Escalate to the user after 2 failed cycles.
  - `Approve with flags`: Resolve each `Flag` (decide directly, or ask the user when it affects scope, public API, or architecture) before step 5.
  - `Approve`: Proceed to step 5.
  - `Minor` findings (any verdict): Batch into one Implementer remediation pass, or waive explicitly with a reason.

### 5. Fold durable docs (Lead Orchestrator)

- Reconcile lasting architectural, schema, DSL, or API changes into `docs/architecture.md` and packaged skills/templates, starting from Reviewer docs-impact flags.

## Draft Spec Template

```markdown
---
title: <Short Title>
date: YYYY-MM-DD
---

## Problem & Scope

- Problem: Core limitation, defect, or missing capability addressed.
- In-scope: Concrete deliverables and behavioral changes.
- Out-of-scope: Deferred capabilities and deliberately avoided patterns.

## Design & Invariants

- Decisions & rationale: Chosen architecture, pass/module boundary, or data structure and why it was chosen over alternatives.
- Invariants: Domain invariants, boundary contracts, and guarantees preserved.

## Data Contracts

Compact type signatures or schema definitions for new/modified AST/IR nodes, schemas, traits, or public SDK/CLI contracts (omit if none).

## Acceptance Criteria

- Scenario 1: <Nominal Behavior>
  - Given <precondition> / When <action> / Then <expected outcome>
- Scenario 2: <Error / Boundary Case>
  - Given <precondition> / When <invalid/boundary input> / Then <structured error/outcome>

## Tasks

1. <Vertical slice title>: <Covering tests, implementation details, and affected file paths>.
2. Verification gate: Run verification suite from `AGENTS.md`.
```
