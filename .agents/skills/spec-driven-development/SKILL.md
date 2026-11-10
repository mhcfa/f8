---
name: spec-driven-development
description: >-
  Tri-agent spec-driven development (SDD) workflow.
  Author draft specs, orchestrate Implementer and Reviewer subagents,
  enforce verification gates, and fold durable knowledge into reference docs.
---

# Spec-Driven Development Workflow

## Core rules

- Truth hierarchy (describes current behavior):
  1. Code, types & tests: Authoritative for behavior, data structures, and edge cases.
  2. Reference docs: System architecture (`docs/architecture.md`) and packaged skills/documentation listed in `docs/architecture.md` or `AGENTS.md`.
  3. Draft specs (`docs/specs/drafts/`): Intended delta for active work, not current behavior. A spec that conflicts with code is expected; a spec that conflicts with reference architecture invariants must be escalated to the user.
- Autonomous increments: One draft spec per independently verifiable increment. Decompose multi-phase work into sequential specs.
- Spec custody: Only the Lead Orchestrator authors and amends draft specs; completed specs remain in `docs/specs/drafts/`. Subagents never edit, move, or delete spec files.
- Commit custody: Leave changes uncommitted. Subagents and Lead Orchestrator do not commit; all changes and draft specs remain uncommitted for user review.
- Model tiers: Lead Orchestrator uses the primary model; Implementer subagents dispatch with `Model: 'flash'`; Reviewer subagents dispatch with `Model: 'inherit'` (or `'pro'`) for critical evaluation.

## Tri-Agent Workflow

### 1. Explore & align (Lead Orchestrator)

- Ground designs in existing modules, schemas, AST/IR types, and reference docs listed in `AGENTS.md`.
- Surface candidate approaches with explicit trade-offs.

### 2. Specify (Lead Orchestrator)

- Create `docs/specs/drafts/YYYY-MM-DD-<feature-name>.md` using the template below.
- Define data structures, signatures, invariants, and `Given / When / Then` acceptance criteria (1:1 testable) without repeating file lists across sections.
- Cite workspace files and source paths via backtick code spans.
- Get user approval of the spec before step 3.

### 3. Implement (Implementer Subagent)

- Dispatch a fresh-context subagent with `Model: 'flash'` in the same workspace instructing it to:
  - Implement the target draft spec under `AGENTS.md`. No scope beyond the spec; never edit or delete the spec.
  - On spec gaps, contradictions, or infeasibility: stop and report to the Lead Orchestrator instead of improvising. Lead amends the spec (re-aligning with the user if scope or design changes) and re-dispatches.
  - Pass the `AGENTS.md` verification suite.
  - Leave changes uncommitted, remove scratch files, and return a concise summary of changed files and verification results.

### 4. Review (Reviewer Subagent)

- Dispatch a fresh-context subagent with `Model: 'inherit'` in the same workspace instructing it to run the `review-implementation` skill against the target draft spec and the uncommitted diff.
- Verdict handling:
  - `Changes required`: Re-dispatch the Implementer with the findings, then re-run review with a fresh Reviewer. After 2 failed cycles, escalate to the user.
  - `Approve with flags`: Resolve each `Flag` (decide directly, or ask the user when it affects scope, public API, or architecture) before step 5.
  - `Approve`: Proceed to step 5.
  - `Minor` findings (any verdict): Batch into one Implementer remediation pass, or waive explicitly with a reason.

### 5. Fold durable docs (Lead Orchestrator)

- Reconcile lasting architectural, schema, DSL, or API changes into `docs/architecture.md` and packaged skills/templates (start from the Reviewer's docs-impact flags).
- Retain the draft spec in `docs/specs/drafts/`; do not delete it.
- Leave all changes (spec, code, and documentation) uncommitted for user review.

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

1. <Task title>: <Implementation details and affected file paths>.
2. Verification gate: Run verification suite from `AGENTS.md`.
```
