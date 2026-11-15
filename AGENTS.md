# AGENTS — Exbound implementation repository

Conventions for any agent or contributor working in an Exbound **implementation**
repository (one that ships code reimplementing a target's behavior). The
organization-wide rules live in
[exbound/.github](https://github.com/exbound/.github); the research method lives
in [exbound/research](https://github.com/exbound/research). This file states what
is specific to building and verifying an implementation. Where a rule here
conflicts with a more specific `AGENTS.md` deeper in the tree, the deeper one
wins.

## What this repository is

An implementation repo reconstructs one or more **capabilities** (C1–C9) of a
single reconstruction target, measured against the vendor application as the
**behavioral reference**. It is not a file converter: reading native files is
one subsystem (C2), not the goal. The target's research dossier in
`exbound/research` is the evidence record; this repo is the independent software
built from the clean-room functional specification, plus its tests.

## This repository — coppice

An independent, Groot2-*compatible* behavior-tree IDE. The reference product is
Groot2 (proprietary); the open-source BehaviorTree.CPP (MIT) is the public
specification of most of the interface. The research record is the Groot2
dossier and compatibility-surface study in `exbound/research`.

- **v1 is a headless Rust core plus a command-line tool**: lossless load and
  save of projects and tree XML in both dialects, `.btlog` read/write, and a
  terminal monitor client. No GUI crate enters the core; a GUI is a later,
  separate crate.
- **Inputs for implementation are this repository and `exbound/research`,
  nothing else.** Do not read, copy from, or cite any non-public analysis
  material, even when it is on the same machine. If a fact the code needs is
  not in `docs/engineering-spec.md` or the public research record, the
  answer is a spec change (with its public source), not a look elsewhere.
- **Public oracles only in CI:** a real BehaviorTree.CPP 4.x
  `Groot2Publisher`, the Apache-2.0 `btview mock`, and self-authored
  fixtures with provenance in `tests/fixtures/README.md`. Vendor sample
  files are never committed.
- **BehaviorTree.CPP is not the proprietary original.** Its sources may be
  read and cited (with version and path), as the research record already
  does.

## Clean-room wall — the rule that defines the project

Where this repo reimplements a proprietary original, the work is split by a
documented two-phase wall:

1. **Analysis** produces a functional specification (`docs/engineering-spec.md`)
   that excludes verbatim vendor code and vendor symbol names. Analysis
   artifacts — disassembly, decompiler output, binary internals, vendor symbol
   names, the tools used to obtain them — stay in the research/private tree and
   **never** enter this repository, its history, its issues, or its PRs.
2. **Implementation** builds from that specification alone.

Consequences that hold in every commit here:

- **No vendor code, symbol names, UI artwork, icons, help text, or sample
  content** in source, comments, tests, or commit messages.
- **No raw binary-internals detail** (addresses, vtable layouts, tag/byte
  grammars beyond what the published spec states) — describe the *format and
  behavior*, never how it was recovered.
- **Nominative compatibility only:** "reads [product] projects,"
  "[vendor]-compatible." Never name the software after the product; never use
  vendor branding.
- **Lawfully held inputs only.** Do not commit or redistribute vendor binaries
  or vendor-copyrighted sample files. Test fixtures are either self-authored,
  openly licensed, or referenced by provenance — see `tests/fixtures/README.md`.
- Open-source readers (liborigin, xylib, Bio-Formats, …) are **not** proprietary
  originals; they may be used and extended directly under their own licenses.

## Evidence and the coverage triplet

This repo reports progress the way the research program does:

- Every capability is tracked across the **coverage triplet** — reference
  *observed* / independently *implemented* / *verified against* the reference —
  recorded independently. A capability is only as complete as its lowest covered
  property. High implementation with low reference coverage means code was
  written that the original was never confirmed to do.
- `full` / `partial` / `none` are meaningful only next to an **enumerated
  feature set**. Without one, say "sketch," never a percentage.
- Mark statements ESTABLISHED (observed/cited) vs. HYPOTHESIS / NEEDS-OBSERVATION;
  do not promote one into the other.
- **Negative results count** and get the same evidence standard: a record that
  cannot yet be decoded, with the specific blocker, is a valid contribution.

## Verification

A fixture-and-oracle discipline, not assertion:

- A compatibility claim is backed by a **fixture** (a native file, a recorded
  reference computation, a documented operation) plus the **expected result**
  measured against a named reference-software version. State that version.
- A fix is not verified until the test named for the defect has been watched to
  fail without the fix and pass with it.
- Run what a change can actually move. `just check && just test` before every
  commit; put anything long in the background and say what ran.
- Report the highest rung of verification actually reached; never promote a
  passing unit test into a verified compatibility claim.

## Commits and changes

- [Conventional Commits](https://www.conventionalcommits.org):
  `type(scope): description` — `feat` `fix` `docs` `refactor` `test` `ci`
  `chore` `build`. Imperative, lowercase, no trailing period; subject ≤ 72.
- One logical change per commit. Reference issues in the body/footer, not the
  subject.
- Describe **what changed**, never the process that produced it — no tool names,
  model names, or "adversarial review" in commit messages, changelogs, or PR
  bodies. **No AI attribution of any kind** (no co-author trailer, no "Generated
  with" footer, no session URL). Read the trailers before committing.
- Small, reversible changes over large rewrites. Edit only what the change needs.

## Commands

If a `justfile` is present, use it (`just --list`). The standard recipes are a
thin, language-agnostic surface; each repo wires them to its own toolchain:

- `just setup` — install dependencies
- `just fmt` — format
- `just lint` — lint
- `just check` — fmt-check + lint + typecheck (the CI equivalent)
- `just test` — run tests

## Layout

```
docs/engineering-spec.md   the clean-room functional spec this code implements
docs/setup.md              contributor toolchain setup (kept separate from the
                           research diary, so requirements need not be inferred)
crates/                    implementation: a Rust workspace (see docs/design-decisions.md D2)
tests/fixtures/            shared fixtures (see tests/fixtures/README.md); each crate's
                           tests live in crates/<crate>/tests/
tests/harness/             the BehaviorTree.CPP test harness (design decision D8; not yet built)
.github/workflows/ci.yml   runs `just check` and `just test`
```
