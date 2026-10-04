---
title: <Feature or Subsystem Name>
last_updated: YYYY-MM-DD
domain: <Domain> # corresponds to specs/living-specs/<domain>/
status: active # active | deprecated
---

## Purpose
High-level summary of subsystem responsibilities and core problems solved.

## Architecture & boundaries

- **Components**: Primary modules, structs, enums, and traits.
- **Data flow**: Sequence of state transitions across compiler passes or boundaries.
- **Dependencies**: Critical internal and external crate dependencies.

## Requirements & invariants

### 1. <Capability or Invariant Group>
Domain rule or compiler guarantee enforced.

#### Scenarios
- **<Nominal Scenario>**:
  - **When** <trigger or input>
  - **Then** <expected outcome and invariant maintained>

- **<Edge or Error Case>**:
  - **When** <invalid condition or boundary event>
  - **Then** <expected diagnostics or failure response>
