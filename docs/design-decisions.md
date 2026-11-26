# Design decisions

How coppice builds what [`engineering-spec.md`](engineering-spec.md) requires.
The spec says *what*; this file says *how*, and why. Each decision is
**Decided** or **Provisional** (to be confirmed by the named check). Changing a
Decided entry means editing it here, with the reason, in the same change.

## D1. Language: Rust — Decided

One language for the document model, the log codec, the monitor client, and
later the GUI. Binary layouts and exact byte output are easier to get right
with explicit integer types and no runtime. The core must be usable without
a window system: from the command line, from tests, and later from a GUI
crate.

## D2. Workspace layout — Decided

The layout below is decided. All four crates exist; `coppice-monitor`
holds only the wire codec until the transport (D4) arrives. A crate arrives
with its first feature, so the workspace never
carries an empty crate with nothing to test.

```
crates/coppice-doc      documents: parse, model, save (F1–F10)
crates/coppice-btlog    .btlog read/write (F11, F12)
crates/coppice-monitor  live-monitor client and recorder (F13–F16)
crates/coppice-cli      the `coppice` binary
```

No GUI dependency in any of these. A later `coppice-gui` crate depends on
them; they never depend on it. Protocol-specific types stay inside
`coppice-monitor`; the shared model is a behavior tree (ordered children,
subtrees, ports, models), never a generic node-and-edge graph, so that a GUI
library's graph type cannot become the data model.

## D3. Two writers: lossless from the source, reference from the model — Decided

No Rust XML library we found writes a document back byte for byte from its
model. Readers can report source positions (`quick-xml`'s byte offsets,
`roxmltree`'s attribute ranges), but nothing guarantees an unmodified
document is reproduced exactly. F5 requires exact bytes, so:

- The document keeps its **original bytes**. Parsing builds the tree model
  with a **byte span** for every node (element, start and end tag,
  attribute name and raw value, text, comment, CDATA, declaration,
  processing instruction, DOCTYPE, byte-order mark).
- **The tokenizer is our own** (`coppice-doc/src/parse.rs`). Its only job is
  to find where each construct starts and ends and to reject input that is
  not well-formed, so the spans are exact by construction rather than
  reconstructed from another library's positions, and the core needs no
  dependency. It is held to the spec's F5 tests: every fixture renders back
  byte for byte, and the spans tile the source with no gap or overlap.
- **Lossless output is rebuilt from the spans**, so an unmodified document
  renders to its original bytes through the same path an edit's splice will
  use. Untouched regions are never re-serialized, so they cannot drift.
  Stamping and include rewriting are reference-mode changes and belong to
  the reference writer below, not to the lossless writer.
- Later editing (L3) uses the same rule: re-emit only the regions an edit
  touches, in the document's own detected style (indentation, quote
  character, line ending).
- **The reference writer is a separate serializer.** The reference
  application rewrites every file into one canonical style (spec §3.2,
  §3.3), so its output cannot come from splicing the source. A second
  writer serializes the document model in that style. The two never mix:
  lossless output never reformats, and reference output never carries
  source formatting.
- Encoding: UTF-8 as required by the formats. Other declared encodings are
  refused as unsupported until a fixture requires them.

## D4. ZeroMQ: the `zmq` crate over libzmq — Decided

The pure-Rust ZeroMQ implementations state that they do not cover the full
feature set, and the reference publisher is libzmq. Using libzmq on both
sides removes a class of interop doubt. Cost: a C library to build or link,
and a non-idiomatic API, wrapped once inside `coppice-monitor`. libzmq's
license is checked with the rest of the dependency tree (D9).

`zmq` 0.10 builds the libzmq **4.3.4** source that its `zeromq-src`
dependency bundles and links it statically; it cannot link a system
libzmq. So the build needs a C++ compiler and no system package, and the
libzmq version is pinned by `Cargo.lock`. libzmq 4.3.4 is licensed under
the LGPL-3.0 with libzmq's static-linking exception (`COPYING.LESSER` in
`zeromq-src`'s `vendor/`, lines 169–181), which allows distributing the
linked executable under terms of one's choice (a modified libzmq must
extend the exception), so coppice stays Apache-2.0. The libzmq build also
compiles a bundled SHA-1 (`vendor/external/sha1`, BSD-3-Clause, whose
notice a binary distribution reproduces) and, on Windows only, wepoll
(`vendor/external/wepoll`, BSD-2-Clause, whose notice a Windows binary
distribution reproduces). The 4.3.4 tree records libzmq's
contributors' permissions to relicense under MPL-2.0 (`vendor/RELICENSE/`);
a later libzmq under MPL-2.0 would need a binding that links it.

The client is blocking on its own thread: REQ polling plus a SUB poll, with
results sent over a channel. No async runtime in v1.

## D5. MessagePack: `rmp-serde` into `serde_json::Value` — Decided

Blackboard dumps are MessagePack-encoded JSON of arbitrary shape (spec
§5.5). They are decoded into a generic JSON value and never into fixed
structs.

## D6. v3 binary wire: not in v1 — Decided

The reference client uses the v4 wire in both modes (spec §5), so v1 needs
no FlatBuffers and no v3 socket layout. A v3-to-v4 translator (L5) would add
the official `flatbuffers` crate then.

## D7. Errors and diagnostics — Decided

Library crates return typed errors that carry the byte offset (or span)
of the problem and, where it helps, the names involved (an end tag's
expected and found names, the open element's name). `Display` and `std::error::Error` are written by hand: a
derive crate would bring a procedural-macro build dependency for a handful
of variants. Diagnostics, meaning conditions the spec says to
report without refusing (dialect mismatch, port-versus-model mismatch,
unknown uid, status byte 10), are a separate, collected list, never an
error. The CLI reports errors as text on standard error, with no error crate:
`anyhow` would be its first third-party dependency, and nothing yet needs
more than a message.

## D8. Test harness — Decided for logs; Provisional for the monitor

- **BehaviorTree.CPP 4.9.0 is built from source in CI** (it is MIT) into a
  small harness program under `tests/harness/`: CMake fetches the release
  archive, pinned by SHA-256, and builds only the core library. The harness
  runs self-authored trees and writes `.btlog` with `FileLogger2` along with
  its own record of what it wrote (`btlog_harness`, in place since F11 and
  F12). `just oracle` builds it, runs it, and runs the ignored oracle tests
  against its output; the CI `oracle` job runs that recipe, with the build
  directory cached by the harness sources. The oracle files are generated
  on each run, never committed. For the monitor-client tests the harness
  will also serve a `Groot2Publisher`; that part is still Provisional.
- **`btview mock`** (Apache-2.0, installed with its version pinned) is the
  second, independent publisher.
- Byte-exact comparisons fail with a hex diff at the first differing
  offset, not with a pass/fail summary.

## D9. Dependency hygiene — Decided

`cargo-deny` in `just check` (`deny.toml`), with an allow-list of licenses
compatible with Apache-2.0 distribution, crates.io as the only source, and
no wildcard versions. It joined the gate with the first third-party
dependencies (D4's `zmq`, and `rmp-serde` and `serde_json` for the
blackboard, F15). `zeromq-src` declares only the license of its own build
code, so `deny.toml` clarifies it to include the licenses of the C it
compiles (D4), each tied to the hash of its license file, and allows those
terms by name. A clarification whose hash stops matching no longer
applies, and the gate fails on the allowance it leaves unused, so a new
license text fails the gate until it is read. Advisories are not in
the gate, since they need a network fetch at check time. Every dependency is
pinned by `Cargo.lock`, which is committed, and every cargo build in the gate
(`clippy`, `test`) runs with `--locked`.

The Rust toolchain is pinned exactly in `rust-toolchain.toml`, so CI and
local builds use the same compiler and the same lints. Moving it is a
deliberate change of its own.

## D10. Command-line surface — Provisional

Verbs map one-to-one onto features, so each is testable on its own:
`check` (parse and diagnostics), `save` and `save-as` (`save-as` is lossless
by default, and `save` exists only with `--reference`, since nothing edits a
document yet; `--reference` for the reference application's canonical
output, which for a project is an export to a directory), `log dump`, `log write`, `monitor`
(a terminal view), `record`. Lossless is the default by the operator's
decision of 2026-11-15: a user's comments and formatting are never lost
unless they ask for reference output. A reference save of a project is
an export: it needs a new or empty destination directory and never
overwrites an existing file (spec §3.3). Output is plain text by default, with `--json` for
scripts.

In place now:

- `check <file>` loads the file with its includes (F3) and prints each
  diagnostic (F10) to standard error as `<path>:<line>:<column>: <kind>:
  <message>`.
- `save-as <input> <output>` writes the lossless form (F5), or with
  `--reference` the standalone reference form (F9), optionally `--v3`
  (F7) and `--target <version>`. It never replaces an existing file.
- `save <tree-file> --reference` rewrites the file in place as the
  reference saves it: within a project with `--project <project-file>`
  (F6, F7), or standalone (F9). `save` without `--reference` is a usage
  error, since a lossless save of an unedited file changes nothing. It
  keeps the file's permissions, owner and group, and refuses a symbolic
  link, a file its user may not write, a file with more than one hard link,
  a file it cannot give the same owner and group, and a file swapped,
  linked or rewritten while it was being saved (seen by its inode, link
  count, size or modification time just before the rename), where
  replacing the file would lose what the user expects kept.
- `log dump <file.btlog>` prints the header and one tab-separated line per
  record, with its absolute time and the node's previous status (F11). On
  a damaged file it prints what was read before the damage.
- `log xml <file.btlog>` prints the tree XML a recording embeds, byte for
  byte: the runtime form, with each node's `_uid`, which a tree file on
  disk lacks. On a damaged file whose XML and T0 are whole, it prints the
  XML and fails.
- `log write <tree.xml> <records> <file.btlog>` writes a recording (F12)
  from a tree's XML, embedded byte for byte, and records in the form
  `log dump` prints. The XML `log xml` printed and the records `log dump`
  printed write back to the same bytes. With another XML, the dump's
  `xml_bytes` line refuses it unless the lengths agree (remove the line to
  use it anyway), and a record's uid may name no node. The records file needs a `t0_us` line; a record is
  `offset_us uid status`, or the five columns `log dump` prints, whose
  `absolute_us` and `previous` are not read. Blank lines, lines starting
  with `#` and a leading byte-order mark are skipped. `records` and
  `xml_bytes` lines are optional, each given at most once, and checked
  when given. A uid the XML lacks is a warning against the XML file. Like
  `save-as`, it never overwrites a file.
- Every file the CLI writes appears complete or not at all.
- Exit codes: 0 done, 1 the input has a problem or was refused, 2 the
  command line is wrong (an unknown or repeated option, a missing, empty
  or flag-like option value, an option where a file name belongs, an
  argument that is not UTF-8). A file whose name starts with `-` is named
  as `./-name`. Output cut short by its
  reader, as by `| head`, ends the run quietly with 0.

Not yet: `monitor` and `record`
(with F13 to F16), a reference project export (F8, waiting on O-18), and
`--json`.
