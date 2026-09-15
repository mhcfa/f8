# Spec-Driven Development Guidelines

Spec-driven development (SDD) separates ephemeral execution tasks from durable domain knowledge, keeping codebases maintainable, verified, and aligned over time.

## Core Philosophy

- **Dual-spec model**:
  - **Living specs** (`specs/living-specs/<domain>/`): The permanent single source of truth describing current system behavior, invariants, and capabilities.
  - **Draft specs** (`specs/drafts/`): Transient working documents defining a specific proposal, design decisions, acceptance criteria, and task checklist.
- **Spec-level decomposition**: One draft spec represents one autonomous, verifiable increment. Complex initiatives are decomposed into a sequence of standalone specs.
- **Verification as proof**: Specifications map directly to testable scenarios, unit tests, and compiler verification checks.

## Workflow

```mermaid
flowchart LR
    A["1. Explore"] --> B["2. Specify"]
    B --> C["3. Implement & Verify"]
    C --> D["4. Reconcile & Archive"]
```

### 1. Explore & Discover

Before proposing solutions or writing code:

- **Grounded in reality**: Inspect relevant living specs in `specs/living-specs/<domain>/`, existing modules, data models, and dependency constraints rather than hypothesizing in the abstract.
- **Explore trade-offs**: Surface 2–3 viable technical paths and evaluate their trade-offs instead of defaulting to a single biased option.
- **Decompose into distinct specs**: If an initiative is complex, ambiguous, or spans multiple compiler phases, outline a high-level roadmap of sequential phases. Actively propose implementing each phase as its own separate draft spec rather than bundling multiple phases into one spec.

### 2. Specify & Design

Capture the plan in a focused draft spec:

- Create `specs/drafts/YYYY-MM-DD-<feature-name>.md` using `specs/templates/draft-spec.md`.
- Set `target-living-spec` in frontmatter to link the draft to its destination under `specs/living-specs/<domain>/`.
- Focus exclusively on the current phase or milestone.
- Define the proposal scope, invariants, architectural choices with rationale, and testable acceptance scenarios (`Given / When / Then`).
- **Concise contracts over code dumps**: Avoid dumping full implementation code. Summarize data structures, AST enums, and traits using concise markdown tables or type signatures. Use short code snippets only when illustrating non-obvious interface designs or subtle edge cases.
- Break down execution into sequential, manageable tasks.

### 3. Implement & Verify

Execute against the defined specification:

- Build code and tests that satisfy the acceptance criteria.
- Keep the task checklist in the draft spec updated as progress occurs.
- Verify work using Rust standard toolchain commands
  - `cargo check`: Fast syntax and type checking.
  - `cargo test`: Comprehensive unit and integration test suite.
  - `cargo clippy --all-targets -- -D warnings`: Strict linter checks with zero warnings and no unhandled panics.
  - `cargo fmt --check`: Formatting conformance.

### 4. Reconcile & Archive

When all tasks and verifications are complete:

- **Update living specs**: Update or create the relevant file in `specs/living-specs/<domain>/` to reflect current system behavior.
- **Archive draft**: Move the completed draft from `specs/drafts/` to `specs/history/`.
