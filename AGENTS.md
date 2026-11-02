# Agent Guidelines

## Architecture & workspace

- `crates/f8-core/`: Rust calculation engine. 6-pass compiler pipeline (see `docs/architecture.md`) and `CompiledModel` facade.
- `crates/f8-python/`: PyO3 native extension (`f8._core`, `abi3-py312`). Deserializes via `pythonize`, bridges `CompiledModel`, maps typed exceptions, and releases GIL on CPU tasks.
- `python/f8/`: Python SDK (`Model`, `Sheet`, `Table`, `Cell`, `TableAuditor`), synchronized `.pyi` stubs (`_core.pyi`), and bundled `f8-modeling` skill.
- `docs/`: Architecture overview (`docs/architecture.md`) and active task specs (`docs/specs/drafts/`).

## Rule vocabulary

- `Never` / `Must`: Hard rule. No exceptions without explicit user approval. Tool-enforced where possible.
- `Prefer` / `Default to` / unqualified principle: Judgment call. Deviate only with a stated reason; surface the trade-off instead of choosing silently.

## Workflow

- Spec-driven: Invoke `spec-driven-development` skill when a change alters a contract (IR/DSL semantics, pass interfaces, public Python API) or needs more than one independently verifiable increment.
- Direct execution: Everything else (bug fixes contained in one pass, ergonomic tweaks, doc/skill updates, test additions). No draft spec or subagent dispatch. Escalate to spec-driven if a direct fix turns out to change a contract.
- Design alignment: Surface architectural ambiguities, cross-pass impacts, and compiler vs. performance trade-offs before coding.

## Hard rules

- Never use `unwrap` / `expect` outside tests (clippy `unwrap_used`, `expect_used`).
- Never use glob imports outside tests (clippy `wildcard_imports`).
- Never leave `todo!()`, `unimplemented!()`, or `dbg!()` in code (clippy `todo`, `unimplemented`, `dbg_macro`).
- Never suppress tooling to get green (`#[allow(...)]`, `# type: ignore`, `# pyright: ignore`, `# noqa`) without user approval and an inline reason (clippy `allow_attributes_without_reason`).
- Never use wildcard arms (`_ =>`) when matching AST, IR, or domain enums in compiler passes or PyO3 bridges. Why: adding a variant must fail compilation at every match site.
- Never add compatibility shims in `f8-core` or `f8-python` (`#[deprecated]`, transition wrappers, old-name re-exports). Update callers in place. Public Python (`f8.__all__`) deprecation warnings only when the user or spec requires them.
- Must pass the verification suite (below) before reporting work complete.

## Core principles

- Single source of truth: Define each concept (type, ID, coordinate mapping, error) once; import it everywhere else. Why: forked definitions drift silently across passes. Duplicated knowledge is a defect; incidental code similarity is not.
- Canonical core IR: One representation per concept in core; no implicit fallbacks or syntactic sugar. Ergonomics live in outer adapters (Python SDK, `CompiledModel`). Why: each alternative form multiplies cases in every downstream pass.
- Clean phase boundaries: Each pass consumes only upstream pass outputs; higher-level domain logic stays out of low-level IR primitives; no filesystem I/O in `f8-core`.
- Pure data flow: Explicit inputs and outputs; no global mutable state or hidden side effects. Why: determinism and per-pass testability.
- Invalid states unrepresentable: Prefer enums, interned IDs, and typed structs over strings and `bool` flags; 2-variant enum over `bool` parameter. No string inspection to infer semantics outside the lexer/parser. Why: the compiler checks semantics instead of the reviewer.
- Concrete internals, ergonomic boundaries: Internal passes take concrete types (`&str`, `&[T]`); generic conversions (`impl Into`, `impl AsRef`) only at public facades (`f8-python`, `CompiledModel`).
- Fix at the origin: Fix defects where they arise (parser, IR, schema), not where they surface. No downstream band-aids, lossy `.ok()`, unverified casts, or test-specific branches. Why: downstream patches mask the bug and multiply.
- Minimal surface: No dead code, commented-out tombstones, or speculative abstractions. Abstract on the second real caller, not the first.

## Code standards & verification

### Rust (2024 edition)

- Panic-free production paths: Prefer `.get()` / pattern matching over indexing; propagate via `?`.
- Structured errors: `thiserror` for compiler diagnostics and domain errors.
- Deterministic output: `IndexMap` / `IndexSet` over `HashMap` / `HashSet` wherever iteration, layout, IR, or serialization order matters. Identical input must yield identical bytes.
- Memory efficiency: Prefer borrowed slices (`&str`, `&[T]`) or `Cow` over cloning in compiler passes.
- Import order: Standard library (`std::*`), external crates, crate-local modules.

### Python (3.12+)

- Strict typing: `pyright` strict, explicit `-> None`, `TypedDict` over `dict[str, Any]`, no bare `Any`.
- Native stubs: Keep `python/f8/_core.pyi` synchronized with `crates/f8-python` exports and exceptions; keep `py.typed` marker present.
- Exception hygiene: No bare `except:` (ruff `E722`) or swallowed `Exception`. Inherit from `F8Error` with typed attributes (coordinates, cycles, tables); wrap native errors via `raise ... from err`.
- GIL & buffers: Release GIL (`py.allow_threads`) on CPU-bound PyO3 tasks; prefer `memoryview` / `bytes` buffer protocols.

### Verification suite

- Rust (any Rust change):
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
- Python (any Rust or `python/` change; core semantics reach Python through the bindings):
  - `uv run maturin develop` (first, after any Rust change)
  - `uv run ruff check .` && `uv run ruff format --check .`
  - `uv run pyright`
  - `uv run pytest`
- Docs/skill-only changes: No suite required.

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
  - Scopes: `core` (cross-pass `f8-core`), `compiler`, `expr`, `functions`, `schemas` (model spec / DSL schema), `python` (`f8-python` bindings), `sdk`, `cli`, `skill`, `docs`, `config`, `tests`.
  - Format: Imperative mood, lowercase, no trailing period. Blank line before body. Contiguous hyphen bullets (`- `).
