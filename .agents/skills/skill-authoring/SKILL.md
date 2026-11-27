---
name: skill-authoring
description: >-
  Authoring and pruning agent instructions.
  Use when creating, editing, or auditing a SKILL.md, references/ checklist, or AGENTS.md;
---

# Skill Authoring Guide

Applies to `SKILL.md`, `references/` checklists, `AGENTS.md`, and other agent-consumed docs.

## Principles

- Write for a capable reader: Every line competes for attention; omit anything the agent already does correctly by default.
- Explain intent: A rule with its reason generalizes to unlisted cases; a bare command gets applied literally or over-applied.
- Single home & co-location: Keep one idea in one place so rules cannot drift, and group a concept's definition, rules, and caveats under one heading rather than scattering them across sections.
- Encode in structure first: Enforce mechanical invariants (syntax, banned APIs, formatting, schema checks) via types, lints, tests, or scripts—not prose. Reserve prose for judgment calls.

## Content selection

Include only for a real workspace contract or an observed failure mode, not speculative "just in case" hedging:

- Workspace contracts: paths, data boundaries, file layouts, CLI invocations, and public APIs.
- Non-obvious pitfalls: errors that are silent, plausible-looking, and costly.
- Corrections to model priors: places where default habits pull the wrong way.
- Judgment frameworks: how to decide, prioritize, or trade off, stated as reasoning rather than ceremony.

Exclude:

- General programming or domain textbook knowledge.
- Facts the agent can cheaply inspect from the environment (`pyproject.toml`, `package.json`, CLI `--help`, directory layout); document only unwritten conventions, rationale, gotchas, or expensive lookups.
- Content owned by `AGENTS.md` or another skill; reference it by name instead.
- Exhaustive example lists; a few representative cases convey the pattern.

## Explicit rules vs. principles

Default to principles. Make a rule explicit (`Never` / `Must`) only when at least one holds:

| Condition                                 | Example                                                  |
| :---------------------------------------- | :------------------------------------------------------- |
| Workspace contract the agent cannot infer | Run verification suite from `AGENTS.md` before finishing |
| Failure is silent and costly              | Exhaustive enum match without `_ =>` wildcard arms       |
| Model instinct is reliably wrong          | Fix defects at origin; never patch symptoms downstream   |
| Pre-commitment is the point               | Get spec approval before dispatching Implementer         |

Rules degrade performance when they:

- Impose arbitrary numeric thresholds that are context-dependent. Give them as examples (`e.g. 5–10%`), not gates, unless the number is a hard contract.
- Read as mandatory ceremony for every case. Mark illustrative lists (`for example`).
- Use absolutes that are sometimes wrong. One false `Never` teaches the agent to distrust the rest.

## Structure, pointers & steps

- Context pointers: Skill `description` fields and `AGENTS.md` pointers to docs or references stay loaded on every turn, taxing attention even when unused.
  - Add an auto-triggered skill only when independent discovery earns that permanent cost (prefer manual-only invocation where the harness supports it).
  - Pointer wording—not the target file—governs whether the agent loads it; treat missed loads as a wording bug before inlining.
  - Front-load the distinguishing concept, list one trigger per genuinely distinct branch (collapse synonyms), cut summaries the body already carries, and add a negative boundary (`do not use for...`) where adjacent skills could collide.
- Progressive disclosure: Inline what every run needs; push into separate reference files (cited by backticked path, e.g., `references/repo-checks.md` or `docs/architecture.md`) what only some branches reach.
- Workflow steps & completion criteria: End each procedural step on a checkable, demanding completion bound (e.g., "map every acceptance criterion to code and covering test" over "review the diff"). Vague bounds invite premature completion when later steps are visible; sharpen the bound first, and split across a real context boundary (e.g., a subagent handoff) only if rushing persists.

## Wording & pruning

- Positive framing: State the target behavior positively rather than steering by bare prohibition. When a `Never` guardrail is required, pair it with the positive alternative so attention lands on what to do.
- Calm register & concrete terms: Avoid stacked intensifiers (`strictly`, `mandatory`, `aggressively`). Prefer plain, established domain terms over mnemonics, backronyms, or repeated multi-word paraphrases.
- Writing style: Follow `AGENTS.md` writing style.

## Review check

Before finishing an instruction update, confirm:

- Every explicit rule meets a condition in the rules table, pairs any `Never` with a positive target, and traces to a workspace contract or observed failure mode.
- No prose rule (new or existing) enforces a mechanical invariant better handled by a type, lint, test, or script.
- No idea is duplicated or scattered; no content mirrors `AGENTS.md`, a sibling skill, or cheap repo lookups.
- Context pointers (frontmatter `description` and doc/reference links) name distinct branches without echoing body content and hold up against positive and near-miss prompts.
- Every workflow step ends on a checkable completion criterion; cross-references to files, commands, and sibling skills resolve cleanly.
