# Implementation Review Checklist

Audit diff against categories relevant to changed files. Default severity in brackets; escalate when impact warrants.

## 1. Compiler Invariants & Architecture

- [ ] **Single source of truth (SSOT)**: Domain models, identifiers, coordinate mappings, and error definitions have exactly one authoritative source. Zero duplicated data structures across compiler phases. [`Blocker`]
- [ ] **Clean phase boundaries**: Compiler phases remain cleanly decoupled (Parser -> AST -> Lowering -> IR -> DAG -> Layout -> Emission / Evaluator). Higher-level domain logic never leaks into lower-level IR primitives. [`Blocker`]
- [ ] **Predictable core IR**: Exactly one canonical representation per concept. Zero magic fallbacks or core syntactic sugar; convenience is isolated to outer adapters/DSL. [`Blocker`]
- [ ] **Strong typing over string heuristics**: Explicit enum variants, interned symbol IDs, or dedicated AST/IR node fields. Zero string guessing, regex inspection, or parsing serialized strings to infer semantics. [`Blocker`]
- [ ] **Pure data flow**: Pure functions with explicit inputs and outputs over hidden or shared mutable state. Functions focused on a single responsibility. [`Major`]
- [ ] **Root-cause correctness**: Fix defects at origin (parser, IR, or schema). Zero downstream band-aids, tactical workarounds, or test-specific branches designed merely to make a test pass. [`Blocker`]

## 2. Greenfield Rigor

- [ ] **No compatibility shims**: Zero `#[deprecated]` attributes, legacy re-exports, alias types, or transition wrappers. Callers updated in-place across repository. [`Blocker`]
- [ ] **No dead code or tombstones**: Zero commented-out code, unused functions, obsolete trait methods, abandoned match arms, or unused imports. [`Minor`]
- [ ] **No speculative code (YAGNI)**: Zero premature trait abstractions, unused generics, or configuration options with only one concrete usage. [`Minor`]
- [ ] **No placeholder stubs**: Zero `todo!()`, `unimplemented!()`, or placeholder structs/functions left in production paths. [`Blocker`]
- [ ] **Zero silent workarounds**: Zero unlogged hacks, temporary skips, or suppressed warnings around toolchain or compiler friction. [`Blocker`]

## 3. Rust Standards (Rust 2024 Edition)

- [ ] **Zero production panics**: Zero calls to `.unwrap()` or `.expect()` in non-test code (`clippy::unwrap_used` and `clippy::expect_used`). Slice and collection indexing uses `.get()` or pattern matching to eliminate out-of-bounds panics. [`Blocker`]
- [ ] **Structured error handling**: Errors defined with `thiserror` and propagated cleanly with `?`. Zero stringly-typed error messages or discarding error context with `.ok()`. [`Major`]
- [ ] **Deterministic ordering**: `IndexMap` and `IndexSet` used instead of standard `HashMap` and `HashSet` wherever iteration order, layout, IR, or serialization determinism matters. [`Major`]
- [ ] **Memory efficiency**: Borrowed references (`&str`, `&[T]`) or `Cow` preferred over unneeded `.clone()`, `.to_vec()`, or `.to_string()`. [`Minor`]
- [ ] **Immutability by default**: Immutable `let` bindings; `let mut` only when mutation is strictly required. [`Minor`]
- [ ] **Guard clauses**: Early returns and guard clauses used to prevent deeply nested `if/else` ladders. [`Nit`]

## 4. Python Standards (Python 3.12+, when touching `crates/f8-python` or `python/`)

- [ ] **100% strict type annotations**: All functions, methods, and fixtures have complete type hints (including explicit return `-> None`). Zero bare `Any` escapes or unjustified `# type: ignore` / `# pyright: ignore`. [`Major`]
- [ ] **Data contracts**: Explicit `TypedDict` contracts used for dict/JSON payloads and compiler outputs instead of untyped `dict[str, Any]`. [`Major`]
- [ ] **Native stub parity**: PyO3 bindings in `crates/f8-python` have corresponding synchronized definitions in `python/f8/_core.pyi` with `TypedDict` contracts and typed exception signatures; PEP 561 `python/f8/py.typed` preserved. [`Blocker`]
- [ ] **Exception hygiene**: Zero bare `except:` or broad `except Exception:` swallows errors. Exceptions inherit from `F8Error` and carry structured diagnostic attributes (coordinates, cycles, tables) instead of requiring string parsing. [`Blocker`]
- [ ] **GIL release & buffer protocols**: CPU-bound operations in native bindings release Python GIL (`py.allow_threads`) and utilize zero-copy buffer protocols (`memoryview`, `bytes`). [`Blocker`]
- [ ] **Predictable defaults**: Zero mutable defaults (`def f(x=[])`). Models use immutable `@dataclass(frozen=True)` or read-only containers. [`Major`]
