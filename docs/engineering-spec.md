# Engineering specification — Groot2-compatible behavior-tree tooling

The enumerated feature set coppice implements, and how compatibility with the
reference product is measured. This is the clean-room functional
specification: it states *what the software must do*. It is the only input
the implementation is built from (see [`clean-room.md`](clean-room.md)).
Library and architecture choices are in
[`design-decisions.md`](design-decisions.md), not here.

- **Reference product and versions:** Groot2 **v1.10.0** (free tier, plus the
  vendor's 30-day PRO trial for the hook features), and BehaviorTree.CPP
  **4.9.0** as the pinned open-source library. BehaviorTree.CPP 4.11.0 is the
  vendor's pairing; its deltas are listed under §6.
- **Sources — the only ones this spec draws on:**
  1. the public research record in
     [exbound/research](https://github.com/exbound/research) at commit
     `d3ca343`: the Groot2 dossier (`dossiers/groot2.md`), cited as
     `dossier WP1`…`G-6` by work package; the compatibility-surface
     study (`docs/groot2-compat-surface.md`), cited as `surface §1`…`§6`;
     and the two save-behavior answer sets (`dossiers/groot2-o-answers.md`
     for O-1 to O-9 and `dossiers/groot2-o-answers-2.md` for O-10 to
     O-16, with the fixture sets `fixtures/groot2/o-answers/` and
     `fixtures/groot2/o-answers-2/`), cited as `answers O-n`,
     `answers, canonical style` or `answers, additional behaviors`. Facts
     read directly from a published reference-output file are cited by
     that file, as `fixture <path>`, relative to `o-answers/` unless the
     path starts `o-answers-2/`;
  2. the BehaviorTree.CPP **4.9.0** source (MIT, the open-source library,
     not the proprietary product), cited as `BT 4.9.0 <path>:<line>`. It
     settles what the publisher and the file writers do, which is
     open-source behavior, not reference-product behavior.

  Anything about the **reference product's** own behavior comes from (1)
  only.
- **Capabilities in scope here:** C2 data (`.btlog`), C3 document (projects
  and tree XML, both dialects), C6 programmability (the live-monitor client).
  C8 (the graphical application) is out of scope for v1.

Conventions: **ESTABLISHED** = stated in a source above, with its citation;
**HYPOTHESIS** = an inference from a cited source that a test must confirm;
**OPEN** = not in the public record, listed in §7. Byte layouts are
little-endian throughout.

## 1. Feature inventory

Coverage triplet: *reference observed* (the research record establishes the
behavior) / *implemented* / *verified* (the §8 oracle passes). Nothing is
implemented yet. `full` / `partial` / `none` are meaningful only against this
list.

### v1 — headless core and command-line tool

| # | Feature | Cap. | Ref. observed | Impl. | Verified | Spec |
|---|---------|------|---------------|-------|----------|------|
| F1 | Parse tree XML, v4 dialect, all elements and attributes | C3 | yes (surface §5) | yes — the span model (`lib.rs`, `parse.rs`) and the behavior-tree view (`tree.rs`: root attributes, includes, trees, models) | partial — self-authored and reference fixtures, `crates/coppice-doc/tests/lossless.rs` and `tree_document.rs`; no comparison against the reference's own reading | §2.1 |
| F2 | Parse tree XML, v3 dialect | C3 | yes (dossier G-5) | yes — the same parser and view; v3 spellings kept as written | partial — one self-authored v3 fixture | §2.2 |
| F3 | Parse `.btproj` projects and resolve `<include>` | C3 | yes (surface §3, dossier WP1) | yes — `project.rs`: the library's include resolution, plus coppice's handling of cycles and of a file reached twice (§2.3.1) | partial — self-authored on-disk fixtures (`tests/fixtures/project/`) and in-memory file sets, `crates/coppice-doc/tests/project.rs`; resolution is checked against the library's source, not run against the library or the reference | §2.3 |
| F4 | Parse `<TreeNodesModel>`, including tolerated extensions | C3 | yes (surface §3) | yes — kinds, ports, `editable`, `<description>`, metadata, merged blocks | partial — self-authored and reference fixtures | §2.4 |
| F5 | Lossless save (coppice's default): load then save unmodified, byte-identical | C3 | n/a — the reference has no lossless save (answers O-9); this is coppice's own guarantee | yes | partial — byte-exact over the self-authored fixture set plus the one-byte negative control | §3.1 |
| F6 | Reference save of a tree file (`--reference`): the canonical writer | C3 | yes (answers, canonical style; O-10, O-13, O-14, O-16), except attributes other than `name` and ports, built-in port lists and order, and some root and include cases (O-17) | yes: `reference.rs`, with the refusals of §3.7 | partial: the F6 rows of §8, byte-exact against the reference's output (`crates/coppice-doc/tests/reference.rs`) | §3.2 |
| F7 | Reference save in V3 mode | C3 | yes (answers O-12: the V4 writer, no stamp) | yes: V3 mode of the same writer | partial: the one F7 row of §8, byte-exact | §3.5 |
| F8 | Reference project export | C3 | partial: the project file's layout, models, stamp and include order (answers O-1, O-2, O-4, O-6, O-11, O-14, O-16); which tree files are written and how include paths are rewritten differs between the two recorded sessions (O-18) | none | none | §3.3 |
| F9 | Reference save-as of a standalone tree: a bare canonical tree | C3 | yes (answers O-5, O-13) | yes: the same writer, with the file's own models | partial: the F9 rows of §8, byte-exact | §3.4 |
| F10 | Dialect detection and port-versus-model diagnostics | C3 | yes (dossier WP1, WP3) | partial — on a single document: dialect suggestion, missing format, leaf with children, unknown port, misplaced node ID, malformed, duplicate or built-in-shadowing models, decided as BehaviorTree.CPP's loader decides (BT 4.9.0 `src/xml_parsing.cpp:557-677,763-850`). Across a project (F3, §2.3.1): nodes checked against the project's merged models, plus include problems, replaced trees, an unknown main tree and subtrees that name no tree. Not yet on logs or the monitor tree | partial — the first set's F10 reference fixtures (the leaf-with-child trees) and inline inputs; the second set's leaf-with-child inputs; and its two unknown-port inputs (a built-in `Script` attribute), checked against the built-in port lists taken from the library source (`tests/reference.rs`) | §2.5, §3.6 |
| F11 | Read `.btlog` | C2 | yes (surface §1, dossier WP2/G-2) | none | none | §4.1 |
| F12 | Write `.btlog` | C2 | yes (surface §1) | none | none | §4.2 |
| F13 | Monitor client: connect, FULLTREE, STATUS polling, heartbeat | C6 | yes (surface §2, dossier G-4/G-5) | none | none | §5.1–5.4 |
| F14 | Monitor client: decode the STATUS buffer, including `10+prev` | C6 | yes (surface §2, dossier G-4) | none | none | §5.4 |
| F15 | Monitor client: blackboard request and MessagePack-JSON decode | C6 | yes (surface §2) | none | none | §5.5 |
| F16 | Monitor client: record a live session to `.btlog` | C2/C6 | yes (surface §2, dossier G-2) | none | none | §5.6 |

### Later — enumerated now so the core does not preclude them

| # | Feature | Cap. | Ref. observed | Notes |
|---|---------|------|---------------|-------|
| L1 | Hooks: breakpoint and replace-node insert/remove, dump, remove-all, disable-all, unlock | C6 | yes (surface §2, dossier G-3) | §5.7 states the contract now |
| L2 | Read `.db3` recordings | C2 | yes (surface §4) | schema fixed; no fixture yet |
| L3 | Editing operations on the document model, including fresh V3 saves and their TreeNodesModel drop | C3 | partial (dossier G-5) | lossless saves must keep untouched regions byte-identical; the drop rule is O-3 |
| L4 | Graphical application (canvas, monitor, replay, blackboard) | C8 | partial | after a GUI spike; not specified here |
| L5 | v3 binary wire translator (legacy publisher to v4 client) | C6 | partial (surface §2 cross-check) | the v3 wire is not needed by the reference client (dossier G-5) |

Explicitly **not** reproduced: the free-tier node cap and feature gating
(dossier G-6: a product decision, not a protocol constraint), licensing
behavior, and vendor UI text.

## 2. Documents — parsing

### 2.1 Tree XML, v4 dialect (F1) — ESTABLISHED, surface §5

- A document is `<root …>` containing, in any order: zero or more
  `<include path="…"/>`, zero or more `<BehaviorTree ID="…">`, and at most
  one `<TreeNodesModel>`. A tree file normally holds at least one
  `<BehaviorTree>` (surface §5); a project file may hold none, as the
  record's own `.btproj` example shows (surface §3). Zero trees is therefore
  valid input, not an error.
- `<root>` attributes seen in the record: `BTCPP_format`, `project_name`,
  `BTCPP_version`. **`BTCPP_format` is advisory**: a missing attribute is a
  warning, not an error, and its value is not checked (surface §5,
  dossier WP1). Unknown `<root>` attributes are preserved.
- A node element's tag is its registration name. Attributes are port
  remappings, `name` (instance name), `ID` (on `<SubTree>`), `_autoremap`,
  the pre-conditions `_failureIf`, `_successIf`, `_skipIf`, `_while`, and the
  post-conditions `_onHalted`, `_onFailure`, `_onSuccess`, `_post`.
- Runtime metadata (present in monitor and log XML, may appear in files):
  `_uid` (the `uint16` node id joining tree to status and transitions) and
  `_fullpath` (subtree instance path). In runtime XML each instantiated
  subtree is its own `<BehaviorTree ID>` block, and repeated instantiations
  consume successive blocks with the same ID.
- The parser accepts any well-formed XML 1.0 in this shape, including
  unknown elements and attributes, and keeps them (F5 depends on it). Three
  stated limits are refused with a distinct "unsupported" error, never as
  malformed: a declared encoding other than UTF-8; any DTD beyond a
  bare `<!DOCTYPE name>` (an external identifier or an internal subset),
  because coppice does not read DTDs; and element nesting deeper than
  1024, four times the 256 BehaviorTree.CPP itself accepts (BT 4.9.0
  `src/xml_parsing.cpp:541-548`). Behavior-tree files use none of these;
  supporting them is not planned until a fixture needs it.

### 2.2 Tree XML, v3 dialect (F2) — ESTABLISHED, dossier G-5

The v3 dialect uses the same document shape with: `SetBlackboard` as a node,
`SubTree` and `SubTreePlus` for subtrees, and `BTCPP_format="3"` (accepted,
not enforced). The parser must treat `SubTree` and `SubTreePlus` as distinct
spellings and never translate one into the other.

### 2.3 Projects (F3) — ESTABLISHED, surface §3, dossier WP1/WP3

- A `.btproj` is a tree-XML document whose `<root>` carries `project_name`
  and which typically holds `<include>` references and a `<TreeNodesModel>`.
- `<include>` resolution follows the library (BT 4.9.0
  `src/xml_parsing.cpp:294,383-425`):
  - an absolute `path` is used as given;
  - a relative `path` with a `ros_pkg` attribute is resolved under that
    ROS package's share directory. coppice has no ROS package index, so it
    reports such an include as unresolvable (a diagnostic) and keeps the
    element unchanged;
  - any other relative `path` is joined to the including file's directory,
    and the base is re-set for each nested include.

  The reference resolves every include in its own sample corpus
  (dossier WP1). That its rule matches the library's is a HYPOTHESIS; the
  F3 fixtures include nested and relative-parent includes.
- A project may hold multiple trees across files; the reference loads a
  7-tree, 10-model project (dossier WP3). Loading builds one project model
  over all included documents, each document keeping its own bytes (§3.1).

#### 2.3.1 How coppice loads a project

Loading starts from one file (a `.btproj` or a tree file) and follows its
includes. Only the starting file is required: if it cannot be read or parsed,
or its top-level element is not `<root>`, the load fails. Anything wrong
below it is a diagnostic (D7), and the include concerned is skipped.

What the library decides, and coppice mirrors (BT 4.9.0
`src/xml_parsing.cpp`):

- Only `<include>` children of `<root>` count, in source order (`:365`).
- An include without a `path` is an error to the library (`:373-377`);
  coppice reports it (`IncompleteInclude`), and treats an empty `path` the
  same way, since it names no file.
- With `ros_pkg` and an absolute `path`, the library ignores `ros_pkg`
  (`:387-393`), and so does coppice. With `ros_pkg` and a relative `path`,
  coppice reports `RosPackageInclude` (above).
- A relative `path` is joined to the directory of the including file as it
  was reached, not of its canonical form (`:409-419`).
- A file that cannot be read or is not well-formed is an error to the
  library (`:339-347`); coppice reports `UnreadableInclude` or
  `MalformedInclude`. A file whose top-level element is not `<root>` is
  `MalformedInclude` too.
- Trees are registered by `ID` after the file's own includes are loaded
  (`:365-426` precede the registration), and a later registration of an
  `ID` replaces the earlier one (`:444-458`).
  So a file's own trees win over its includes' trees, and a later include
  wins over an earlier sibling. coppice reports each replacement
  (`DuplicateTree`, at the replacing tree) and resolves the name as the
  library does.
- `main_tree_to_execute` is read from the starting file's `<root>`
  (`:700-718`). A name that no tree has is reported (`UnknownMainTree`).
  Without the attribute, the library needs exactly one tree unless the
  caller names one (`:710-718`); coppice does not report that, since a
  project file usually holds many trees and the caller usually names one.
- A `<SubTree ID>` naming no tree in the project fails when the library
  instantiates it (`:1195-1199`); coppice reports it (`UnknownSubTree`).

What the library leaves undefined or does differently, and coppice
decides:

- **The starting path** is made absolute against the current directory.
  The library takes `absolute(parent_path())` of it (`:294`), which throws
  under libstdc++ for a bare file name (`main.xml`, empty parent) but not
  for `./main.xml`; coppice loads both.
- **Which trees are checked for `UnknownSubTree`:** every tree, including
  one that a later registration replaces and one the main tree never
  reaches. The library fails only on a tree it instantiates.
- **`<SubTreePlus ID>`** (v3) naming no tree is reported like `<SubTree>`
  (`UnknownSubTree`). The 4.9.0 library registers no `SubTreePlus` node, so
  this follows the v3 dialect (§2.2), not the library.
- **A tree without an `ID`** is not listed or resolvable by name. The
  library names it `BehaviorTree_<n>` with a counter (`:449-456`); coppice
  does not mirror that name.

- **Cycles.** The library has no cycle check, so a file that includes
  itself, directly or through others, recurses until it fails. coppice
  reports the include that closes the cycle (`IncludeCycle`) and does not
  follow it.
- **The same file reached twice.** The library loads it again. coppice
  loads each file once, identified by its canonical path (so a symbolic
  link to a loaded file is the same file), and does not report the repeat.
  Its trees keep the registration position of the first visit, where the
  library's would be the last; the two differ only when another file
  declares the same tree `ID`.
- **Models across a project.** The library reads no models from the XML
  except subtree models (`:319-337`). coppice merges the
  `<TreeNodesModel>` entries of every loaded
  document in load order (the starting file, then each include depth-first
  in source order), and checks every tree against the merged list
  (`LeafWithChildren`, `UnknownPort`, §2.5). An `ID` declared again in a
  later document is reported there (`DuplicateModel`), and the first
  declaration is used. How the reference merges models across a project's
  files is not in the record; this choice is coppice's.

### 2.4 `<TreeNodesModel>` (F4) — ESTABLISHED, surface §3

```
<TreeNodesModel>
  <Action|Condition|Control|Decorator|SubTree ID="…">
    <input_port|output_port|inout_port name="…" [type="…"] [default="…"]>description</…>
    <MetadataFields><Metadata k="v"/>…</MetadataFields>
  </…>
</TreeNodesModel>
```

- `type` is omitted for untyped ports; `default` is omitted when empty; the
  element text is the port description.
- Subtree models may declare port defaults (subtree default remappings).
- Tolerated extensions seen in Groot2-era files: an `editable="true"`
  attribute on model elements, and a `<description>` child element
  (surface §3). Both must parse, and both survive coppice's lossless save
  (F5).
- The reference's save **drops** `<description>` and **keeps**
  `editable="true"` (answers O-6, fixture `out-o6/proj.btproj`), which
  confirms the vendor-tracker issue cited in surface §3. coppice's
  lossless save keeps both; its reference save follows the reference.

### 2.5 Dialect detection and diagnostics (F10)

- **Dialect is a mode the user holds, as in the reference** (dossier WP3:
  the reference offers a switch to V3 on mismatch and retitles the
  project). Detection only *suggests* the mode, and never changes it
  silently:
  - `BTCPP_format="3"` suggests v3, and `BTCPP_format="4"` suggests v4;
  - with no `BTCPP_format`, a `SubTreePlus` element suggests v3, since the
    v4 library registers no node of that name (BT 4.9.0 `src/bt_factory.cpp`
    has none);
  - anything else suggests v4.

  `SetBlackboard` is **not** a v3 marker: v4 registers it as a built-in
  (BT 4.9.0 `src/bt_factory.cpp:152`). That this detection agrees with the
  reference's own mismatch check is a HYPOTHESIS; the record does not state
  the reference's rule.
- **Port-versus-model check:** the reference validates instance ports
  against the declared model when it loads a log, and rejects a node whose
  port name does not match (dossier WP3). coppice reports such mismatches
  as diagnostics on load (documents, logs and the monitor tree) and does
  not refuse the input.
- **Reference-generation regressions:** three of the reference's own sample
  files throw at instantiate under BehaviorTree.CPP 4.9.0, though they parse
  (dossier WP1). Parsing never instantiates, so coppice must load any file
  that parses, and report what it can about the structure.

## 3. Documents — saving

coppice has **two save modes**, and they are different operations
(answers O-9: the reference is a normalizing writer, so "save exactly as
the reference does" and "keep the file as written" cannot be the same
thing).

- **Lossless** (F5) — the **default** (operator decision, 2026-11-15). The
  file's bytes are kept exactly; nothing is reformatted. This is coppice's
  own guarantee, not reference behavior.
- **Reference** (F6–F9), opted into with `--reference` — produces the
  reference's output byte for byte: a canonical serialization of the
  document model in which no lexical property of the source survives.

### 3.1 Lossless save (F5)

- Saving an unmodified document reproduces its input bytes exactly:
  attribute order and quoting, whitespace inside and between tags,
  comments, the XML declaration, character references and entity
  spelling, line endings (LF or CRLF), and the presence or absence of a
  trailing newline.
- Lossless save-as writes the same bytes to the new path. It does not
  rewrite include paths; a moved project's relative includes may then
  need the user's attention, which coppice reports as a diagnostic.
- Later edits (L3) change only the regions they touch.
- **Test:** byte equality of the output against the input. A normalized
  or semantic comparison does not pass.
- Node layout is **not** a document property: no file type stores canvas
  coordinates (dossier G-1), and coppice writes none.

### 3.2 Reference save of a tree file (F6) — answers, canonical style

The reference writes every tree file in one canonical style, on every
write path (in-place per-file save, save-as, export), whether or not the
file was modified (answers O-8: an unmodified tree is rewritten), and in
V3 mode too (§3.5). Fixture paths in this section are under
`o-answers/` unless they start `o-answers-2/`. Byte by byte:

1. **Declaration.** Line 1 is `<?xml version="1.0" encoding="UTF-8"?>`.
2. **Root.** `<root>` carries `BTCPP_format` first, then
   `BTCPP_version="<target>"` (V4 only, see *Version stamp*); a
   `project_name` on a tree file's root is dropped (answers O-16, fixture
   `o-answers-2/out-o16b/ordered.xml`, whose source put `project_name`
   before `BTCPP_format`). No input puts the stamp first, so normalizing
   that order too is a **HYPOTHESIS**. With two
   attributes the tag wraps by rule 7. Other root attributes (such as
   `main_tree_to_execute`) are not covered (§7, O-17).
3. **Indentation and element forms.** The body is indented **2 spaces per
   level**, whatever the source used (tabs included: answers O-13,
   `o-answers-2/out-o13/two-trees-tab.xml`). An element with no child
   element is written self-closing (`<X/>`), including one written in
   open/close form in the source, with or without attributes; one with
   children gets an end tag on its own line (answers O-13,
   `o-answers-2/out-o13/mixed-forms.xml`). **Text content inside a node
   is dropped** (answers O-13, `o-answers-2/out-o13/content.xml`).
4. **Blank lines and trailer.** No empty line before the first
   `<BehaviorTree>` or inside a tree (true of every output; no input had
   a blank line inside a tree). Between two `<BehaviorTree>`
   elements, **exactly one** empty line, whether the source had none or
   several (answers O-13, `o-answers-2/out-o13/two-trees-blank.xml`,
   `two-trees-tab.xml`). After the last `</BehaviorTree>`: one empty
   line, then at body indent the comment
   `<!-- Description of Node Models (used by Groot) -->`, then the
   `<TreeNodesModel>` block (or `<TreeNodesModel/>` when empty), then one
   empty line, then `</root>` and a **single trailing LF**. Where
   `<include>` elements in a tree file go is not covered (§7, O-17).
5. **Characters.** Line ends are LF, CRLF input included (answers O-9:
   `in-o9-awkward/crlf.xml` has CRLF line ends, its save
   `in-o9-awkward/crlf-rt.xml` none). Attribute values
   are double-quoted. In values, `&` is written `&amp;`, `<` `&lt;`, `>`
   `&gt;`, `"` `&quot;` and `'` `&apos;`; non-ASCII characters are
   written as literal UTF-8; character references are resolved to the
   character (answers O-9, O-13; fixtures `in-o9-awkward/charrefs-rt.xml`,
   `o-answers-2/out-o13/attr-escapes.xml`). **All source comments are
   dropped**; only the comment in rule 4 is written (answers O-9).
6. **Attribute set and order on a node.** A node of a **declared**
   model is written with `name` first when it has one (a missing `name`
   is not added: fixture `o-answers-2/out-o14/tree.xml`), then **every
   port of its model, in the model's declaration order**, whatever subset
   the source set. A port the source did not set is written with an empty value
   (`x=""`) (answers O-10, `o-answers-2/out-o10/tree.xml`). The
   empty-port emission is a setting of the reference, recorded as
   configured; coppice takes it as a setting with that default.
   - **Declared models:** the order is the model entry's port order.
     Every input sets its ports already in that order and has `name`
     first, so reordering a declared model's ports, and moving `name`
     to the front, are a **HYPOTHESIS** (§7, O-17).
   - **Built-in nodes:** the library declares their ports in an
     unordered container (BT 4.9.0
     `include/behaviortree_cpp/basic_types.h:599`, `PortsList`), so it
     gives no order. The one multi-port built-in in the fixtures,
     `SetBlackboard`, was reordered to the order its ports appear in the
     library source (BT 4.9.0
     `include/behaviortree_cpp/actions/set_blackboard_node.h:45-50`):
     `<SetBlackboard output_key="k" value="1"/>` was written `value`
     first (answers O-12, `o-answers-2/out-o12/tree.xml`). That save was
     in V3 mode. coppice writes built-in ports in their library source
     order and writes only the ports the source set; both are a
     **HYPOTHESIS** for V4, and for built-ins other than `SetBlackboard`
     (§7, O-17).
   - **Subtree nodes** (`<SubTree>`, and `<SubTreePlus>` in v3) are
     written with their `ID` first and nothing added: the one fixture,
     a V3-mode save, writes `<SubTree ID="Side"/>` with no `_autoremap`,
     though the v4 library declares that port (BT 4.9.0
     `src/decorators/subtree_node.cpp:17`). For V4 this is a
     **HYPOTHESIS**. Where `name` goes, remapping attributes, and
     `_autoremap` set in the source are not covered (§7, O-17).
   - Other attributes that are neither `name` nor a port (`ID` on the
     explicit `<Action ID="…">` form, `_skipIf` and the other reserved
     attributes) are not covered (§7, O-17).
7. **Attribute wrapping.** In tree files, a tag with **two or more
   attributes** is written one attribute per line: the first on the
   tag's line, each further one on its own continuation line indented to
   the first attribute's column. A tag with zero or one attribute stays
   on one line in every output, but none has a single long value, so
   that a long value alone stays on one line is a **HYPOTHESIS** (§7,
   O-17 g). Above the
   count, value length has no effect (answers O-10:
   `o-answers-2/out-o10/tree.xml`, where a 200-character value wraps
   exactly as short values do). The rule covers the root (rule 2), body
   nodes and model rows. Project files never wrap (§3.3).
8. **Which models the `<TreeNodesModel>` block holds:** exactly the
   declared models the tree **uses**, and an empty `<TreeNodesModel/>`
   when it uses none (answers O-14, `o-answers-2/out-o14/tree.xml` and
   `unused.xml`; consistent with `out-o6/tree.xml`, the `out-o2-*` trees
   and `in-o9-awkward/comments-rt.xml`). A model is used when a node's
   element name equals its `ID`. The models come from the project's
   merged list when the tree is saved through a project (§2.3.1; the
   models in `o-answers-2/out-o10/tree.xml` and `out-o14/tree.xml` are
   declared in the project files `o-answers-2/out-o10-corrected/proj.btproj`
   and `o-answers-2/in-o14-models/proj.btproj`, not in the tree file),
   plus, in V3 mode, the importer's models (§3.5); and from the file's
   own blocks when it is saved on its own (§3.4). The explicit
   `<Action ID="…"/>` form and subtree models are not covered by the
   fixtures, so for them the rule is a **HYPOTHESIS**.
9. **The model block's layout** (answers O-6, O-10, O-12): entries
   sorted by `ID` (every fixture `ID` is capitalized ASCII, so byte order
   and case-insensitive order agree; coppice uses byte order, a
   **HYPOTHESIS**); each entry is `<Kind ID="…" [editable="true"]>` with
   its ports as children, in declaration order, each
   `<input_port|output_port|inout_port name="…" [type="…"]/>`, where
   `editable` and `type` are written when the declaration has them
   (`o-answers-2/out-o10/tree.xml`: no `editable`;
   `o-answers-2/out-o14/tree.xml`: `editable="true"`);
   `<description>` is dropped; an entry with no
   ports is self-closing. Rules 3 and 7 apply (fixtures `out-o6/tree.xml`,
   `o-answers-2/out-o10/tree.xml`, `o-answers-2/out-o12/tree.xml`). The
   placement of a port's `default` attribute and of its description text
   is not covered (§7, O-17).

The comment in rule 4 is a format element the reference writes into
every file, and it already appears in public ecosystem project files
(surface §3); coppice emits it for byte compatibility in reference mode
only, and never otherwise.

**Version stamp** (answers O-1, O-2, O-16). The written stamp is always
the reference's **current target** (4.10 for Groot2 v1.10.0), in tree
files and project files alike:

- a file with no stamp gets the target;
- an **older** stamp (4.9) is rewritten to the target, after a prompt on
  opening that announces it (fixtures `out-o2-diff/proj.btproj`,
  `o-answers-2/out-o16b/stamped.xml`);
- a **newer** stamp (4.11) makes the reference offer to switch its own
  target to the newer version; once switched, every save writes the
  newer stamp (fixtures `o-answers-2/out-o16a/t411.xml`,
  `o-answers-2/out-o16a/proj.btproj`). Lowering the target while such a
  project is open is refused.

coppice takes the target as a setting (default 4.10) and writes it on
every reference save. Where that raises an existing stamp, coppice
reports a diagnostic in place of the reference's prompt. A stamp
**newer** than the target is never lowered: the reference instead
switches its target or refuses, so coppice refuses the reference save
and names the stamp, and the user can set the target to it. V3-mode
saves write no stamp (§3.5).

**Inputs the reference refuses to load** (§3.6) are refused by coppice's
reference save too, with the diagnostic.

### 3.3 Reference project export (F8) — answers O-1, O-2, O-4, O-6, O-11, O-12, O-14, O-16

The reference never writes a project file in place; its only project
write is an **export to a directory** (answers O-8). coppice's
`save --reference` of a project is that export, and it takes the
destination directory as a required argument.

- **Project file** (fixtures `out-o2-same/proj.btproj`,
  `out-o4/proj.btproj`, `out-o6/proj.btproj`,
  `o-answers-2/out-o12-export/proj.btproj`,
  `o-answers-2/out-o14-export/proj.btproj`): line 1 is the declaration;
  `<root>` stays on **one line** with attributes in order `BTCPP_format`,
  `BTCPP_version` (V4 only), then the rest in source order (e.g.
  `project_name`); the body is indented **4 spaces per level**; the
  `<include>` elements come first, then the comment of §3.2 rule 4, then
  the `<TreeNodesModel>` block; then `</root>` and a single trailing LF.
  **No empty lines**, and **no attribute wrapping** anywhere in the
  project file, model rows included.
- **Models** (answers O-6, O-14): the project file holds **every** model
  the project declares, used or not
  (`o-answers-2/out-o14-export/proj.btproj`), laid out as in §3.2
  rule 9 but unwrapped.
- **Include order:** sorted (`out-o4/proj.btproj`,
  `o-answers-2/out-o16b-export/proj.btproj`, where the source order was
  the reverse; `o-answers-2/out-o11/proj.btproj` was already in order).
  Sorting by the written path and by basename agree wherever they can
  differ in the fixtures; which it is, is a **HYPOTHESIS**.
- **Include paths and tree files — NOT ESTABLISHED.** The record holds
  two different export behaviors, and the variable that separates them
  is not measured (answers, the exporter's file-copy behavior):
  - **copy and flatten** (the first session: answers O-2, O-4, O-6;
    fixtures `out-o2-same/`, `out-o2-diff/`, `out-o4/`, `out-o6/`): every
    included tree is copied beside the exported project in the canonical
    tree style (§3.2), and every `<include path>` is written as the
    basename;
  - **project file only** (the second session: answers O-11, O-12, O-14,
    O-16): no tree file is written to the destination; each `<include>`
    points, relative to the new project file, at the tree's last-saved
    location (where that is beside the new project, as in
    `o-answers-2/out-o16a/proj.btproj`, the path is unchanged); a
    modified tree is saved in place at its source path.

  F8 waits on that measurement (§7, O-18). Whatever it shows, coppice's
  export never overwrites an existing file: the destination must be a new
  or empty directory, and an export whose includes cannot all be resolved
  is refused before anything is written. Under the project-only behavior,
  two includes sharing a basename cannot collide (answers O-11).

### 3.4 Reference save-as of a standalone tree (F9) — answers O-5, O-13

Saving a tree file opened on its own (not through a project) to a new
path writes a **bare tree file** under the chosen name, in the canonical
tree style of §3.2: no project wrapper, no `<include>`, no
`project_name`, the `BehaviorTree ID` unchanged, and the model block of
§3.2 rule 8 (fixtures `out-o5/in-o5-single.xml` and the five
`o-answers-2/out-o13/` files). This corrects dossier G-1's one-include
wrapper, which was the project export route.

### 3.5 Reference save in V3 mode (F7) — answers O-12

A V3-mode save **normalizes with the same writer as V4** (fixtures
`o-answers-2/out-o12/tree.xml` and
`o-answers-2/out-o12-export/proj.btproj`; the set's CRLF variant,
`tree-crlf.xml`, saved to the same bytes, but the record does not show
how that input reached a save, so CRLF handling rests on O-9, §3.2
rule 5):

- every rule of §3.2 applies, the attribute order of rule 6 included;
- **no version stamp**: the tree root is `<root BTCPP_format="3">` and
  the project root `<root BTCPP_format="3" project_name="…">`;
- v3 spellings are kept as written (`SetBlackboard`, `SubTree`,
  `SubTreePlus`); nothing is translated to v4.

On opening, the reference offers to create a model for a node type the
tree uses but no `<TreeNodesModel>` declares, with the detected ports
filled in; accepted with its defaults, the created model is written like
any declared one (answers O-12, additional behaviors; fixture
`o-answers-2/out-o12/tree.xml`, where `Script`, which the v3 dialect does
not build in, became `<Action ID="Script" editable="true">` with one
untyped `<input_port name="code"/>`).

coppice cannot prompt, so in V3 mode it applies those defaults and
reports a diagnostic naming each model it created: an **Action** model,
`editable="true"`, whose ports are the node's attributes other than
`name`, as untyped input ports in source order. The fixture shows one
node with one attribute; that the defaults are the same for other nodes
(another kind, several attributes, several uses with different
attributes) is a **HYPOTHESIS** (§7, O-17). Which node types V3 mode
treats as built-in is not in the record, beyond `Script` not being one
and `Sequence`, `SetBlackboard` and `SubTree` getting no model
(`o-answers-2/out-o12/tree.xml`; §7, O-17). In V4 mode, the
reference's behavior for an undeclared custom node is not in the record,
so coppice refuses that reference save with a diagnostic naming the
node. Fresh V3 saves (dossier G-5) are L3.

### 3.6 Load rules seen in the reference (F10)

The reference refuses to load three kinds of input (answers, additional
behaviors; answers O-1, O-10, O-13):

- **An Action node with a child element.** That is BehaviorTree.CPP's own
  loader rule, which applies to Actions and Conditions (BT 4.9.0
  `src/xml_parsing.cpp:671-677`).
- **A node attribute that is not a port of its model**, shown for the
  built-in `Script` (an attribute other than `code`:
  `o-answers-2/in-o10-wrap/tree.xml`,
  `o-answers-2/in-o13-escape/attr-escapes.xml`). That the reference
  refuses the same for a declared model is a **HYPOTHESIS**. The library refuses it
  too, when it builds the tree rather than when it parses: an attribute
  that is a valid port name but not a port of the node's registered type
  (BT 4.9.0 `src/xml_parsing.cpp:805-819`).
- **A declared model whose `ID` is a built-in node's**, at least one that
  adds ports to the built-in (the analysis side's attempt to extend
  `Script`). Whether a declaration that agrees with the built-in, as the
  library itself writes into monitor and log XML, is refused too is not
  in the record. The library uses its built-in either way.

coppice parses all three (parsing never instantiates, §2.5) and reports a
diagnostic: `LeafWithChildren`, `UnknownPort` and `ModelShadowsBuiltin`.
`UnknownPort` covers declared models and built-ins: coppice takes the
built-in port lists from the library source (`BUILTIN_PORTS`; for
`Script`, BT 4.9.0 `include/behaviortree_cpp/actions/script_node.h:46`:
`code` only). `ModelShadowsBuiltin` fires only on a leafness mismatch, as
the library decides (§2.5); a used declaration of a built-in is refused by
the writer instead (§3.7). A reference-mode save of any input the reference refuses is
refused.

### 3.7 What coppice's reference writer refuses (F6, F7, F9)

The reference writer follows §3.2 to §3.5: rules established from the
fixtures, and the rules those sections state as a **HYPOTHESIS** (for
example a single long attribute value kept on one line, rule 7; a source
that puts the stamp first, rule 2; port reordering for declared models
and the case of the model sort key, rules 6 and 9). Where those sections
give no rule at all, it refuses the save and names the case
(`NotInRecord`) rather than guessing. So its output matches the reference
wherever the record establishes the bytes, and follows a stated
hypothesis elsewhere. It refuses:

- **any input with a diagnostic** (§2.5, §2.3.1): the inputs the reference
  refuses to load (§3.6) are among them, and a missing format, a malformed
  model block or a duplicate are not covered. The diagnostics check
  built-in ports by the v4 lists in either mode, so in V3 mode a `Script`
  with an attribute other than `code` is refused as `UnknownPort`, though
  the writer otherwise treats V3 `Script` as not built in;
- a **stamp newer than the target** (`NewerStamp`, §3.2), and a stamp or
  target that is not a canonical `MAJOR.MINOR` (`4.10`, not `4.010`,
  `4.10.1` or `5`);
- a `BTCPP_format` other than `4` in V4 mode or `3` in V3 mode, and a
  stamp in V3 mode;
- a root attribute other than `BTCPP_format`, `BTCPP_version` and
  `project_name`; a root child other than `<BehaviorTree>` and
  `<TreeNodesModel>` (an `<include>`, an unknown element); a
  `<BehaviorTree>` without an `ID` or with another attribute (O-17 c, d);
- what the library's loader refuses and the record does not show the
  reference writing (BT 4.9.0 `src/xml_parsing.cpp:109-127,566-640`): a
  `<BehaviorTree>` with an empty `ID`, two trees with the same `ID`, a
  tree without exactly one child node; a subtree node without an `ID`,
  with an empty one, one naming a built-in or failing the library's
  name check (empty, `Root`, or a forbidden character), or with children;
  a custom node type whose name fails that check; a decorator without
  exactly one child, a control without children, a `TryCatch` with
  fewer than two, and a `ReactiveSequence` with more than one
  `AsyncSequence` child. Two library rules the record shows the reference does
  **not** apply to tree IDs are not refused: a tree `ID` naming a
  built-in (fixture `out-o4/tree.xml`, `ID="SubTree"`) and one with a
  forbidden character (`in-o9-awkward/charrefs-rt.xml`, `ID="Ain&amp;tree"`);
- the explicit `<Action|Condition|Control|Decorator ID="…">` form; an
  attribute that is neither `name` nor a port name (a reserved attribute
  such as `_skipIf`); a subtree node with an attribute other than `ID`
  (O-17 a, b);
- a `<BehaviorTree>` or `<TreeNodesModel>` used as a node inside a tree;
- a subtree whose `ID` has a declared `SubTree` model, and a node named
  after a declared `SubTree` model (O-17 f); `<SubTreePlus>` in V4 mode,
  which v4 does not build in, is an undeclared node;
- a used declared model with a port `default` (empty or not), port
  description text, `<MetadataFields>`, `editable` other than `"true"`,
  any other attribute, child element, text or CDATA, a port declared
  twice, or a port named like a reserved attribute such as `name` (O-17
  e), wherever the model was declared; a declared model, used, whose `ID` is
  a built-in's (O-17 i); an attribute on `<TreeNodesModel>`;
- an attribute value containing a tab or a line break, in the tree or in
  a used model;
- a DOCTYPE, a processing instruction, a byte-order mark or CDATA
  anywhere, the model block included, and text outside a node (rule 5
  covers comments; rule 3 covers text inside a node);
- in V4 mode, a node type that is neither built in nor declared
  (`UndeclaredNode`, §3.5);
- in V3 mode: a v4 built-in the record does not place (only `Sequence`,
  `SetBlackboard` and `SubTree` are shown built in, and `Script` shown not
  built in); and an undeclared node type that has children or is used
  with different attributes (O-17 h).

As a last check, the writer parses its own output, and refuses to return
output that is not well-formed; the refusals above are meant to make that
unreachable.

What the writer changed that the reference would have announced in a
prompt is returned as a note: a raised stamp (`StampRaised`) and a model
created by the V3 importer defaults (`ModelCreated`).

## 4. Recordings — `.btlog` (F11, F12)

ESTABLISHED, surface §1 and dossier WP2/G-2. The reference's own recorder
writes this exact format (dossier G-2).

```
offset    size  field
0         18    ASCII "BTCPP4-FileLogger2", no NUL
18        1     uint8 protocol version = 1
19        4     int32 N, length of the XML
23        N     UTF-8 tree XML (runtime form: _uid, _fullpath, built-in models)
23+N      8     int64 T0, microseconds since the Unix epoch
31+N      9·k   k transition records, to end of file
```

Transition record, 9 bytes:

| bytes | field |
|-------|-------|
| 0–5 | low 48 bits of the offset from T0, in microseconds (unsigned) |
| 6–7 | `uint16` node uid (`_uid` in the XML) |
| 8 | `uint8` status: 0 IDLE, 1 RUNNING, 2 SUCCESS, 3 FAILURE, 4 SKIPPED |

### 4.1 Reading (F11)

- The magic and version are checked exactly; a mismatch is a typed error
  naming the offset and the bytes found.
- The file must end on a record boundary; trailing bytes are a typed error
  that still returns the records read so far.
- Statuses are the plain enum. The `10+prev` encoding never appears in a
  log. IDLE transitions are logged. A record carries only the *new* status,
  so "idle after success" is derived by tracking each uid's last status.
- Records are in nearly non-decreasing time order (the writer batches); the
  reader preserves file order and does not sort.
- A uid with no node in the XML is a diagnostic, not an error.
- Absolute time = T0 + offset.

### 4.2 Writing (F12)

- Writes the layout above exactly; the XML is written verbatim as given.
- Offsets are truncated to 48 bits. Status values outside 0–4 are refused.
- **Atomic output:** the file appears complete or not at all. This
  deliberately differs from the reference recorder, which creates the file
  at 0 bytes when recording starts, buffers everything, and writes it when
  recording stops (dossier G-2). The byte content when complete is the
  same; coppice simply never leaves an empty or partial file behind.

## 5. Live-monitor client (F13–F16, L1)

ESTABLISHED, surface §2, dossier G-3/G-4/G-5. The reference client speaks
**one wire protocol (v4) in both dialect modes**; the dialect is only in the
served XML (dossier G-5).

### 5.1 Sockets

- **REQ** to `tcp://<host>:<port>`, default port **1667**.
- **SUB** to `tcp://<host>:<port+1>` (default **1668**), subscribed to all
  messages. The PUB side carries only `BREAKPOINT_REACHED`.
- Host and port are user-supplied. The 4.11 bind-address option is a
  server-side setting; it changes nothing for the client.

### 5.2 Framing

Requests and replies are ZMQ multipart messages.

- **Request frame 1**, 6 bytes: `uint8 protocol = 2`, `uint8 type` (tag,
  below), `uint32 unique_id`.
- **Reply frame 1**, 22 bytes: the same 6 bytes (`unique_id` echoed), then
  a 16-byte `tree_id`. The `tree_id` is fixed for the lifetime of a
  publisher instance, so **a changed `tree_id` means the publisher
  restarted** and the client must re-fetch FULLTREE.
- **Error reply:** two frames, the literal `error` and the error text.
- The client checks every reply: protocol byte, echoed type, echoed
  `unique_id`. A mismatch is a typed error. A REQ socket that loses a reply
  is unusable, so the client closes and reconnects it.

### 5.3 Request tags

| Tag | Name | Request frames | Reply payload |
|-----|------|----------------|---------------|
| `T` | FULLTREE | header | tree XML (runtime form, fixed for the publisher's lifetime) |
| `S` | STATUS | header | status buffer (§5.4) |
| `B` | BLACKBOARD | header + `;`-separated names | MessagePack of a JSON object `{name → dump}` |
| `I` | HOOK_INSERT | header + hook JSON, object or array | none |
| `R` | HOOK_REMOVE | header + `{"uid":n,"position":p}` | none |
| `U` | BREAKPOINT_UNLOCK | header + `{"uid","desired_status","position","remove_when_done"}` | none |
| `D` | HOOKS_DUMP | header | JSON array of hooks |
| `A` | REMOVE_ALL_HOOKS | header | none |
| `X` | DISABLE_ALL_HOOKS | header | none |
| `r` | TOGGLE_RECORDING | header + `start` or `stop` | `start`: the epoch in µs, as a decimal string; `stop`: none |
| `t` | GET_TRANSITIONS | header | packed 9-byte records (as §4); the buffer is cleared by the read |
| `N` | BREAKPOINT_REACHED | — | published on SUB: the node uid as a decimal string |

"none" means the reply is the 22-byte header alone. That is a reading of
BT 4.9.0 `src/loggers/groot2_publisher.cpp:291-491`; the F13 oracle
confirms it for every tag v1 sends.

v1 sends `T`, `S`, `B`, `r` and `t`. The hook tags are L1, specified in §5.7.

### 5.4 Session, polling and status (F13, F14)

- **Connect:** the session is up once a FULLTREE round-trip succeeds. There
  is no handshake.
- **Heartbeat:** the publisher treats more than **5000 ms** without any
  request as a dead client and disables all hooks. The client keeps its
  interval between requests well under that.
- **Cadence is the client's choice;** the protocol is permissive. The
  reference was observed at about **10 Hz** STATUS polling after one
  FULLTREE on connect (dossier G-5). coppice defaults to 10 Hz, with the
  rate configurable.
- **Reconnect** policy is the client's choice. OPEN: the reference's
  back-off (§7, O-7). coppice uses bounded exponential back-off and never
  floods: one failure produces one report, not one per poll. The record
  shows the reference flooding on a mismatch (dossier G-4); that is not to
  be reproduced.
- **STATUS buffer:** 3 bytes per node in tree order: `uint16 uid`,
  `uint8 status`. Status is the plain enum (0–4), or **10 + previous
  status** for a node that has transitioned to IDLE (11 = idle after
  RUNNING, 12 = after SUCCESS, 13 = after FAILURE, 14 = after SKIPPED). A
  conformant stream never emits 10 (dossier G-4).
  - The two public sources differ on the edge. Surface §2's decoder note
    accepts 0–4 and **10–14** and treats anything else as IDLE. The
    reference client accepts 0–4 and **11–14** and rejects everything else,
    10 included (dossier G-4).
  - coppice decodes 0–4 and 11–14 as specified. It decodes 10 (idle after
    idle) and any other value as IDLE, with a diagnostic, and never
    disconnects over a status byte. That accepts everything both sources
    accept.

### 5.5 Blackboard (F15)

- The request's frame 2 lists subtree names separated by `;` (a name is an
  instance name or a tree ID). The reply is **MessagePack** of a JSON
  object mapping each name to its blackboard dump. A name with no matching
  subtree is skipped: it is absent from the reply, which is not an error
  (BT 4.9.0 `src/loggers/groot2_publisher.cpp:546-565`, the `find` check
  at `:555`).
- Values are arbitrary JSON: nested objects, arrays and numbers. coppice
  keeps them as a generic JSON value.
- 4.11: a subtree whose blackboard is backed by a distinct root blackboard
  is exported under the reserved name **`ROOT`**, deduplicated, and an
  ambiguous request gets an error reply (surface, version-delta note).
  coppice must accept `ROOT` as a key.

### 5.6 Recording a live session (F16)

All of this subsection is publisher behavior from BT 4.9.0
`src/loggers/groot2_publisher.cpp`, except the record flow, which is
reference behavior from dossier G-2.

- **Start** (`r` + `start`, `:438-458`): sets recording on, takes a new time
  base, **clears the transition buffer**, and replies with the current
  epoch time in µs as a decimal string. Every later record's offset is
  measured from that time base (`callback`, `:212-217`). The publisher
  takes the base and the reply time from two different clocks, so the
  reply is the best available T0, not an exact one. coppice records that
  as a stated limit, not an error.
- **Drain** (`t`, `:466-483`): returns every buffered record, packed 9 bytes
  each in `.btlog` record layout, and clears the buffer.
- **Overflow** (`:221-224`): the buffer holds at most **1000** records, and
  older ones are dropped silently. A drain that returns exactly 1000
  records therefore *may* have lost records. coppice reports such a drain
  as a possible gap. It is a heuristic, since the publisher gives no
  explicit signal. The polling rate must drain faster than the tree fills
  the buffer.
- **Stop** (`r` + `stop`, `:459-462`): sets recording off. It neither
  returns nor clears the buffer, so records captured since the last drain
  are still there.
- **Ending a recording** is therefore three steps, in order: send `stop`,
  send one final `t` drain, then write the file. Writing on `stop` alone
  loses the tail.
- **Writing the file:** coppice writes a `.btlog` (§4.2) from the FULLTREE
  XML, T0 taken from the start reply, and every drained record in order.
- **Record flow** (dossier G-2): the destination path is chosen *before*
  recording starts. The file is written when recording stops, atomically
  in coppice's case (see §4.2 for how that differs from the reference).
- **Shared-publisher hazard:** the buffer and the time base are
  per-publisher, not per-client. A second client's `start` clears the
  buffer and resets the base, which silently invalidates the first
  client's T0; its drains also take records the first client never sees.
  The publisher offers nothing a client can use to detect this. coppice
  documents the hazard in its recording command's help and does not claim
  to detect it.

### 5.7 Hooks (L1) — contract only, not in v1

- A hook is `{enabled: bool, uid: uint16, mode: 0|1 (BREAKPOINT|REPLACE),
  position: 0|1 (PRE|POST), once: bool, desired_status:
  "SUCCESS"|"FAILURE"|"RUNNING"|"IDLE"|"SKIPPED"}` (surface §2).
- Semantics are **server-authoritative**: whether a breakpoint pauses the
  tree is decided by the publisher's pre-tick callback. A pause ends on
  `BREAKPOINT_UNLOCK`, or by the heartbeat rule (§5.4). `BREAKPOINT_REACHED`
  arrives on SUB (dossier G-3).
- Observed reference client behavior (dossier G-3), recorded so coppice can
  deliberately differ: it sends `HOOK_INSERT` as soon as a hook is
  configured; it never sends `BREAKPOINT_UNLOCK`; replace-node inserts are
  silent on PUB; its "once" option does not appear on the wire; removal is
  `HOOK_REMOVE`. coppice will send `once` as the protocol defines it, and
  will offer unlock.

## 6. Version deltas to track

- BehaviorTree.CPP **4.11** adds `BTCPP_version` to `<root>` (and the
  reference's V4 save stamps `"4.10"`, §3.2), a bind-address publisher
  constructor (server side only), and the `ROOT` blackboard name (§5.5).
  None changes a layout pinned above. Every fixture records the library
  version it came from.

## 7. Open questions — needed from the analysis side

Each is a functional fact about the reference that the public record does
not yet state precisely enough to implement. Each answer must reach the
public research record before it enters this spec (see `clean-room.md`).

| # | Question | Blocks | Needed form |
|---|----------|--------|-------------|
| O-1, O-2, O-4, O-5, O-6, O-8, O-9 | answered (`dossiers/groot2-o-answers.md`) | — | — |
| O-10 to O-16 | answered (`dossiers/groot2-o-answers-2.md`), with the remainders split out below | — | — |
| O-3 | V3 TreeNodesModel rule: which models are embedded on a fresh save and which are dropped | L3 (not v1) | input/output pairs covering built-in, declared-custom and included models |
| O-7 | The reference's reconnect back-off after a publisher restart | F13 | a timing description; non-blocking (coppice's own policy stands) |
| O-17 | Writer cases the fixtures do not cover: (a) which ports of a **built-in** node are written, in what order, and whether unset ones are written empty (e.g. `Sleep`, `Repeat`, `RetryUntilSuccessful`, `SubTree` with and without `_autoremap`); (b) attributes that are neither `name` nor a port: `ID` on the explicit `<Action ID="…">` form, `_skipIf` and the other reserved attributes, subtree remappings; (c) root attributes other than the format, version and project name, such as `main_tree_to_execute`; (d) `<include>` in a tree file; (e) a port's `default` attribute and description text in the written model block; (f) model selection for the explicit form and for subtree models; (g) whether a tag with one long attribute value wraps; (h) the V3-mode model importer's defaults beyond one node with one attribute, and which node types V3 mode treats as built-in; (i) whether a declared model that agrees with a built-in is refused; (j) a V4 save of a node whose declared model's ports, or `name`, are set out of order in the source; (k) whether the reference refuses an attribute that is not a port of a **declared** model; (l) the model block's sort key for IDs differing in case | F6, F7, F9 | one saved input per case |
| O-18 | Project export: which tree files are written to the destination, and how include paths are rewritten. The two recorded sessions differ (copy and flatten in O-2, O-4 and O-6; project file only, includes repointed, in O-11/12/14/16); the candidate variables are the export dialog's per-row selection, whether the trees were opened or modified before the export, and the dialog's pre-filled destination | F8 | the same project exported with its rows selected and not selected |

## 8. Compatibility oracle

### Fixtures

All committed fixtures are self-authored or openly licensed, each with
provenance in `tests/fixtures/README.md`. Vendor sample files are never
committed.

1. **Documents:** self-authored tree XML and `.btproj` in both dialects,
   plus the example trees shipped in BehaviorTree.CPP's own repository
   (MIT, with the version and path recorded). A deliberately awkward set
   exercises F5: comments everywhere, unusual attribute order and quoting,
   CRLF and LF line endings, no trailing newline, entities and character
   references, unknown elements and attributes.
2. **Reference saves:** the two answers fixture sets (self-authored
   inputs and the reference's own written outputs), copied from the
   research record at `d3ca343` into `tests/fixtures/reference/o-answers/`
   and `tests/fixtures/reference/o-answers-2/`, provenance in
   `tests/fixtures/README.md`. Which input produced which output (paths
   under `o-answers/` unless they start `o-answers-2/`):

   | Feature | Input | Route | Reference output |
   |---|---|---|---|
   | F6 | `in-o2-same-corrected/tree.xml` (identical bytes to the tree the O-8 save started from; that save ran inside the `in-o1-proj-corrected` project) | per-file save, unmodified | `in-o1-proj-corrected/tree.xml` (the reference's rewrite) |
   | F8 | `in-o2-same-corrected/` | project export, copy-and-flatten behavior (O-18) | `out-o2-same/` |
   | F8 | `in-o2-diff-corrected/` (older stamp, upgrade chosen) | project export, copy-and-flatten behavior (O-18) | `out-o2-diff/` |
   | F8 | `in-o6-model/` | project export, copy-and-flatten behavior (O-18) | `out-o6/` |
   | F6 | `in-o4-paths/sub/tree.xml`, `in-o4-paths/deep/nest/tree2.xml`, `in-o4-paths/abs.xml`, `outside.xml` (the O-4 export's trees, as single trees) | canonical tree write | `out-o4/tree.xml`, `out-o4/tree2.xml`, `out-o4/abs.xml`, `out-o4/outside.xml` |
   | F8 | `in-o4-paths/` (with `outside.xml`, its `../` include) | project export, copy-and-flatten behavior (O-18) | `out-o4/` |
   | F6 | `o-answers-2/out-o10-corrected/` (the corrected input; a 5-port model, nodes setting 2 or 3 of them) | per-file save | `o-answers-2/out-o10/tree.xml` |
   | F6 | `o-answers-2/in-o14-models/` | per-file save of each tree | `o-answers-2/out-o14/tree.xml`, `o-answers-2/out-o14/unused.xml` |
   | F6 | `o-answers-2/in-o16-stamps/proj-stamp.btproj` (its `stamped.xml` carries an older stamp; target kept) | per-file save of each tree | `o-answers-2/out-o16b/stamped.xml`, `o-answers-2/out-o16b/ordered.xml` |
   | F6 | `o-answers-2/in-o16-stamps/proj-411.btproj` (a newer stamp; target switched to 4.11) | per-file save | `o-answers-2/out-o16a/t411.xml` |
   | F7 | `o-answers-2/in-o12-v3/` (V3 mode; the project includes `tree.xml`, and the importer's model for `Script` is created as §3.5 states) | save of the tree (the export's include, `../out-o12/tree-crlf.xml`, suggests the tree was last saved under a new name; the bytes are the same either way) | `o-answers-2/out-o12/tree.xml` (`o-answers-2/out-o12/tree-crlf.xml` is byte-identical, but the record does not show how the CRLF input reached a save, so it is not an oracle row) |
   | F8 | `o-answers-2/in-o14-models/`, `o-answers-2/in-o16-stamps/proj-stamp.btproj`, `o-answers-2/in-o12-v3/` (V3), `o-answers-2/out-o11-corrected/` (target 4.10) | project export, project-file-only behavior (O-18) | `o-answers-2/out-o14-export/`, `o-answers-2/out-o16b-export/`, `o-answers-2/out-o12-export/proj.btproj`, `o-answers-2/out-o11/proj.btproj` |
   | F8 | `o-answers-2/in-o16-stamps/proj-411.btproj` (target switched to 4.11) | project export, project-file-only behavior (O-18) | `o-answers-2/out-o16a/proj.btproj` |
   | F9 | `o-answers-2/in-o13-escape/content.xml`, `two-trees-tab.xml`; `o-answers-2/out-o13-corrected/attr-escapes.xml`, `mixed-forms.xml`, `two-trees-blank.xml` | standalone save-as | the same names under `o-answers-2/out-o13/` |
   | F9 | `in-o5-single.xml` | standalone save-as | `out-o5/in-o5-single.xml` |
   | F9 | each `in-o9-awkward/<name>.xml` | standalone save-as | `in-o9-awkward/<name>-rt.xml` |
   | F10 | `in-o1-proj/`, `in-o2-same/`, `in-o2-diff/` (an Action with a child) | load | refused: a diagnostic, and no reference-mode save (§3.6) |
   | F10 | `o-answers-2/in-o11-collide/b/tree.xml`, `o-answers-2/in-o13-escape/mixed-forms.xml`, `two-trees-blank.xml` (an Action with a child) | load | refused, as above |
   | F10 | `o-answers-2/in-o10-wrap/`, `o-answers-2/in-o13-escape/attr-escapes.xml` (a `Script` attribute that is not its port) | load | refused: a diagnostic, and no reference-mode save (§3.6) |
3. **Logs:** `.btlog` files generated in CI by BehaviorTree.CPP 4.9.0's own
   `FileLogger2`, running self-authored trees.
4. **Monitor:** a live BehaviorTree.CPP 4.9.0 `Groot2Publisher`, built in CI
   from source and running a self-authored tree; and `btview mock`
   (Apache-2.0) as a second, independent publisher.

### Expected results and tolerances

| Feature | Oracle | Tolerance |
|---------|--------|-----------|
| F1–F4, F10 | parse of fixture set 1, and the F10 rows of the set-2 table (each refused with a diagnostic naming the node: the leaf that has a child, or the attribute that is not a port); diagnostics as a reviewed snapshot | exact |
| F5 | lossless save, compared with the input, over the awkward set; and a negative control: a fixture pair differing only by one substituted whitespace byte between attributes (space versus tab) must produce outputs that differ in exactly that byte, so a normalizing writer fails | byte-exact |
| F6 | the F6 rows of the table above: reference-mode per-file save, compared with the reference's output | byte-exact |
| F7 | the F7 row of the table above: V3-mode reference save | byte-exact |
| F8 | the F8 rows of the table above: reference-mode export to a fresh directory. Until O-18 settles which behavior applies, the project file is compared with its include lines excluded, and the tree files only for the copy-and-flatten rows | byte-exact on what is compared |
| F9 | the F9 rows of the table above: reference-mode save-as, compared with the reference's output | byte-exact |
| F11 | `FileLogger2` takes T0 privately at construction (BT 4.9.0 `src/loggers/bt_file_logger_v2.cpp:78,107`), so the harness records each transition's uid, status and **absolute** timestamp through a second, absolute-mode status logger on the same tree. Both loggers are notified with the same time point. The harness then reads T0 from the file. coppice's decode must give the same uid and status, and offsets equal to the harness's absolute time minus T0 | exact |
| F12 | from scratch: coppice writes from the harness's XML, the T0 read from the `FileLogger2` file, and the harness's transition list (offsets computed as for F11), with no decoded record involved; the output must equal the `FileLogger2` file. Plus write-then-read | byte-exact |
| F13, F15 | session against the BT.CPP harness publisher. The harness ticks only on command and is quiescent while the client samples. Tree XML equals the harness's own `WriteTreeToXML` output; blackboard equals the harness's dump; replies to every v1 tag are checked against §5.3 | exact |
| F13 (`btview mock`) | connect, FULLTREE, STATUS and BLACKBOARD parse with no error, and the decoded tree matches the mock's served XML | exact on what the mock serves |
| F14 | the harness drives scripted transitions (RUNNING halted to IDLE, RUNNING to SUCCESS to IDLE, FAILURE to IDLE, SKIPPED to IDLE), and coppice decodes 11, 12, 13 and 14 as idle-after-RUNNING, -SUCCESS, -FAILURE and -SKIPPED, not as plain IDLE. Unit tests on synthetic buffers cover 10 and out-of-range bytes, which no conformant publisher emits | exact |
| F16 | live recording from the harness compared with a `FileLogger2` run on the same tree and the same scripted ticks; plus a test that drains exactly 1000 records and expects the possible-gap report | same uid/status sequence; absolute times (T0 + offset) within one tick |

Reference re-baselining against Groot2 v1.10.0 itself happens on the
analysis side and returns as updated public fixtures. CI never needs the
reference product.
