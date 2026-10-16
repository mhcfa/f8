# Agent Guidelines

Guidelines for AI agents working with the f8 compiler, runtime, and language workspace.

> [!IMPORTANT]
> **Polyglot workspace**: Core IR and compiler in Rust 2024; native bindings in PyO3; Python 3.12+ high-level package.
> **Spec-driven**: Invoke [spec-driven development workflow and guidelines](docs/specs/sdd-guidelines.md) for non-trivial features or refactoring.

## Repository context & architecture

| Path                | Layer / Target      | Purpose & architectural rules                                                                                                                                                 |
| :------------------ | :------------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/f8-core/`   | Core engine (Rust)  | Pure in-memory calculation core: 6 compiler passes (IR, DAG/SCC, evaluator, layout, lowering, emitter) and `CompiledModel` facade.                                            |
| `crates/f8-python/` | Native C-ABI (PyO3) | Native C-extension (`f8._core`, `abi3-py312`). Deserializes inputs via `pythonize`, bridges `CompiledModel`, maps typed exceptions, and releases GIL on CPU tasks.            |
| `python/f8/`        | High-level SDK (Py) | Fluent modeling DSL (`Model`, `Table`, `Cell`), project disk I/O, auditing facade (`TableAuditor`), synchronized stubs (`.pyi`), `py.typed`, and bundled `f8-modeling` skill. |
| `python/tests/`     | Test suite (Pytest) | End-to-end integration suite for Python DSL, PyO3 bindings, project I/O, inspection audit, and Excel calculation parity.                                                      |
| `.agents/`          | Agent workflows     | Git-tracked agent skills, subagent prompts, and verification routines.                                                                                                        |
| `docs/`             | Docs & SDD specs    | Architecture overview, 2D matrix DSL design, function catalog, and spec-driven development workflow (`docs/specs/sdd-guidelines.md` & `docs/specs/drafts/`).                  |

## Development principles

- **Single source of truth**: Centralize shared definitions. Zero logic duplication across crates or modules.
- **Predictable core IR**: Strictly one canonical representation per concept. Zero magic fallbacks or core syntactic sugar; isolate ergonomics and convenience to outer adapters.
- **Pure data flow**: Pure functions with explicit inputs and outputs. Zero global mutable state or implicit side effects.
- **Root-cause correctness**: Fix defects at parser, IR, or schema origin. Zero downstream band-aids or test overrides.
- **Strong typing**: Model domain concepts with explicit enums and typed structs. Ban stringly-typed heuristics.
- **Pragmatic simplicity**: Direct, idiomatic Rust and Python. Ban premature abstractions or speculative generalizations.
- **Greenfield rigor**: Zero compatibility shims, `#[deprecated]` aliases, transition wrappers, dead code, commented-out tombstones, or placeholder stubs (`todo!()`, `unimplemented!()`). Update callers in-place across the repository.
- **Zero silent workarounds**: Never patch around toolchain, compiler, or typing friction with silent workarounds (e.g. hacky `# type: ignore`, lossy `.ok()`, special-case branches, or unverified casts). Surface the root cause and resolve it cleanly; log tooling friction or upstream gaps explicitly.

## Reasoning & judgment

- **Intent over letter**: Identify architectural purpose. Flag when literal compliance defeats intent or violates compiler invariants.
- **Competing interpretations & tensions**: Surface ambiguity, trade-offs, and compiler vs. performance tensions before executing.
- **Second-order effects**: Trace downstream consequences across passes before editing.
- **Calibrated confidence & disconfirmation**: Separate facts from assumptions; actively test edge and boundary invariants.
- **Precision over hedging**: Nuance ≠ verbosity. Specific technical caveats beat generic disclaimers.

## Spec-driven development & subagent lifecycle

- **Proportional scope**: Governed by `docs/specs/sdd-guidelines.md`.
  - **Full Tri-Agent SDD**: Mandatory for cross-crate features, compiler pass changes, new DSL/IR semantics, and multi-step refactors.
  - **Direct execution**: Bug fixes, localized ergonomic tweaks, documentation/skill updates, and test additions execute directly without draft specs or subagent dispatch.
- **Tri-agent roles**:
  - **Lead Orchestrator**: Aligns design, authors ephemeral draft specs, dispatches subagents, folds durable changes into living reference docs, and deletes completed draft specs.
  - **Implementer**: Builds strictly to draft spec contracts and satisfies verification gates.
  - **Reviewer**: Adversarially audits diff against spec and checklist in report-only mode using `review-implementation` skill.
- **Spec custody**: Draft spec lifecycle (`docs/specs/drafts/`) is exclusively managed by the Lead Orchestrator. Subagents never edit, move, or delete specs directly.

## Code standards

### Rust standards (Rust 2024 edition)

- **Immutability by default**: Immutable `let` bindings; `let mut` only when mutation is strictly required.
- **Zero production panics**: Enforced via `clippy::unwrap_used = "deny"` and `clippy::expect_used = "deny"`. Propagate via `?`. Allowed only in `#[cfg(test)]` or `tests/`.
- **Structured errors**: Use `thiserror` for compiler diagnostics and domain errors.
- **Deterministic ordering**: Use `IndexMap` / `IndexSet` over standard `HashMap` / `HashSet` for layout, IR, or serialization.
- **Memory efficiency**: Borrowed references (`&str`, `&[T]`) or `Cow` over unnecessary cloning in compiler passes.
- **Guard clauses**: Early returns over deep branching nesting.
- **Verification suite**:
  - `cargo test`: Unit and integration test suite.
  - `cargo clippy --all-targets -- -D warnings`: Compilation check, zero warnings, and no unhandled panics.
  - `cargo fmt --check`: Formatting adherence.

### Python standards (Python 3.12+)

- **Strict typing**: 100% typed (PEP 604 `X | Y`, built-in generics, explicit return `-> None`). Zero bare `Any` or unjustified `# type: ignore`.
- **Data contracts**: Explicit `TypedDict` for dict/JSON payloads and compiler outputs over `dict[str, Any]`.
- **Native stubs**: Synchronized `.pyi` type stubs for `f8._core` with typed exceptions; maintain PEP 561 `py.typed`.
- **Exception hygiene**: Zero bare `except:`. Structured errors inheriting from `F8Error` with typed attributes (coordinates, cycles, tables). Use `raise ... from err` when wrapping native errors.
- **GIL & buffers**: Release GIL (`py.allow_threads`) during CPU-bound tasks in PyO3; use `memoryview`/`bytes` buffer protocols.
- **Predictable defaults**: Ban mutable defaults (`def f(x=[])`). Immutable `@dataclass(frozen=True)` or read-only dicts.
- **Verification suite**:
  - `uv run ruff check .`: Zero warnings (`E`, `F`, `I`, `UP`, `B`, `SIM`).
  - `uv run ruff format --check .`: Code formatting adherence.
  - `uv run maturin develop`: Recompile native extension after Rust binding changes.
  - `uv run pyright`: Static type checking in strict mode with zero errors/warnings.
  - `uv run pytest`: Unit and integration test suite.

## Naming conventions

- **Rust**: `snake_case` (files, functions, modules, variables), `PascalCase` (structs, enums, traits), `SCREAMING_SNAKE_CASE` (constants).
- **Python**: `snake_case` (files, packages, functions, variables), `PascalCase` (classes, exceptions), `_snake_case` (native/internal modules), `SCREAMING_SNAKE_CASE` (constants).
- **Docs & specs**: `kebab-case.md` for filenames; prefix ephemeral task specs with `YYYY-MM-DD-<feature-name>.md` under `docs/specs/drafts/`.

## Writing style

Applies to documentation, specs, docstrings, and commit messages.

- **Telegraphic and high-density**:
  - Prioritize conciseness, density, and clarity over conversational narrative.
  - Favor bulleted lists, sentence fragments, structured tables.
  - Eliminate conversational pleasantries, introductory filler, and redundant conclusions.
  - Sentence case for list items and bold prefix labels.
- **Cross-references**:
  - Relative links exclusively for stable reference documentation (`docs/`).
  - Code spans (e.g. `crates/f8-core/src/compiler/mod.rs`) in draft specs to avoid link bit-rot.

## Git conventions

- **Conventional commits**: Follow `<type>(<scope>): <summary>`.
  - Types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`.
  - Format: Imperative mood, lowercase after colon, no trailing period.
  - Body: Blank line before body. Itemize changes with contiguous hyphen bullets (`- `) and zero empty lines between bullets.
