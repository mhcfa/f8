# Agent Guidelines

## Architecture & workspace

- **`crates/f8-core/`**: Rust calculation engine. 6 compiler passes (IR, DAG/SCC, evaluator, layout, lowering, emitter) and `CompiledModel` facade.
- **`crates/f8-python/`**: PyO3 native extension (`f8._core`, `abi3-py312`). Deserializes via `pythonize`, bridges `CompiledModel`, maps typed exceptions, and releases GIL on CPU tasks.
- **`python/f8/`**: Python SDK (`Model`, `Table`, `Cell`, `TableAuditor`), project disk I/O, synchronized `.pyi` stubs (`_core.pyi`), and bundled `f8-modeling` skill.
- **`docs/`**: Reference docs (`architecture.md`, `dsl/`, `functions/catalog.md`) and active task specs (`docs/specs/drafts/`).

## Workflow

- **Full Tri-Agent SDD**: Invoke `spec-driven-development` skill for cross-crate features, compiler pass changes, new DSL/IR semantics, and multi-step refactors.
- **Direct execution**: Execute bug fixes, localized ergonomic tweaks, doc/skill updates, and test additions directly without draft specs or subagent dispatch.
- **Design alignment**: Surface architectural ambiguities, cross-pass impacts, and compiler vs. performance trade-offs before coding.

## Core principles

- **Single source of truth**: Centralize shared definitions; zero logic or type duplication across crates or passes.
- **Predictable core IR**: One canonical representation per concept. Isolate syntactic sugar and ergonomics to outer adapters/DSL.
- **Pure data flow**: Explicit inputs and outputs; zero global mutable state or hidden side effects.
- **Strong typing**: Explicit enums, interned IDs, and typed structs; ban stringly-typed heuristics.
- **Root-cause correctness & zero silent workarounds**: Fix defects at parser, IR, or schema origin. Never patch around compiler, typing, or toolchain friction with downstream band-aids, `# type: ignore`, lossy `.ok()`, or unverified casts.
- **Greenfield rigor**: Ban compatibility shims, `#[deprecated]` aliases, dead code, commented-out tombstones, premature abstractions, and `todo!()`/`unimplemented!()` stubs. Update callers in-place.

## Code standards & verification

### Rust (2024 edition)

- **Zero production panics**: `unwrap_used` and `expect_used` are denied. Use `.get()` / pattern matching and propagate via `?` (`unwrap`/`expect` allowed only in tests).
- **Structured errors**: `thiserror` for compiler diagnostics and domain errors.
- **Deterministic ordering**: `IndexMap` / `IndexSet` over `HashMap` / `HashSet` wherever layout, IR, or serialization order matters.
- **Memory efficiency**: Borrowed slices (`&str`, `&[T]`) or `Cow` over unnecessary cloning in compiler passes.
- **Verification**:
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`

### Python (3.12+)

- **Strict typing**: 100% typed (`pyright` strict, explicit `-> None`, `TypedDict` over `dict[str, Any]`). Zero bare `Any`.
- **Native stubs**: Keep `python/f8/_core.pyi` and `py.typed` synchronized with `crates/f8-python`.
- **Exception hygiene**: Zero bare `except:`. Inherit from `F8Error` with typed attributes (coordinates, cycles, tables); wrap native errors via `raise ... from err`.
- **GIL & buffers**: Release GIL (`py.allow_threads`) on CPU-bound PyO3 tasks; prefer `memoryview`/`bytes` buffer protocols.
- **Verification** (when touching `crates/f8-python` or `python/`):
  - `uv run ruff check .` && `uv run ruff format --check .`
  - `uv run maturin develop` (after Rust binding changes)
  - `uv run pyright`
  - `uv run pytest`

## Writing & git conventions

- **Telegraphic style**: Concise, high-density prose across docs, specs, docstrings, and commits. Favor bulleted lists with bold prefix labels and structured tables; omit filler.
- **Conventional commits**: `<type>(<scope>): <summary>` (`feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`), imperative mood, lowercase summary, no trailing period, followed by a blank line and contiguous `- ` bullets.
