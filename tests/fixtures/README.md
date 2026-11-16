# Test fixtures

Fixtures are the evidence a compatibility claim is tested against: native files,
recorded reference computations, and documented operations with expected
results.

## What may live here

- **Self-authored** files created with the reference product under a lawful
  license, where their content is your own data (not vendor sample content).
- **Openly licensed** sample files, with the license recorded.
- **References by provenance** — a pointer (URL + checksum) to a lawfully
  published file, fetched at test time rather than committed, when the file
  itself cannot be redistributed here.

## What must not live here

- **Vendor binaries** or any vendor-distributed program files.
- **Vendor-copyrighted sample content** (the product's bundled example projects,
  artwork, help content).
- Anything obtained by defeating a DRM, dongle, or activation check.

Each fixture records where it came from and why it is lawfully usable. A fixture
whose provenance cannot be stated does not belong in the suite.

## Catalog

| Path | Provenance | Used by |
|------|------------|---------|
| `doc/awkward-v4.xml` | self-authored for coppice, 2026-11-15: comments in every position, odd attribute spacing, single quotes, tabs, entity and character references, CDATA, a processing instruction, unknown elements, empty and self-closing elements | F1, F5 |
| `doc/crlf-no-trailing-newline.xml` | self-authored, 2026-11-15: CRLF line endings, no XML declaration, no trailing newline | F5 |
| `doc/bom-doctype.xml` | self-authored, 2026-11-15: UTF-8 byte-order mark, DOCTYPE, multibyte text | F1, F5 |
| `doc/project.btproj` | self-authored, 2026-11-15: a project file with path, relative-parent and `ros_pkg` includes, no trees, a models block | F1, F3, F5 |
| `doc/v3-dialect.xml` | self-authored, 2026-11-15: v3 dialect (`BTCPP_format="3"`, `SetBlackboard`, `SubTreePlus`) | F2, F5 |
| `doc/no-format.xml` | self-authored, 2026-11-15: no `BTCPP_format` attribute | F1, F10 |
| `reference/o-answers/` (44 files) | the Groot2 save-behavior answers fixture set for O-1 to O-9, copied from exbound/research at commit `d3ca343` (`fixtures/groot2/o-answers/`, CC-BY-4.0); every file matches the research MANIFEST at that commit. Inputs (`in-*`, and `outside.xml`, the O-4 project's `../outside.xml` include) are self-authored, except `in-o1-proj-corrected/tree.xml`, which is the reference's own rewrite of a self-authored tree (the O-8 per-file save). Outputs (`out-*`, `*-rt.xml`) are the reference application's own written bytes for those inputs. The O-4 input's absolute include is a neutral path, as in the research copy; the capture used a machine-specific one. The set's MANIFEST and PROTOCOL stay in the research record | F6, F8, F9, F10 |
| `reference/o-answers-2/` (50 files) | the second answers fixture set, for O-10 to O-16, copied from exbound/research at commit `d3ca343` (`fixtures/groot2/o-answers-2/`, CC-BY-4.0); every file matches the research MANIFEST at that commit. Inputs (`in-*`) are self-authored, and `out-*-corrected/` are self-authored corrected copies of the inputs the reference refused to load. The other outputs (`out-*`, and the three `*.evidence.xml` files: two copies of an exported project file and one tree the reference saved in place during an export) are the reference application's own written bytes. The set's MANIFEST and PROTOCOL stay in the research record | F6, F7, F8, F9, F10 |
| `project/` (4 files) | self-authored, 2026-11-15: a project (`app/main.btproj`) declaring one model and a main tree, including a tree file by a subdirectory path (`app/trees/a.xml`, which itself includes `sub/b.xml` relative to its own directory) and one by a parent-relative path (`shared/common.xml`); `a.xml` uses one port the project's model does not declare | F3, F10 |
| `doc/pair-space.xml`, `doc/pair-tab.xml` | self-authored, 2026-11-15: identical except one space versus one tab between attributes (byte offset 22, counting from 0) | F5 negative control |
