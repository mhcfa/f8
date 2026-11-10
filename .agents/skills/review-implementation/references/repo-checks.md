# f8 Repository Review Checks

- [ ] Clean phase & I/O boundaries (`Blocker`): Zero filesystem I/O in `crates/f8-core` and `crates/f8-wasm`; `.xlsx` disk writes (`save()`) stay isolated in outer language bindings (`crates/f8-python`, `python/f8/`, `ts/`).
- [ ] Excel calculation & syntax parity (`Blocker`): New or changed functions and operators match Excel semantics (IEEE-754 `-0.0` normalization, type coercions, `Number < Text < Boolean` comparison ranking, error bubbling, lazy branch evaluation); tests encode Excel-verified expectations.
- [ ] Structured `F8Error` hierarchy (`Blocker`): Exceptions across `crates/f8-python`, `python/f8/`, `crates/f8-wasm`, and `ts/` inherit from `F8Error` (except native `TypeError`) with structured context attributes (coordinates, cycles, tables, offsets).
- [ ] Packaged skill & architecture sync (`Flag`): Changes altering compiler pipeline, DSL, or function behavior name the section of `docs/architecture.md` or `python/f8/skills/f8-modeling/` to update in the fold step.
