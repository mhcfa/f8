---
title: <Short Title>
date: YYYY-MM-DD
status: draft # draft | in-progress | completed
target-living-spec: specs/living-specs/<target-domain>/<target-living-spec>.md # path, list of paths, or 'none'
---

## Proposal

### Problem
Core limitation, bug, or missing capability addressed.

### Solution
High-level summary of proposed change and mechanical approach.

### Scope
- **In-scope**: Explicit boundaries and deliverables.
- **Out-of-scope**: Deferred capabilities and non-goals.

### Impacted components
Affected crates, modules, structs, enums, traits, CLI arguments, or dependencies.

### Capabilities
- **New capabilities**: System features added.
- **Modified capabilities**: Existing behaviors changed or deprecated.

## Design

### Goals & non-goals
- **Goals**: Target outcomes upon completion.
- **Non-goals**: Deliberately avoided patterns or premature generalizations.

### Invariants
Core domain invariants, compiler guarantees, and constraints preserved.

### Technical decisions
- **Decision**: Chosen architecture or data structure.
- **Rationale**: Why chosen over alternatives.
- **Alternatives considered**: Trade-offs evaluated and reason for rejection.

### Data contracts (optional)
Compact schema tables or type signatures for new AST nodes, IR structs, or trait signatures.

### Risks & mitigations
- **Risk**: Identified failure point or regression danger.
- **Mitigation**: Strategy, architectural guard, or verification mechanism.

### Acceptance criteria (Given / When / Then)

- [ ] **Scenario 1: <Scenario Title>**
  - **Given** <precondition>
  - **When** <trigger or action>
  - **Then** <expected system behavior>

- [ ] **Scenario 2: <Error / Edge Case>**
  - **Given** <precondition>
  - **When** <invalid input or error condition>
  - **Then** <expected rejection or structured error>

## Tasks

- [ ] 1. <Task title>: <Task details and affected files>.
- [ ] 2. <Task title>: <Task details and affected files>.
- [ ] 3. Run verification suite (`cargo test`, `cargo clippy`, `uv run pytest`).
