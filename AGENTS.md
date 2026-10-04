# Agent Guidelines

Guidelines for AI agents working with the f8 compiler, runtime, and language workspace.

> [!IMPORTANT]
> **Polyglot workspace**: Core IR, compiler, and CLI in Rust 2024; native bindings in PyO3; Python 3.12+ high-level package.
> **Spec-driven**: Invoke [spec-driven development workflow and guidelines](specs/sdd-guidelines.md) for non-trivial features or refactoring.

## Repository context & architecture

| Path                | Layer / Target      | Purpose & architectural rules                                                                                                                                                 |
| :------------------ | :------------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/f8-core/`   | Core engine (Rust)  | Pure in-memory calculation core: 6 compiler passes (IR, DAG/SCC, evaluator, layout, lowering, emitter) and `CompiledModel` facade.                                            |
| `crates/f8-cli/`    | CLI binary (Rust)   | Standalone CLI binary (`f8`). Subcommands (`init`, `check`, `compile`, `inspect`), disk filesystem I/O, terminal renderer.                                                    |
| `crates/f8-python/` | Native C-ABI (PyO3) | Native C-extension (`f8._core`, `abi3-py312`). Deserializes inputs via `pythonize`, bridges `CompiledModel`, maps typed exceptions, and releases GIL on CPU tasks.            |
| `python/f8/`        | High-level SDK (Py) | Fluent modeling DSL (`Model`, `Table`, `Cell`), project disk I/O, auditing facade (`TableAuditor`), synchronized stubs (`.pyi`), `py.typed`, and bundled `f8-modeling` skill. |
| `python/tests/`     | Test suite (Pytest) | End-to-end integration suite for Python DSL, PyO3 bindings, project I/O, inspection audit, and Excel calculation parity.                                                      |
| `.agents/skills/`   | Agent workflows     | Git-tracked agent skills and verification routines.                                                                                                                           |
| `specs/`            | SDD specifications  | Spec-driven development: SDD guidelines, permanent living specs (`living-specs/`), active RFCs (`drafts/`), and archived milestones (`history/`).                             |
| `docs/`             | Durable docs        | Product architecture overview, 2D matrix DSL design, and Excel standard function catalog.                                                                                     |

## Development principles

- **Single source of truth**: Centralize shared definitions. Zero logic duplication across crates or modules.
- **Predictable core IR**: Strictly one canonical representation per concept. Zero magic fallbacks or core syntactic sugar; isolate ergonomics and convenience to outer adapters.
- **Pure data flow**: Pure functions with explicit inputs and outputs. Zero global mutable state or implicit side effects.
- **Root-cause correctness**: Fix defects at parser, IR, or schema origin. Zero downstream band-aids or test overrides.
- **Strong typing**: Model domain concepts with explicit enums and typed structs. Ban stringly-typed heuristics.
- **Pragmatic simplicity**: Direct, idiomatic Rust and Python. Ban premature abstractions or speculative generalizations.

## Code standards

### Rust standards (Rust 2024 edition)

- **Immutability by default**: Immutable `let` bindings; `let mut` only when mutation is strictly required.
- **Zero production panics**: Enforced via `clippy::unwrap_used = "deny"` and `clippy::expect_used = "deny"`. Propagate via `?`. Allowed only in `#[cfg(test)]` or `tests/`.
- **Structured errors**: Use `thiserror` for compiler diagnostics and domain errors.
- **Deterministic ordering**: Use `IndexMap` / `IndexSet` over standard `HashMap` / `HashSet` for layout, IR, or serialization.
- **Memory efficiency**: Borrowed references (`&str`, `&[T]`) or `Cow` over unnecessary cloning in compiler passes.
- **Guard clauses**: Early returns over deep branching nesting.
- **Verification suite**:
  - `cargo check`: Quick compilation check.
  - `cargo test`: Unit and integration test suite.
  - `cargo clippy --all-targets -- -D warnings`: Zero warnings and no unhandled panics.
  - `cargo fmt --check`: Formatting adherence.

### Python standards (Python 3.12+)

- **Strict typing**: 100% typed (PEP 604 `X | Y`, built-in generics, explicit return `-> None`). Zero bare `Any` or unjustified `# type: ignore`.
- **Data contracts**: Explicit `TypedDict` for dict/JSON payloads and compiler outputs over `dict[str, Any]`.
- **Native stubs**: Synchronized `.pyi` type stubs for `f8._core` with typed exceptions; maintain PEP 561 `py.typed`.
- **Exception hygiene**: Zero bare `except:`. Structured errors inheriting from `F8Error` with typed attributes (coordinates, cycles, tables). Use `raise ... from err` when wrapping native errors.
- **GIL & buffers**: Release GIL (`py.allow_threads`) during CPU-bound tasks in PyO3; use `memoryview`/`bytes` buffer protocols.
- **Predictable defaults**: Ban mutable defaults (`def f(x=[])`). Immutable `@dataclass(frozen=True)` or read-only dicts.
- **Verification suite**:
  - `uv run pytest`: Unit and integration test suite.
  - `uv run ruff check .`: Zero warnings (`E`, `F`, `I`, `UP`, `B`, `SIM`).
  - `uv run ruff format --check .`: Code formatting adherence.
  - `uv run pyright`: Static type checking in strict mode with zero errors/warnings.
  - `uv run maturin develop`: Recompile native extension after Rust binding changes.

## Naming conventions

- **Rust**: `snake_case` (files, functions, modules, variables), `PascalCase` (structs, enums, traits), `SCREAMING_SNAKE_CASE` (constants).
- **Python**: `snake_case` (files, packages, functions, variables), `PascalCase` (classes, exceptions), `_snake_case` (native/internal modules), `SCREAMING_SNAKE_CASE` (constants).
- **Specs**: `kebab-case.md` for filenames; group living specs under `specs/living-specs/<domain>/<topic>.md`.

## Writing style

Applies universally across documentation, specs, docstrings, and commit messages.

- **Telegraphic and high-density**:
  - Prioritize conciseness, density, and clarity over conversational narrative.
  - Favor bulleted lists, sentence fragments, structured tables.
  - Eliminate conversational pleasantries, introductory filler, and redundant conclusions.
  - Sentence case for list items and bold prefix labels.
- **Cross-references**:
  - Relative links exclusively for living specs (`specs/living-specs/`) and stable documentation.
  - Code spans (e.g. `crates/f8-core/src/compiler/mod.rs`) in draft specs and archives to avoid link bit-rot.

## Git conventions

- **Conventional commits**: Follow `<type>(<scope>): <summary>`.
  - Types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`.
  - Format: Imperative mood, lowercase after colon, no trailing period.
  - Body: Blank line before body. Itemize changes with contiguous hyphen bullets (`- `) and zero empty lines between bullets.
