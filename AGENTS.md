# Agent Guidelines

## Architecture & workspace

- `crates/f8-core/`: Rust calculation engine. 6 compiler passes (IR, DAG/SCC, evaluator, layout, lowering, emitter) and `CompiledModel` facade.
- `crates/f8-python/`: PyO3 native extension (`f8._core`, `abi3-py312`). Deserializes via `pythonize`, bridges `CompiledModel`, maps typed exceptions, and releases GIL on CPU tasks.
- `python/f8/`: Python SDK (`Model`, `Table`, `Cell`, `TableAuditor`), synchronized `.pyi` stubs (`_core.pyi`), and bundled `f8-modeling` skill.
- `docs/`: Architecture overview (`docs/architecture.md`) and active task specs (`docs/specs/drafts/`).

## Workflow

- Full Tri-Agent SDD: Invoke `spec-driven-development` skill for cross-crate features, compiler pass changes, new DSL/IR semantics, and multi-step refactors.
- Direct execution: Execute bug fixes, localized ergonomic tweaks, doc/skill updates, and test additions directly without draft specs or subagent dispatch.
- Design alignment: Surface architectural ambiguities, cross-pass impacts, and compiler vs. performance trade-offs before coding.

## Core principles

- Single source of truth: Centralize shared definitions; zero logic or type duplication across crates or passes.
- Predictable core IR: Strictly one canonical representation per concept. Zero magic fallbacks or syntactic sugar in core; isolate ergonomics and convenience to outer adapters.
- Pure data flow: Explicit inputs and outputs; zero global mutable state or hidden side effects.
- Strong typing: Explicit enums, interned IDs, and typed structs; ban stringly-typed heuristics and boolean configuration/behavior toggles (use explicit 2-variant enums).
- Boundary vs. internal signature separation: Keep internal compiler passes concrete (`&str`, `&[T]`) without generic conversions (`impl Into`, `impl AsRef`); isolate ergonomic conversions exclusively to public facade adapters (`f8-python`, `CompiledModel`).
- Root-cause correctness & zero silent workarounds: Fix defects at parser, IR, or schema origin. Never patch around compiler, typing, or toolchain friction with downstream band-aids, `# type: ignore`, lossy `.ok()`, or unverified casts.
- Zero unrequested shims: No `#[deprecated]`, compatibility wrappers, dead code, commented-out tombstones, premature abstractions, or `todo!()`/`unimplemented!()` stubs. Update callers in-place; only add public Python (`f8.__all__`) deprecation warnings when explicitly required by the user or spec, keeping `f8-core` and `f8-python` strictly shim-free.

## Code standards & verification

### Rust (2024 edition)

- Zero production panics: `unwrap_used` and `expect_used` are denied. Use `.get()` / pattern matching and propagate via `?` (`unwrap`/`expect` allowed only in tests).
- Structured errors: `thiserror` for compiler diagnostics and domain errors.
- Deterministic ordering: `IndexMap` / `IndexSet` over `HashMap` / `HashSet` wherever layout, IR, or serialization order matters.
- Memory efficiency: Borrowed slices (`&str`, `&[T]`) or `Cow` over unnecessary cloning in compiler passes.
- Import discipline: Ban glob imports (`use foo::*;`) in non-test code (`wildcard_imports = "deny"`). Deterministic import order: standard library (`std::*`), external crates, crate-local modules.
- Exhaustive pattern matching: Match AST, IR, and domain enums explicitly; ban wildcard catch-alls (`_ =>`) in compiler passes and PyO3 bridges so adding variants triggers compile-time exhaustiveness checks across the workspace.
- Verification:
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`

### Python (3.12+)

- Strict typing: 100% typed (`pyright` strict, explicit `-> None`, `TypedDict` over `dict[str, Any]`). Zero bare `Any`.
- Native stubs: Keep `python/f8/_core.pyi` and `py.typed` synchronized with `crates/f8-python`.
- Exception hygiene: Zero bare `except:`. Inherit from `F8Error` with typed attributes (coordinates, cycles, tables); wrap native errors via `raise ... from err`.
- GIL & buffers: Release GIL (`py.allow_threads`) on CPU-bound PyO3 tasks; prefer `memoryview`/`bytes` buffer protocols.
- Verification (when touching `crates/f8-python` or `python/`):
  - `uv run ruff check .` && `uv run ruff format --check .`
  - `uv run maturin develop` (after Rust binding changes)
  - `uv run pyright`
  - `uv run pytest`

## Writing style

Applies to documentation, specs, docstrings, and commit messages.

- Telegraphic and high-density:
  - Prioritize conciseness, density, and clarity over conversational grammar or narrative prose.
  - Favor bulleted lists, sentence fragments, structured tables.
  - Eliminate conversational pleasantries, introductory filler, and redundant conclusions.
  - Sentence case for list items and plain `Key: value` prefix labels.
- No document hyperlinks: Reference workspace documents and code files by backticked filename.
- No LaTeX math notation: Avoid `$$...$$`, `$...$`, `\frac`, or `\text` notation across all specifications and documentation unless specifically requested.

## Git conventions

- Conventional commits: Follow `<type>(<scope>): <summary>`.
  - Types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`.
  - Scopes: `cli`, `schemas`, `templates`, `tests`, `docs`, `config`, `ingest`.
  - Format: Imperative mood, lowercase, no trailing period. Blank line before body. Contiguous hyphen bullets (`- `).
