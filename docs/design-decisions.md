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

## D2. Workspace layout — Provisional (first code change confirms it)

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

## D3. Lossless XML: keep the source, splice edits — Decided

No Rust XML library we found writes a document back byte for byte from its
model. Readers can report source positions (`quick-xml`'s byte offsets,
`roxmltree`'s attribute ranges), but nothing guarantees an unmodified
document is reproduced exactly. F5 requires exact bytes, so:

- The document keeps its **original bytes**. Parsing (with `quick-xml`'s
  event reader, which reports byte positions) builds the tree model with a
  **byte span** for every element, attribute, attribute value and text run.
- **Saving unmodified output writes the original bytes.** Each change the
  spec requires (the `BTCPP_version` stamp, include-path rewrites) is a
  **splice** at a known span. Untouched regions are never re-serialized, so
  they cannot drift.
- Later editing (L3) uses the same rule: re-emit only the regions an edit
  touches, in the document's own detected style (indentation, quote
  character, line ending).
- Encoding: UTF-8 as required by the formats. Other declared encodings are
  refused with a clear error until a fixture requires them.

## D4. ZeroMQ: the `zmq` crate over libzmq — Decided

The pure-Rust ZeroMQ implementations state that they do not cover the full
feature set, and the reference publisher is libzmq. Using libzmq on both
sides removes a class of interop doubt. Cost: a C library to build or link,
and a non-idiomatic API, wrapped once inside `coppice-monitor`. libzmq's
license is checked with the rest of the dependency tree (D9).

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

Library crates return typed errors (`thiserror`) that carry the offset or
span and the bytes found. Diagnostics, meaning conditions the spec says to
report without refusing (dialect mismatch, port-versus-model mismatch,
unknown uid, status byte 10), are a separate, collected list, never an
error. The CLI uses `anyhow` at its top level only.

## D8. Test harness — Provisional (the CI change confirms it)

- **BehaviorTree.CPP 4.9.0 is built from source in CI** (it is MIT) into a
  small harness program under `tests/harness/`. The harness runs
  self-authored trees, writes `.btlog` with `FileLogger2` along with its own
  record of what it wrote, and serves a `Groot2Publisher` for the
  monitor-client tests. The build is cached by version.
- **`btview mock`** (Apache-2.0, installed with its version pinned) is the
  second, independent publisher.
- Byte-exact comparisons fail with a hex diff at the first differing
  offset, not with a pass/fail summary.

## D9. Dependency hygiene — Decided

`cargo-deny` in `just check`, with an allow-list of licenses compatible with
Apache-2.0 distribution. It joins the gate with the first third-party
dependency; until then there is nothing for it to check. Every dependency is
pinned by `Cargo.lock`, which is committed, and every cargo command in the
gate runs with `--locked`.

The Rust toolchain is pinned exactly in `rust-toolchain.toml`, so CI and
local builds use the same compiler and the same lints. Moving it is a
deliberate change of its own.

## D10. Command-line surface — Provisional

Verbs map one-to-one onto features, so each is testable on its own:
`check` (parse and diagnostics), `save` (reference save; `--lossless` for
F5 with no stamp), `save-as`, `log dump`, `log write`, `monitor` (a terminal
view), `record`. Output is plain text by default, with `--json` for scripts.
