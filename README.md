# coppice — Groot2-compatible behavior-tree tooling

An independent document model, recorder and live-monitor client for
BehaviorTree.CPP behavior trees that reads and writes Groot2 projects, tree
files and recordings the way the reference IDE does.

This is an Exbound implementation repository. Exbound builds independent, open,
*compatible* reimplementations of proprietary scientific and engineering
software. The vendor application is the **behavioral reference, not the
architectural blueprint**; see the [research program](https://github.com/exbound/research)
for the C1–C9 capability model and the target dossiers.

## Target and scope

- **Reference product:** Groot2 v1.10.0 (free tier and PRO trial), with BehaviorTree.CPP 4.9.0 as the pinned library
- **Capabilities covered (v1):** C2 data (`.btlog`), C3 document (projects and tree XML, both dialects), and the live-monitor client
- **Research dossier:** [Groot2 (target #14)](https://github.com/exbound/research/blob/main/dossiers/groot2.md)
- **Engineering spec:** [`docs/engineering-spec.md`](docs/engineering-spec.md) —
  the enumerated feature set this software implements and how compatibility is
  measured.

## Status

The enumerated feature set (F1–F16) and each feature's coverage triplet
(reference observed / implemented / verified) are in
[`docs/engineering-spec.md`](docs/engineering-spec.md) §1. Implemented so
far, in `crates/coppice-doc`: F1, F2 and F4 (parsing a tree or project
file in either dialect into a span-preserving model, with a behavior-tree
view of root attributes, includes, trees and node models), F5 (lossless
round-trip), F3 (loading a project with its included files, following
BehaviorTree.CPP's include rules as read from its source, with the
departures listed in the spec's §2.3.1), F6, F7 and F9 (the reference
writer: a tree file written byte for byte as the reference writes it, in
V4 or V3 mode, refusing what the research record does not cover), and part
of F10 (dialect suggestion and diagnostics, on a document or a whole
project). In `crates/coppice-btlog`: F11 and F12 (reading and writing
`.btlog` recordings, checked against BehaviorTree.CPP 4.9.0's own logger).
In `crates/coppice-monitor`: the live-monitor wire codec (spec §5: headers,
reply checks, STATUS decoding, transition drains), without a transport yet.
The `coppice` binary exposes them as `check`, `save`, `save-as`,
`log dump`, `log xml` and `log write` (see [`docs/design-decisions.md`](docs/design-decisions.md)
D10):

```sh
coppice check project.btproj                          # diagnostics, includes followed
coppice save-as tree.xml out.xml --reference          # the reference's save-as
coppice save trees/a.xml --reference --project project.btproj
coppice log dump run.btlog > run.txt                  # one line per transition
coppice log xml run.btlog > run.xml                   # the tree XML it embeds
coppice log write run.xml run.txt copy.btlog          # written back: the same bytes
```

## Build and test

This repo follows the Exbound `just` convention. See
[`docs/setup.md`](docs/setup.md) for toolchain prerequisites.

```sh
just setup   # install the pinned Rust toolchain (rustup >= 1.28)
just check   # rustfmt check + clippy, warnings as errors (CI equivalent)
just test    # cargo test across the workspace
just oracle  # build the BehaviorTree.CPP harness and test .btlog against it
```

`just oracle` also needs CMake and a C++17 compiler; it downloads the
BehaviorTree.CPP 4.9.0 release archive unless `BTCPP_SOURCE_DIR` points at
an extracted copy.

## Compatibility and boundaries

Describes compatibility **nominatively** — "reads [product] projects,"
"[vendor]-compatible" — and is not named after the product or affiliated with
the vendor. This software:

- operates on **lawfully held** files only;
- is built **clean-room** where it reimplements a proprietary original — an
  analysis phase produces a functional specification excluding verbatim vendor
  code and symbol names, and this implementation is built from that
  specification alone;
- copies **no** vendor code, UI artwork, icons, help text, or trademarks;
- does **not** defeat DRM, dongles, or activation, and ships **no** circumvention
  tools.

See the research program's [legal basis](https://github.com/exbound/research/blob/main/docs/legal-basis.md).
This is orientation, not legal advice.

## License

Apache-2.0 — see [`LICENSE`](LICENSE). Clean-room code with no copyleft
dependency, so a permissive license with an explicit patent grant is the
default.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) and the organization
[contribution guide](https://github.com/exbound/.github/blob/main/CONTRIBUTING.md).
Progress is published as small, independently verifiable units.
