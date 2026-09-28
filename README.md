# f8

Financial modeling engine with compiler verification and live Excel formulas.

## Architecture

The f8 architecture separates declarative model specification, intermediate representation expansion, dependency graph cycle detection, topological evaluation, spatial layout planning, formula lowering, and OpenXML binary emission into distinct compiler passes.

- [Architecture overview](docs/architecture.md)
- [CLI & JSON authoring guide](docs/cli-authoring-guide.md)
- [2D matrix DSL reference](docs/dsl/spec-format.md)
- [Standard function catalog](docs/functions/catalog.md)
- [Python modeling skill](python/f8/skills/f8-modeling/SKILL.md)
- [Compiler living specifications](specs/living-specs/)

## Workspace crates

- `crates/f8-core`: Pure compiler engine, IR data models, evaluator, and OpenXML emitter.
- `crates/f8-cli`: Command-line interface for compiling, validating, and inspecting models.
- `crates/f8-python`: PyO3 C-ABI native extension bridging the core compiler into Python.
