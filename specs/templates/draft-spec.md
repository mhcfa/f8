---
title: <Short Title>
date: YYYY-MM-DD
status: draft # draft | in-progress | completed
target-living-spec: specs/living-specs/<target-domain>/<target-living-spec>.md # single path, list of paths, or 'none'
---

## Proposal

### What

Explain what is being built or modified using concise, objective language.

### Why

Explain why this is needed and what problem it solves.

### Scope

Outline what is in-scope and out-of-scope.

### What Changes

Affected modules, structs, enums, traits, CLI arguments, or dependencies.

### Capabilities

- **New capabilities**: What the system can now do.
- **Modified capabilities**: What existing behaviors change.

## Design

### Goals

Key outcomes to achieve upon completion.

### Invariants

Core domain rules, invariants, and compiler guarantees that must not be broken.

### Decisions

Key choices made during planning:

- **Choice**: What choice was made.
- **Rationale**: Why this choice was selected.
- **Alternatives considered**: Other options evaluated and why they were rejected.

### Data Contracts (Optional)

Summarize new or modified structs, AST nodes, and enums using concise tables or type signatures.

### Risks & Trade-offs

Identified trade-offs, potential failure points, and mitigation strategies.

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: <Scenario Title>**
  - **Given** <precondition>
  - **When** <action/event>
  - **Then** <expected outcome>

- [ ] **Scenario 2: <Error / Edge Case>**
  - **Given** <precondition>
  - **When** <action/event>
  - **Then** <expected outcome>

## Tasks

1. Make tasks manageable, sequential, and independently verifiable.
2. Group tasks into logical phases appropriate for the change.
3. Include explicit verification checks (`cargo test`, `cargo clippy`, `cargo fmt`).
