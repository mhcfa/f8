# Agent Guidelines

## Architecture & workspace

- `docs/architecture.md` & `README.md`: Module boundaries, domain invariants, packaged skills, and CLI/API entrypoints. Read `docs/architecture.md` before planning or modifying code.
- `docs/specs/drafts/`: Ephemeral task specs for active work.
- `.agents/skills/`: Bundled agent skills and optional `.agents/skills/review-implementation/references/repo-checks.md` for repository-specific review checks.

## Rule vocabulary

- `Never` / `Must`: Hard rule. No exceptions without explicit user approval.
- `Prefer` / `Default to` / unqualified principle: Judgment call. Deviate only with a stated reason; surface trade-offs instead of choosing silently.

## Workflow

- Spec-driven: Invoke `spec-driven-development` when a change alters or introduces a contract (schemas, AST/IR/DSL, module boundaries, public CLI/SDK/API), carries open architectural trade-offs, or spans multiple increments.
- Direct execution: Handle single-increment work within existing contracts (e.g., bug fixes, self-contained features, refactors, doc/skill updates) directly without draft specs or subagents. Escalate to spec-driven if a contract or boundary change emerges.
- Design alignment: Surface architectural ambiguities, conflicting instructions or invariants, and cases where literal compliance defeats intent before coding; never silently pick a side. Separate verified facts from inferences and assumptions.
- Testing: Every executable behavior change requires falsifiable tests that fail if the behavior regresses. For bug fixes, reproduce the defect with a failing test before changing production code to confirm root cause.

## Hard rules

- Never suppress tooling (`#[allow(...)]`, `# type: ignore`, `# pyright: ignore`, `# noqa`, `eslint-disable`, `oxlint-disable`, `@ts-ignore`, `@ts-expect-error`) without user approval and an inline reason.
- Never add compatibility shims (`#[deprecated]`, `@deprecated`, transition wrappers, old-name re-exports, `_v2` functions, or placeholder stubs). Update callers in place; add public Python (`__all__`) deprecations only when requested.
- Never patch symptoms downstream: fix defects at the origin (parser, ingest, IR, schema, solver). Never add input- or test-specific branches, silent fallback masks (permissive `.get()`, lossy `.ok()`), validation bypass flags (`skip_validation`, `force`), or single-use callsite patches.
- Never use `unwrap` / `expect` outside tests, bare `except:`, or silently swallowed exceptions.
- Never leave unfinished placeholders or debug macros (`todo!()`, `unimplemented!()`, `dbg!()`) or use glob imports outside tests.
- Never use wildcard arms (`_ =>`) when matching AST, IR, or domain enums in compiler passes, emitters, or PyO3 bridges, so new variants fail compilation at every match site.
- Must keep active draft specs in `docs/specs/drafts/` during post-implementation review and delete finished specs when preparing a commit.
- Must pass the applicable verification suite (below) before reporting work complete.

## Core principles

- Single source of truth: Define each concept (type, ID, schema, coordinate mapping, error) once and import it everywhere. Duplicated domain knowledge is a defect; incidental code similarity is not.
- Canonical contracts & IR: One representation per concept in core modules, IR, and schemas; no implicit fallbacks, untyped dicts, or syntactic sugar. Confine ergonomics to outer adapters (presets, SDKs, UI wrappers).
- Behavioral symmetry & uniformity: Enforce uniform validation strictness, error policies, and boundary semantics across sibling modules and entrypoints; handle variants and boundary sizes (`0`, `1`, `N`) via general logic without ad-hoc carve-outs unless a domain invariant requires divergence.
- Clean boundaries: Unidirectional dependencies. Modules and passes consume only upstream outputs; foundational modules stay free of higher-level presets, application layers, and unexpected I/O.
- Pure data flow: Explicit inputs and outputs; never use hidden side effects, global mutable state, or mutable default arguments.
- Invalid states unrepresentable: Prefer enums, `Literal` types, interned IDs, newtypes, frozen dataclasses, and Pydantic/Zod models over string heuristics and `bool` flags (`2`-variant enum/union over `bool` parameter). Never inspect strings to infer semantics outside lexer/parser/ingest boundaries.
- Consistent conventions: Match established naming, parameter order, CLI/API shapes, and test idioms across sibling modules (never by copying a rule violation).
- Minimal surface: No dead code, commented-out tombstones, single-use helper shims, or speculative abstractions. Abstract on the second real caller, not the first.

## Code standards & verification

### Python (3.12+)

- Strict typing: `pyright` strict, explicit `-> None`, `TypedDict` / frozen dataclasses / Pydantic models over `dict[str, Any]`, no bare `Any`.
- Exceptions: Structured diagnostics with context attributes; preserve cause via `raise ... from err`.

### Rust & PyO3 (2024 edition)

- Error & panic safety: `thiserror` diagnostics propagated via `?`; prefer `.get()` or pattern matching over indexing.
- Deterministic output: `IndexMap` / `IndexSet` over `HashMap` / `HashSet` wherever iteration, layout, IR, or serialization order matters; identical input must yield identical bytes.
- Signatures & style: Concrete types (`&str`, `&[T]`, `Cow`) over cloning in internal passes; `impl Into` / `impl AsRef` only at public facades. Group imports `std`, external, crate-local.
- PyO3 bridges & stubs: Keep `_core.pyi` synchronized with exports and exceptions (`py.typed` present), release GIL (`py.allow_threads` / `py.detach`) on CPU-bound work, and use `memoryview` / `bytes` buffer protocols for bulk data.

### TypeScript & React

- Strict module & promise hygiene: `import type` for type-only imports (`verbatimModuleSyntax`), zero unused locals/parameters, and zero floating or misused promises (`oxlint` type-aware).
- Boundary validation: Parse external inputs, search params, and network payloads via `zod` schemas rather than unverified type assertions (`as`).
- React 19 & styling: Pass `ref` directly as a prop (`React.RefAttributes<T>`) without `React.forwardRef`; compose variants with `cva`, merge classes with `cn`, and use semantic theme tokens over hardcoded values.

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

- Telegraphic & high-density: Favor bulleted lists, sentence fragments, structured tables, sentence case, and plain `Key: value` labels; omit narrative filler, pleasantries, and redundant conclusions.
- No document hyperlinks: Cite workspace documents and code files by backticked filename.
- No LaTeX math: Avoid `$...$`, `$$...$$`, `\frac`, or `\text` unless requested.

## Git conventions

- Conventional commits: Follow `<type>(<scope>): <summary>`.
  - Types: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`.
  - Scopes: Target package, crate, module, or domain (e.g., `core`, `compiler`, `schemas`, `python`, `sdk`, `web`, `ui`, `cli`, `sync`, `skill`, `docs`, `config`, `tests`).
  - Format: Imperative mood, lowercase, no trailing period. Blank line before body. Contiguous hyphen bullets (`- `).
