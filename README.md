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
far, in `crates/coppice-doc`: F1, F2 and F4 (parsing projects and tree XML
in both dialects into a span-preserving model, with a behavior-tree view
of root attributes, includes, trees and node models), F5 (lossless
round-trip), and the document half of F10 (dialect suggestion and
diagnostics). The
`coppice` binary answers only `--version`.

## Build and test

This repo follows the Exbound `just` convention. See
[`docs/setup.md`](docs/setup.md) for toolchain prerequisites.

```sh
just setup   # install the pinned Rust toolchain (rustup >= 1.28)
just check   # rustfmt check + clippy, warnings as errors (CI equivalent)
just test    # cargo test across the workspace
```

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
