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
| `doc/pair-space.xml`, `doc/pair-tab.xml` | self-authored, 2026-11-15: identical except one space versus one tab between attributes (byte offset 22, counting from 0) | F5 negative control |
