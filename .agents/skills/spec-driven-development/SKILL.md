---
name: spec-driven-development
description: >-
  Tri-agent spec-driven development (SDD) workflow for the f8 compiler workspace.
  Author ephemeral draft specs, orchestrate Implementer and Reviewer subagents,
  enforce verification gates, fold durable knowledge into reference docs, and clean up specs.
---

# Spec-Driven Development Workflow

## Core rules

- Truth hierarchy:
  1. Code, types & tests: Authoritative truth for behavior, data structures, and edge cases.
  2. Reference docs (`docs/`): System architecture (`docs/architecture.md`), DSL schemas (`docs/dsl/`), and function catalog (`docs/functions/catalog.md`).
  3. Draft specs (`docs/specs/drafts/`): Ephemeral execution contracts for active work.
- Autonomous increments: One draft spec per independently verifiable increment. Decompose multi-phase work into sequential specs.
- Spec custody: Draft specs (`docs/specs/drafts/`) are authored and deleted exclusively by the Lead Orchestrator. Subagents never edit, move, or delete spec files.

## Tri-Agent Workflow

### 1. Explore & align (Lead Orchestrator)

- Ground designs in existing Rust/Python modules, AST/IR types, and `docs/architecture.md`.
- Surface candidate approaches with explicit trade-offs and align with the user before coding when requirements are ambiguous.

### 2. Specify (Lead Orchestrator)

- Create `docs/specs/drafts/YYYY-MM-DD-<feature-name>.md` using the template below.
- Define data structures, signatures, invariants, and `Given / When / Then` acceptance criteria (1:1 testable) without repeating file lists across sections.
- Cite source paths via backtick code spans (e.g. `crates/f8-core/src/compiler/mod.rs`); reserve markdown links for stable `docs/` files.

### 3. Implement (Implementer Subagent)

- Dispatch an isolated subagent in the shared workspace instructing it to:
  - Implement strictly to the target draft spec and `AGENTS.md` (zero scope creep; never edit or delete the draft spec).
  - Run and pass the `AGENTS.md` verification suite (Rust always; Python when touching `crates/f8-python` or `python/`).
  - Clean up any temporary scratch files and return a concise summary of changed files and verification results.

### 4. Review (Reviewer Subagent)

- Dispatch an independent subagent in the shared workspace instructing it to run the `review-implementation` skill against the target draft spec and current git diff.
- Verdict handling:
  - `Changes required`: Re-dispatch the Implementer with the findings to remediate, then re-verify.
  - `Approve with flags` / `Approve`: Surface any architectural `Flag` items and proceed to step 5.

### 5. Fold & Delete (Lead Orchestrator)

- Reconcile lasting architectural, DSL, or function changes into `docs/architecture.md`, `docs/dsl/`, or `docs/functions/catalog.md`.
- Delete the completed draft spec from `docs/specs/drafts/` so zero stale specs remain in the working tree.

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

- Decisions & rationale: Chosen architecture, compiler pass, or data structure and why it was chosen over alternatives.
- Invariants: Domain invariants, compiler guarantees, and phase boundaries preserved.

## Data Contracts

Compact type signatures or schema definitions for new/modified AST nodes, IR structs, traits, or Python SDK contracts (omit if none).

## Acceptance Criteria

- Scenario 1: <Nominal Behavior>
  - Given <precondition> / When <action> / Then <expected outcome>
- Scenario 2: <Error / Boundary Case>
  - Given <precondition> / When <invalid/boundary input> / Then <structured error/outcome>

## Tasks

1. <Task title>: <Implementation details and affected file paths>.
2. Verification gate: Run verification suite from `AGENTS.md`.
```
