# AGENTS.md

Guidelines for AI agents working with this codebase.

## Development and Quality Principles

- **Single source of truth**: Centralize shared definitions and avoid duplicating logic.
- **Separation of concerns**: Keep clean boundaries between features.
- **Local reasoning and single responsibility**: Keep functions and modules small and focused on a single task.
- **Pragmatic simplicity**: Favor direct, idiomatic Rust code. Avoid premature abstractions, unnecessary wrappers, or speculative generalities unless there is a demonstrated requirement.
- **Explicit and predictable core data structures**: Keep compiler core types and IR representations dead simple, explicit, and predictable, with strictly one way to represent any concept. Reject implicit behavior, magic fallbacks, or core syntactic sugar; leave authoring ergonomics, scaffolding, and syntactic convenience entirely to outer adapters and tooling.
- **Explicit data flow**: Prefer pure functions with explicit inputs and outputs. Minimize hidden side effects, implicit state dependencies, and unexpected argument mutations.
- **Root-cause correctness**: Fix bugs and design gaps at their origin in the proper compiler phase or data model. Never introduce downstream band-aids, test-specific overrides, or tactical workarounds.
- **Strong typing over string heuristics**: Model domain concepts, states, and entities with explicit enums and typed structures. Avoid stringly-typed logic, string guessing, substring matching, or ad-hoc parsing to determine semantics.

## Spec-Driven-Development

- When tasked with non-trivial features or refactoring, invoke [spec-driven development workflow and guidelines](specs/sdd-guidelines.md).

## Git Conventions

- **Format**: Follow Conventional Commits (`<type>(<scope>): <summary>`). Common types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`.
- **Subject line**: Imperative mood, lowercase after colon, no trailing period.
- **Body breakdown**:
  - Separate subject from body with a single blank line.
  - Use hyphen bullet points (`- `) for itemized changes.
  - Keep bullets contiguous with no empty lines between breakdown items.

## Code Style & Rust Guidelines

- **Rust edition**: Use Rust 2024 edition features and idioms.
- **Variables**: Use immutable `let` bindings by default; use `let mut` only when mutation is strictly required.
- **Zero production panics**: Never use `.unwrap()` or `.expect()` in production code. Propagate errors cleanly. Unwraps are allowed only in test targets (`#[cfg(test)]` or `tests/`).
- **Error handling**: Use `thiserror` for structured domain and compiler diagnostic errors; bubble errors via `?`.
- **Deterministic ordering**: Use `IndexMap` / `IndexSet` instead of standard `HashMap` / `HashSet` when iteration order or deterministic serialization/layout generation is required.
- **Allocations and efficiency**: Prefer borrowed references (`&str`, `&[T]`) or `Cow` over unnecessary cloning in compiler passes.
- **Guard clauses**: Use early returns and guard clauses; avoid deeply nested conditionals.
- **Verification suite**: Always verify changes using standard Cargo commands:
  - `cargo check`: Quick compile check.
  - `cargo test`: Unit and integration tests.
  - `cargo clippy --all-targets -- -D warnings`: Linting with zero warnings and no unhandled panics.
  - `cargo fmt --check`: Code formatting adherence.

## Markdown and Documentation Style

- **Sentence case for lists and labels**: In English documentation, capitalize only the first word (and proper nouns/acronyms) in list items and bold prefix labels.
- **Standard and clean formatting**: Use standard GitHub-Flavored Markdown.
- **Consistent references**: Prefer clean relative markdown links when cross-referencing internal docs and specs.
- **Extremely concise and objective**: Use EXTRA concise and objective language when drafting documentation and specs.

## Naming Conventions

- **Rust**:
  - Files: `snake_case.rs`
  - Structs, enums, traits, variants, type aliases: `PascalCase`
  - Functions, methods, variables, modules: `snake_case`
  - Constants and statics: `SCREAMING_SNAKE_CASE`
- **Specs and documentation**:
  - Spec filenames: `kebab-case.md`
  - Living specs: grouped by domain in `specs/living-specs/<domain>/<topic>.md`

## Documentation Layout

- `docs/`: Strategic architecture and design specifications.
- `specs/`: Technical specifications for spec-driven development.
  - `specs/living-specs/<domain>/`: Permanent single source of truth for active subsystems.
  - `specs/drafts/`: In-flight draft specifications.
  - `specs/templates/`: Markdown templates for draft and living specs.
  - `specs/history/`: Completed, reconciled, and archived specifications.
