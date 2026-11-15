# Clean-room record

coppice reimplements behavior of a proprietary product (Groot2) and is built
under a documented two-phase wall: an **analysis** phase that observes the
reference and publishes functional facts, and an **implementation** phase
that builds only from those published facts. This file is the dated record
of who worked on which side, and of any exposure across the wall.

BehaviorTree.CPP is open source (MIT) and is not the proprietary original. Its
sources may be read and cited on either side.

## The wall

- **Analysis side.** Observes the reference product (Groot2 v1.10.0) as a
  black box: running it against self-authored and reference-supplied inputs,
  and recording files, wire traffic and UI behavior. Its private working
  material never enters this repository. It publishes functional facts,
  with no vendor code, no vendor text and no recovery mechanics, to the
  public research record in `exbound/research`.
- **Implementation side.** Builds coppice only from this repository, the
  public research record, and the open-source BehaviorTree.CPP code.
  `docs/engineering-spec.md` is the bridge. Every fact in it cites one of
  those, every fact about the reference product's own behavior cites the
  public research record, and anything the record lacks is an open
  question addressed to the analysis side (spec §7), not something looked
  up elsewhere.
- **Review.** Spec changes are reviewed by the analysis side for leaked
  mechanics or vendor text.

## Record

| Date | Side | Who | What |
|------|------|-----|------|
| 2026-11-13 to 2026-11-14 | analysis | the operator, with an analysis agent session | observed the reference (work packages G-0 to G-6) and published the results to the research record |
| 2026-11-15 | implementation | an implementation agent session, directed by the operator | wrote `docs/engineering-spec.md` and `docs/design-decisions.md` from the public research record at commit `bc319c2` and the BehaviorTree.CPP 4.9.0 source |
| 2026-11-15 | exposure note | the implementation agent session | before drafting the spec, while assessing the plan, read the analysis side's non-public readiness summary of the observations (prose describing observed behavior; no vendor code, binaries or vendor-authored files). The spec was then written from the public record and the open-source BehaviorTree.CPP code only, and its draft was checked against that record line by line, which removed two details that came from the summary. A characterization of the failing sample files was dropped. A blackboard edge case (unknown names are skipped) was first kept as a HYPOTHESIS; it is open-source library behavior, not reference-product behavior, so it now cites the BehaviorTree.CPP 4.9.0 publisher source. No reference-product fact in the spec rests on the summary. Analysis-side review: requested; by the operator's decision of 2026-11-15 the spec merges once green and through two rounds of independent review, and the analysis-side review follows the merge. |
| 2026-11-15 | analysis | the operator, with an analysis agent session | answered open questions O-1, O-2, O-4, O-5, O-6, O-8 and O-9 against the reference and published the findings and fixtures to the research record (commit `debb96f`) |
| 2026-11-15 | implementation | the implementation agent session | updated the spec's save sections and copied the fixture set from the public research record at `a1990bb` only; research `39c7acc` later changed one fixture to match coppice's copy, and the provenance now cites that commit |

The operator approved this arrangement, including the exposure note, on
2026-11-15.
