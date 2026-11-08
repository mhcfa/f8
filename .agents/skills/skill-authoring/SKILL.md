---
name: skill-authoring
description: >-
  Authoring and refining agent skills (SKILL.md) and reference checklists.
  Content selection, explicit rules vs. principles, wording, structure, and cross-skill boundaries.
  Use when creating, rewriting, or reviewing a SKILL.md.
---

# Skill Authoring Guide

## Principles

- Write for a capable reader: The agent already knows general programming and domain textbooks. A skill adds what it lacks: workspace contracts, judgment calls, and pitfalls where default instincts fail.
- Explain intent: A rule with its reason generalizes to cases the author never listed; a bare command gets applied literally or over-applied.
- Smallest sufficient text: Every line competes for attention. Cut what the agent would do correctly without being told.
- Self-consistent: One idea, one place. Duplicated rules drift apart; contradictory rules force the agent to guess.

## Content selection

Include:

- Workspace contracts: paths, data boundaries, file layouts, CLI invocations, and public APIs.
- Non-obvious pitfalls: errors that are silent, plausible-looking, and costly.
- Corrections to model priors: places where default habits pull the wrong way.
- Judgment frameworks: how to decide, prioritize, or trade off, stated as reasoning rather than ceremony.

Exclude:

- General domain knowledge the model reliably has.
- Content owned by `AGENTS.md` or another skill; reference it by name instead.
- Exhaustive example lists; 3–4 representative cases convey the pattern.

## Explicit rules vs. principles

Default to principles. Make a rule explicit (`Never` / `Must`) only when at least one holds:

| Condition                                 | Example                                                  |
| :---------------------------------------- | :------------------------------------------------------- |
| Workspace contract the agent cannot infer | Run verification suite from `AGENTS.md` before finishing |
| Failure is silent and costly              | Exhaustive enum match without `_ =>` wildcard arms       |
| Model instinct is reliably wrong          | Fix defects at parser/schema origin, not via band-aids   |
| Pre-commitment is the point               | Get spec approval before dispatching Implementer         |

Rules degrade performance when they:

- Impose arbitrary numeric thresholds that are context-dependent. Give them as examples (`e.g. 5–10%`), not gates, unless the number is a hard contract.
- Read as mandatory ceremony for every case. Mark illustrative lists (`for example`).
- Use absolutes that are sometimes wrong. One false `never` teaches the agent to distrust the rest.

## Wording & structure

- Plain descriptive terms over mnemonics or backronyms.
- Calm register: Reserve `never`, `always`, `must` for genuine contracts. Avoid stacked intensifiers (`strictly`, `mandatory`, `aggressively`).
- Frontmatter `description`: State what the skill covers and when to activate it in descriptive terms.
- Progressive disclosure: Keep `SKILL.md` concise. Move detailed repo-specific checklists, schemas, or reference tables into `references/` and cite them by backticked path (`references/repo-checks.md`).
- Follow `AGENTS.md` writing style (telegraphic, sentence case, `Key: value` labels, backticked filenames, no hyperlinks, no LaTeX).

## Review check

Before finishing a skill update, confirm:

- Every explicit rule meets a condition in the rules table; everything else reads as a principle with its reason.
- No idea appears twice; no content duplicates `AGENTS.md` or a sibling skill.
- Cross-references to files, commands, and sibling skills resolve cleanly.
