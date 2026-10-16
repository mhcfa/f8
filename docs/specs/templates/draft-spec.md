---
title: <Short Title>
date: YYYY-MM-DD
---

## Problem & Scope

### Problem
Core limitation, defect, or missing capability addressed.

### Scope
- **In-scope**: Concrete deliverables and behavioral changes.
- **Out-of-scope / Non-goals**: Deferred capabilities and deliberately avoided patterns.

## Design & Invariants

### Technical decisions
- **Decision**: Chosen architecture, compiler pass, or data structure.
- **Rationale**: Why chosen over alternatives (including rejected options and trade-offs).

### Invariants
Core domain invariants, compiler guarantees, and phase boundaries preserved.

## Data Contracts

Compact type signatures or schema definitions for new/modified AST nodes, IR structs, traits, or Python SDK contracts (omit if none).

## Acceptance Criteria

- **Scenario 1: <Nominal Behavior>**
  - **Given** <precondition>
  - **When** <trigger or action>
  - **Then** <expected system behavior>

- **Scenario 2: <Error / Boundary Case>**
  - **Given** <precondition>
  - **When** <invalid input or boundary condition>
  - **Then** <expected rejection or structured error>

## Tasks

1. **<Task title>**: <Implementation details and affected file paths>.
2. **<Task title>**: <Implementation details and affected file paths>.
3. **Verification gate**: Run verification suite from `AGENTS.md`.
