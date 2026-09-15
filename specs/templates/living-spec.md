---
title: <Feature or Subsystem Name>
last_updated: YYYY-MM-DD
domain: <Domain> # corresponds to specs/living-specs/<domain>/
status: active # active | deprecated
---

## Purpose

High-level summary of what this subsystem or capability provides and the core problem it solves.

## Architecture & Boundaries

- **Modules & components**: Primary modules, traits, or structs involved.
- **Data flow & interfaces**: How state and data move across architectural boundaries.
- **Dependencies**: Critical internal or external crate dependencies.

## Requirements & Invariants

Describe the permanent domain rules, compiler invariants, and behaviors using clear, testable statements and scenarios.

### 1. <Capability or Invariant Group>

High-level description of what the system guarantees.

#### Scenarios

- **<Scenario Title>**:
  - **When** <trigger or state transition occurs>
  - **Then** <expected system behavior or invariant that holds>

- **<Error or Edge Case>**:
  - **When** <invalid input or compiler error occurs>
  - **Then** <expected diagnostics, recovery, or rejection>
