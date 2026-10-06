# Spec-Driven Development Guidelines

Spec-driven development (SDD) decouples ephemeral task execution from durable domain knowledge.

## Core philosophy

- **Dual-spec architecture**:
  - **Living specs** (`specs/living-specs/<domain>/`): Permanent single source of truth for runtime behavior, compiler invariants, and capabilities.
  - **Draft specs** (`specs/drafts/`): Ephemeral execution plans defining proposals, technical trade-offs, acceptance criteria, and task checklists.
- **Autonomous increments**: One draft spec represents one independently verifiable increment. Decompose multi-phase initiatives into sequential specs.
- **Verification as proof**: Acceptance criteria map 1:1 to executable tests and compiler checks.

## Workflow

```mermaid
flowchart LR
    A["1. Explore"] --> B["2. Specify"]
    B --> C["3. Implement & Verify"]
    C --> D["4. Reconcile & Archive"]
```

### 1. Explore & discover

- **Inspect reality**: Ground designs in existing modules, AST/IR models, and living specs under `specs/living-specs/<domain>/`. Ban unverified assumptions.
- **Evaluate trade-offs**: Surface 2–3 technical options with explicit pros/cons before choosing an approach.
- **Decompose milestones**: Split multi-pass compiler changes or multi-layer features into sequential draft specs.

### 2. Specify & design

- **Instantiate template**: Create `specs/drafts/YYYY-MM-DD-<feature-name>.md` from `specs/templates/draft-spec.md`.
- **Target destination**: Set `target-living-spec` frontmatter pointing to `specs/living-specs/<domain>/`.
- **Compact contracts**: Define data structures, AST enums, and trait signatures via markdown tables or type signatures. Ban speculative code dumps.
- **Durable references**: Cite source paths and modules via backtick code spans (e.g. `crates/f8-core/src/compiler/mod.rs`). Reserve relative links exclusively for living specs and stable docs.
- **Acceptance criteria**: Enforce testable behavior using `Given / When / Then` scenarios.
- **Actionable checklist**: Break work into ordered, independently testable tasks.

### 3. Implement & verify

- **Fulfill criteria**: Build implementation and test coverage against specified scenarios.
- **Track execution**: Maintain real-time task checklist progress in the draft spec.
- **Run verification suites**:
  - **Rust**:
    - `cargo check`: Syntax and type validation.
    - `cargo test`: Unit and integration suite.
    - `cargo clippy --all-targets -- -D warnings`: Strict linting with zero warnings and no unhandled panics.
    - `cargo fmt --check`: Formatting conformance.
  - **Python** (SDK or PyO3 changes):
    - `uv run ruff check .`: Linting (`E`, `F`, `I`, `UP`, `B`, `SIM`).
    - `uv run ruff format --check .`: Code formatting.
    - `uv run pyright`: Strict type checking with zero errors/warnings.
    - `uv run pytest`: Integration and parity test suite.
    - `uv run maturin develop`: Recompile native extension after Rust binding changes.

### 4. Reconcile & archive

- **Update living specs**: Sync changes into `specs/living-specs/<domain>/` to reflect current system behavior.
- **Archive draft**: Move completed spec from `specs/drafts/` to `specs/history/`.
