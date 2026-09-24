---
name: review-implementation
description: >-
  Review code changes, refactors, and feature implementations for greenfield rigor,
  clean architecture, modularity, single source of truth, zero dead code or legacy shims,
  and Rust 2024 quality standards. Use when asked to review an implementation, audit a refactor,
  or inspect code quality.
---

# Implementation Review Workflow

Use this skill to conduct rigorous, objective code reviews for implementations, refactors, and feature increments in this greenfield codebase.

---

## Core Review Pillars

1. **Greenfield minimalism**: Zero tolerance for dead code, backward-compatibility shims, deprecated aliases, or half-baked stubs.
2. **Single source of truth (SSOT)**: Domain models, identifiers, invariants, and error definitions must have exactly one authoritative source.
3. **Spec-driven alignment**: Implementation must adhere to active draft specs (`specs/drafts/`) and preserve living spec contracts (`specs/living-specs/`).
4. **Rust 2024 robustness**: Strict error propagation, zero unhandled panics, deterministic collections, and allocation efficiency.
5. **Root-cause correctness**: Zero tolerance for tactical hacks, symptom-patching workarounds, or string-based semantic guessing.

---

## Step-by-Step Review Procedure

### Step 1: Context and Spec Alignment

1. Inspect git status and changed files:
   - Identify newly added, modified, and removed files.
2. Locate the corresponding draft or living spec:
   - Draft specs: `specs/drafts/YYYY-MM-DD-<feature>.md`
   - Living specs: `specs/living-specs/<domain>/<topic>.md`
3. Verify contract coverage:
   - Confirm all acceptance criteria (`Given / When / Then`) from the spec are implemented.
   - Confirm no out-of-scope features were smuggled into the implementation.

### Step 2: Greenfield Purge Audit

Because this is a greenfield codebase with no legacy production callers, enforce clean breaks:

- **No compatibility shims**: Reject `#[deprecated]` attributes, legacy re-exports, alias types, or transition wrappers. Callers must be updated in-place across the repository.
- **No dead code or tombstones**: Remove commented-out code, unused functions, obsolete trait methods, abandoned match arms, and unused imports.
- **No speculative code (YAGNI)**: Flag premature trait abstractions, unused generics, or configuration options with only one concrete usage.
- **No placeholder stubs**: Eliminate `todo!()`, `unimplemented!()`, or empty placeholder structs/functions left behind.
- **No hacks or workarounds**: Reject downstream band-aids that patch symptoms instead of root causes. Reject special-cased branches, hardcoded magic values, or test-specific overrides designed merely to make a test pass.

### Step 3: Architecture, Modularity, and SSOT

- **Single source of truth**:
  - Verify no duplicated data structures or logic across compiler phases (e.g., AST vs IR vs Table models).
  - Verify centralized identifier and error definitions rather than parallel redefinitions.
- **Clean phase boundaries**:
  - Ensure compiler phases remain cleanly decoupled (e.g., Parser -> AST -> Lowering -> IR -> Graph -> Layout -> Emission / Eval).
  - Prevent higher-level domain knowledge from leaking into lower-level primitives.
- **Local reasoning and explicit data flow**:
  - Keep functions focused on a single responsibility.
  - Prefer pure functions with explicit inputs and outputs over shared or hidden mutable state.
  - Verify use of immutable `let` bindings by default; allow `let mut` only when mutation is strictly required.
- **Strong typing over string heuristics**:
  - Reject string guessing and ad-hoc matching (e.g., `starts_with`, `ends_with`, regex, substring inspection, or splitting formatted strings) to infer semantics, types, or roles.
  - Require explicit enum variants, interned symbol IDs, or dedicated AST/IR node fields.
  - Prevent round-trip string serialization (e.g., converting to string just to inspect or compare values) when typed representations exist.

### Step 4: Rust 2024 and Operational Robustness

- **Zero production panics**:
  - Ensure zero calls to `.unwrap()` or `.expect()` in non-test code (`clippy::unwrap_used` and `clippy::expect_used`).
  - Verify index operations do not risk out-of-bounds panics; prefer `.get()` or pattern matching.
- **Structured error handling**:
  - Verify errors use `thiserror` and bubble up cleanly with `?`.
  - Avoid stringly-typed error messages or discarding error context with `.ok()`.
- **Deterministic ordering**:
  - Ensure `IndexMap` and `IndexSet` are used instead of `HashMap` and `HashSet` wherever iteration order or output determinism matters.
- **Allocation efficiency**:
  - Prefer borrowed references (`&str`, `&[T]`) or `Cow` over unneeded `.clone()`, `.to_vec()`, or `.to_string()`.
- **Guard clauses**:
  - Ensure early returns and guard clauses are used to prevent deeply nested `if/else` ladders.

### Step 5: Automated Verification Suite

Run the project verification commands and verify clean passes:

```bash
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

All commands must complete with zero errors, zero warnings, and zero unhandled panics.
