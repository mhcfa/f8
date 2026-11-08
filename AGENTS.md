# Agent Guidelines

## Architecture & workspace

- `docs/architecture.md` & `README.md`: Repository architecture, module boundaries, domain invariants, packaged skills, and CLI/API entrypoints. Read `docs/architecture.md` before planning or modifying code.
- `docs/specs/drafts/`: Ephemeral task specs for active work.
- `.agents/skills/`: Bundled agent skills and optional `.agents/skills/review-implementation/references/repo-checks.md` for repository-specific review checks.

## Rule vocabulary

- `Never` / `Must`: Hard rule. No exceptions without explicit user approval. Tool-enforced where possible.
- `Prefer` / `Default to` / unqualified principle: Judgment call. Deviate only with a stated reason; surface the trade-off instead of choosing silently.

## Workflow

- Spec-driven: Invoke `spec-driven-development` skill when a change alters a contract (schemas, IR/DSL semantics, pass or module boundaries, public CLI/SDK/API surface) or needs more than one independently verifiable increment.
- Direct execution: Everything else (localized bug fixes, ergonomic tweaks, doc/skill updates, test additions). No draft spec or subagent dispatch. Escalate to spec-driven if a direct fix turns out to change a contract.
- Design alignment: Surface architectural ambiguities, cross-boundary impacts, and trade-offs before coding.

## Hard rules

- Never suppress tooling to get green (`#[allow(...)]`, `# type: ignore`, `# pyright: ignore`, `# noqa`, `eslint-disable`, `oxlint-disable`, `@ts-ignore`, `@ts-expect-error`) without user approval and an inline reason.
- Never add compatibility shims (`#[deprecated]`, `@deprecated`, transition wrappers, old-name re-exports, `_v2` functions, or placeholder stubs). Update callers in place. Public Python (`__all__`) deprecation warnings only when the user or spec requires them.
- Never use `unwrap` / `expect` outside tests (clippy `unwrap_used`, `expect_used`), bare `except:` (ruff `E722`), or silently swallow exceptions.
- Never leave `todo!()`, `unimplemented!()`, or `dbg!()` in code (clippy `todo`, `unimplemented`, `dbg_macro`).
- Never use glob imports outside tests (clippy `wildcard_imports`).
- Never use wildcard arms (`_ =>`) when matching AST, IR, or domain enums in compiler passes, emitters, or PyO3 bridges. Why: adding a variant must fail compilation at every match site.
- Never use mutable default arguments or hidden global mutable state.
- Never manually edit generated files (e.g., `routeTree.gen.ts`); regenerate via workspace scripts.
- Must pass the applicable verification suite (below) before reporting work complete.

## Core principles

- Single source of truth: Define each concept (type, ID, schema, coordinate mapping, error) once; import it everywhere else. Why: forked definitions drift silently across modules and passes. Duplicated knowledge is a defect; incidental code similarity is not.
- Canonical contracts & IR: One representation per concept in core modules, IR, and schemas; no implicit fallbacks, untyped dicts, or syntactic sugar. Ergonomics live in outer adapters (presets, Python SDK, UI wrappers, inspection facades). Why: each alternative form multiplies cases downstream.
- Clean boundaries: Unidirectional dependency graph. Each stage, pass, or module consumes only upstream outputs; shared and foundational modules stay out of higher-level presets and application layers; no filesystem I/O in core compiler crates.
- Pure data flow: Explicit inputs, outputs, and boundary-driven state transitions; no global mutable state or hidden side effects. Why: determinism and isolated testability.
- Invalid states unrepresentable: Prefer enums, `Literal` types, interned IDs, newtypes, frozen dataclasses, Pydantic/Zod schemas, and typed structs over string heuristics and `bool` flags; 2-variant enum/union over `bool` parameter. No string inspection to infer semantics outside lexer/parser/ingest boundaries. Why: the compiler or type checker checks semantics instead of the reviewer.
- Concrete internals, ergonomic boundaries: Internal passes take concrete types (`&str`, `&[T]`); generic conversions (`impl Into`, `impl AsRef`) only at public facades (PyO3 bindings, inspection facades).
- Fix at the origin: Fix defects where they arise (parser, ingest, IR, layout solver, schema, query), not where they surface. No downstream band-aids, lossy `.ok()`, unverified casts, or test-specific branches. Why: downstream patches mask the bug and multiply.
- Minimal surface: No dead code, commented-out tombstones, or speculative abstractions. Abstract on the second real caller, not the first.

## Reasoning & judgment

- Intent over letter: Identify the purpose behind each request, rule, and spec. Flag when literal compliance defeats intent.
- Tension surfacing: Name conflicts between instructions, specs, and invariants explicitly. Never silently pick a side.
- Disconfirmation: Before concluding, test the strongest alternative design or root cause. Passing tests do not guarantee a sound contract.
- Precision over hedging: Separate verified facts, inferences, and assumptions. One specific caveat beats generic disclaimers.

## Code standards & verification

### Python (3.12+)

- Strict typing: `pyright` strict, explicit `-> None`, `TypedDict` / frozen dataclasses / Pydantic models over `dict[str, Any]`, no bare `Any`.
- Exception hygiene: No bare `except:` (ruff `E722`) or swallowed `Exception`; structured diagnostics with context attributes; preserve cause via `raise ... from err`.

### Rust & PyO3 (2024 edition)

- Panic-free production paths: Prefer `.get()` / pattern matching over indexing; propagate via `?`.
- Structured errors: `thiserror` for compiler diagnostics and domain errors.
- Deterministic output: `IndexMap` / `IndexSet` over `HashMap` / `HashSet` wherever iteration, layout, IR, or serialization order matters. Identical input must yield identical bytes.
- Memory efficiency: Prefer borrowed slices (`&str`, `&[T]`) or `Cow` over cloning in compiler passes.
- Import order: Standard library (`std::*`), external crates, crate-local modules.
- PyO3 bridges & stubs: Keep `_core.pyi` synchronized with PyO3 crate exports and exceptions; keep `py.typed` marker present. Release GIL (`py.allow_threads` / `py.detach`) on CPU-bound tasks; prefer `memoryview` / `bytes` buffer protocols.

### TypeScript & React

- Strict module & type hygiene: Enforce `import type` for type-only imports (`verbatimModuleSyntax`), zero unused locals/parameters, and zero floating or misused promises (`oxlint` type-aware).
- Boundary validation: Parse external inputs, search params, and network payloads via `zod` schemas rather than unverified type assertions (`as`).
- React 19 & styling: Pass `ref` directly as a prop (`React.RefAttributes<T>`); never wrap components in legacy `React.forwardRef`. Compose component variants with `cva` and merge classes with `cn`; use semantic theme tokens over hardcoded values.

### Verification suite

- Python (any Python change; run `uv run maturin develop` first in PyO3 repos after any Rust change):
  - `uv run ruff check .` && `uv run ruff format --check .`
  - `uv run pyright`
  - `uv run pytest`
- Rust (any Rust change):
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
- TypeScript & React (any TypeScript/UI change):
  - `pnpm typecheck`
  - `pnpm lint`
  - `pnpm format:check`
  - `pnpm build`
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
  - Scopes: Target package, crate, module, or domain (e.g., `core`, `compiler`, `schemas`, `python`, `sdk`, `web`, `ui`, `cli`, `sync`, `skill`, `docs`, `config`, `tests`).
  - Format: Imperative mood, lowercase, no trailing period. Blank line before body. Contiguous hyphen bullets (`- `).
