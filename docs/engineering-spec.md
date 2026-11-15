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
     `39c7acc`: the Groot2 dossier (`dossiers/groot2.md`), cited as
     `dossier WP1`…`G-6` by work package; the compatibility-surface
     study (`docs/groot2-compat-surface.md`), cited as `surface §1`…`§6`;
     and the save-behavior answers (`dossiers/groot2-o-answers.md`, with
     the fixture set `fixtures/groot2/o-answers/`), cited as
     `answers O-n` or `answers, canonical style`. Facts read directly from
     a published reference-output file are cited by that file, as
     `fixture <path>`;
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
| F3 | Parse `.btproj` projects and resolve `<include>` | C3 | yes (surface §3, dossier WP1) | none | none | §2.3 |
| F4 | Parse `<TreeNodesModel>`, including tolerated extensions | C3 | yes (surface §3) | yes — kinds, ports, `editable`, `<description>`, metadata, merged blocks | partial — self-authored and reference fixtures | §2.4 |
| F5 | Lossless save (coppice's default): load then save unmodified, byte-identical | C3 | n/a — the reference has no lossless save (answers O-9); this is coppice's own guarantee | yes | partial — byte-exact over the self-authored fixture set plus the one-byte negative control | §3.1 |
| F6 | Reference save of a tree file (`--reference`): the canonical writer | C3 | partial — canonical style established (answers); attribute wrapping and escaping beyond the fixtures OPEN | none | none | §3.2 |
| F7 | Reference save in V3 mode | C3 | partial — dossier G-1 only; whether V3 saves also normalize is OPEN (O-12) | none | none | §3.5 |
| F8 | Reference project export: flatten includes, copy trees, embed models | C3 | partial (answers O-1, O-2, O-4, O-6; include order, model selection and duplicate basenames are open) | none | none | §3.3 |
| F9 | Reference save-as of a standalone tree: a bare canonical tree | C3 | yes (answers O-5) | none | none | §3.4 |
| F10 | Dialect detection and port-versus-model diagnostics | C3 | yes (dossier WP1, WP3) | partial — on a single document: dialect suggestion, missing format, leaf with children, unknown port, misplaced node ID, malformed, duplicate or built-in-shadowing models, decided as BehaviorTree.CPP's loader decides (BT 4.9.0 `src/xml_parsing.cpp:557-677,763-850`). Not yet across a project (models in the project file, trees in included files: F3), nor on logs or the monitor tree | partial — the F10 reference fixtures (the leaf-with-child trees) and inline inputs | §2.5, §3.6 |
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
file was modified (answers O-8: an unmodified tree is rewritten). Byte by
byte, as fixtures `out-o5/in-o5-single.xml` and `out-o6/tree.xml` show:

1. Line 1 is `<?xml version="1.0" encoding="UTF-8"?>`.
2. The `<root>` tag is split: `<root BTCPP_format="4"` on its own line,
   then `BTCPP_version="<target>">` on a continuation line indented to the
   column of the first attribute (6 spaces).
3. The body is indented **2 spaces per level**. An element with no
   children is written self-closing (`<X/>`); one with children gets an
   end tag on its own line.
4. After the last `</BehaviorTree>`: one empty line, then at body indent
   the comment `<!-- Description of Node Models (used by Groot) -->`,
   then the `<TreeNodesModel>` block (or `<TreeNodesModel/>` when empty),
   then one empty line, then `</root>` and a **single trailing LF**.
5. Line ends are LF; attribute values are double-quoted; character
   references are resolved and `&` is written as `&amp;` (fixture
   `in-o9-awkward/charrefs-rt.xml`); **all source comments are dropped**;
   only the comment in rule 4 is written (answers O-9).
6. Attribute order within a tag is kept (answers O-1).
7. **Attribute wrapping:** in tree files, every two-attribute tag the
   fixtures show (`<root>` and model tags) has its second attribute on a
   continuation line aligned to the first attribute's column (fixture
   `out-o6/tree.xml`: 6, 12 and 15 spaces). No tree-file tag in the
   fixtures has three or more attributes (project-file roots with three
   stay on one line, as project files never wrap).
   That the rule is "two or more attributes" rather than a length
   threshold is a **HYPOTHESIS**: the answers doc proposes a length band,
   but its one-line counter-example is in a project file, where nothing
   wraps, and no tree-body node in the fixtures has two attributes
   (§7, O-10).
8. **Which models the `<TreeNodesModel>` block holds:** every written
   tree in the fixtures holds exactly the declared models that the tree
   **uses** (a model is used when a node's element name equals the
   model's `ID`; the explicit `<Action ID="…"/>` form and subtree
   references are not covered by the fixtures), and nothing else. `out-o6/tree.xml` uses three declared
   models and embeds all three; `out-o2-same/tree.xml`,
   `out-o2-diff/tree.xml` and `in-o1-proj-corrected/tree.xml` use none of
   their project's declared model and embed nothing; `comments-rt.xml`
   drops a model its tree never uses. This rule is a **HYPOTHESIS**: it
   fits every written file, and no fixture has a tree using some declared
   models but not others (§7, O-14). The answers doc's statement that a
   project export embeds the project's models into every copied tree file
   is contradicted by the `out-o2-*` exports.

The comment in rule 4 is a format element the reference writes into
every file, and it already appears in public ecosystem project files
(surface §3); coppice emits it for byte compatibility in reference mode
only, and never otherwise.

**Version stamp** (answers O-1, O-2). Measured: a file with no stamp gets
`BTCPP_version="<target>"` (4.10 for Groot2 v1.10.0), immediately after
`BTCPP_format`; a project already stamped with the target keeps it; a
project stamped with an **older** minor version (4.9) is written with the
target, after the user chose to upgrade in a prompt the reference shows
on opening such a file. coppice takes the target as a setting with that
default, and when it rewrites an existing stamp it reports a diagnostic
in place of the prompt. Not measured, so HYPOTHESIS until O-16: a newer
stamp than the target, an existing stamp in a tree file, and a stamp not
immediately after `BTCPP_format`.

Not established, so not yet byte-exact (§7): escaping of `<`, `>`, `"` and
`'` in attribute values and of characters in text, text content inside
nodes, and the spacing between two or more `<BehaviorTree>` elements
(O-13); tags with three or more attributes, and wrapping of body nodes
(O-10); model selection (O-14).

### 3.3 Reference project export (F8) — answers O-1, O-2, O-4, O-6

The reference never writes a project file in place; its only project
write is an **export to a directory** (answers O-8). coppice's
`save --reference` of a project is that export, and it takes the
destination directory as a required argument. The destination must be a
new or empty directory, and coppice never overwrites an existing file:
the reference's export overwrites whatever shares a written file's name,
including the project file and, because includes are flattened to
basenames, any source tree that happens to sit in the destination
(answers O-4). An export whose includes cannot all be resolved is refused
before anything is written.

- **Project file** (fixtures `out-o2-same/proj.btproj`,
  `out-o4/proj.btproj`, `out-o6/proj.btproj`): line 1 is the declaration;
  `<root>` stays on **one line** with attributes in order `BTCPP_format`,
  `BTCPP_version`, then the rest in source order (e.g. `project_name`);
  the body is indented **4 spaces per level**; the `<include>` elements
  come first, then the comment of §3.2 rule 4, then the
  `<TreeNodesModel>` block; then `</root>` and a single trailing LF.
  **No empty lines**, and **no attribute wrapping** in the project file.
- **Includes are flattened** (answers O-4): every `<include path>` is
  written as the included file's **basename**, whatever the input form
  (subdirectory, `..`, nested, absolute), and every included tree is
  **copied beside the exported project** in the canonical tree style
  (§3.2). The fixture shows the includes **sorted by basename**
  (`out-o4/proj.btproj`); one sample only, so a HYPOTHESIS. Two includes
  with the same basename are not covered (O-11).
- **Models** (answers O-6): entries sorted alphabetically by `ID`;
  `editable` kept; `<description>` dropped; ports kept. The project file
  holds every declared model; each copied tree holds the models it uses
  (§3.2 rule 8, a HYPOTHESIS).

### 3.4 Reference save-as of a standalone tree (F9) — answers O-5

Saving a tree file opened on its own (not through a project) to a new
path writes a **bare tree file** under the chosen name, in the canonical
tree style: no project wrapper, no `<include>`, no `project_name`, the
`BehaviorTree ID` unchanged, and the model block of §3.2 rule 8, empty in
the fixture because the tree declares no models (fixture
`out-o5/in-o5-single.xml`). This corrects dossier G-1's one-include
wrapper, which was the project export route.

### 3.5 Reference save in V3 mode (F7)

Dossier G-1 records a byte-identical V3 in-place round-trip on its
corpus, but that corpus may already have been in canonical form, and the
answers set contains no V3 input. Whether V3-mode saves also normalize,
and to which style, is OPEN (O-12); F7 waits on it. Fresh V3 saves
(dossier G-5) are L3.

### 3.6 A loader rule seen in the reference (F10)

The reference refuses to load a tree in which an Action node has a child
element (answers, additional behaviors). That is BehaviorTree.CPP's own
loader rule, which applies to Actions and Conditions (BT 4.9.0
`src/xml_parsing.cpp:671-677`). coppice parses such a file (parsing never instantiates,
§2.5) and reports a diagnostic naming the node; a reference-mode save of
it is refused.

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
| O-1, O-2, O-4, O-5, O-6, O-8, O-9 | answered (answers doc and fixtures, research `39c7acc`), with the remainders split out below | — | — |
| O-3 | V3 TreeNodesModel rule: which models are embedded on a fresh save and which are dropped | L3 (not v1) | input/output pairs covering built-in, declared-custom and included models |
| O-7 | The reference's reconnect back-off after a publisher restart | F13 | a timing description; non-blocking (coppice's own policy stands) |
| O-10 | Attribute wrapping in tree files: is it every tag with two or more attributes, or a length threshold? Does it apply to body nodes? How are three or more attributes laid out? | F6 | tree-file saves with body nodes of 2 and 3 short attributes, and of 1 very long attribute |
| O-11 | Project export when two includes share a basename (`a/tree.xml`, `b/tree.xml`) | F8 | an input/output pair |
| O-12 | Does a V3-mode save normalize, and to what style? | F7 | V3 inputs with the O-9 awkward properties, saved in V3 mode |
| O-13 | Escaping, content and layout in the canonical writer: `<`, `>`, `"`, `'` and non-ASCII in attribute values; text content inside a node; spacing between two or more `<BehaviorTree>` elements | F6 | one input per property, saved |
| O-14 | Model selection: when a tree uses some of its declared models but not others, which go into the written tree's `<TreeNodesModel>`, on per-file save, on export, and on a standalone save-as? | F6, F8, F9 | a project with two declared models whose tree uses one, saved per file and exported; and a standalone tree that declares two models and uses one, saved-as |
| O-15 | The O-4 input `../outside.xml` was not published (only its hash is in the answers doc), so the O-4 export cannot be replayed in full | F8 | the input file itself |
| O-16 | Version stamp cases not measured: a newer stamp than the target; an existing stamp in a tree file; a stamp not immediately after `BTCPP_format` | F6, F8 | one input per case, saved |

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
2. **Reference saves:** the answers fixture set (self-authored inputs and
   the reference's own written outputs), copied from the research record
   at `39c7acc` into `tests/fixtures/reference/o-answers/`, provenance in
   `tests/fixtures/README.md`. New answers (O-10 to O-16) arrive the same
   way. Which input produced which output:

   | Feature | Input (`tests/fixtures/reference/o-answers/…`) | Route | Reference output |
   |---|---|---|---|
   | F6 | `in-o2-same-corrected/tree.xml` (identical bytes to the tree the O-8 save started from; that save ran inside the `in-o1-proj-corrected` project) | per-file save, unmodified | `in-o1-proj-corrected/tree.xml` (the reference's rewrite) |
   | F8 | `in-o2-same-corrected/` | project export | `out-o2-same/` |
   | F8 | `in-o2-diff-corrected/` (older stamp, upgrade chosen) | project export | `out-o2-diff/` |
   | F8 | `in-o6-model/` | project export | `out-o6/` |
   | F6 | `in-o4-paths/sub/tree.xml`, `in-o4-paths/deep/nest/tree2.xml` (as single trees; O-15: the full O-4 export is not replayable) | canonical tree write | `out-o4/tree.xml`, `out-o4/tree2.xml` |
   | F9 | `in-o5-single.xml` | standalone save-as | `out-o5/in-o5-single.xml` |
   | F9 | each `in-o9-awkward/<name>.xml` | standalone save-as | `in-o9-awkward/<name>-rt.xml` |
   | F10 | `in-o1-proj/`, `in-o2-same/`, `in-o2-diff/` (an Action with a child) | load | refused: a diagnostic, and no reference-mode save (§3.6) |
3. **Logs:** `.btlog` files generated in CI by BehaviorTree.CPP 4.9.0's own
   `FileLogger2`, running self-authored trees.
4. **Monitor:** a live BehaviorTree.CPP 4.9.0 `Groot2Publisher`, built in CI
   from source and running a self-authored tree; and `btview mock`
   (Apache-2.0) as a second, independent publisher.

### Expected results and tolerances

| Feature | Oracle | Tolerance |
|---------|--------|-----------|
| F1–F4, F10 | parse of fixture set 1, and the F10 rows of the set-2 table (each refused with a diagnostic naming the leaf node that has a child); diagnostics as a reviewed snapshot | exact |
| F5 | lossless save, compared with the input, over the awkward set; and a negative control: a fixture pair differing only by one substituted whitespace byte between attributes (space versus tab) must produce outputs that differ in exactly that byte, so a normalizing writer fails | byte-exact |
| F6 | the F6 row of the table above: reference-mode per-file save, compared with the reference's output | byte-exact |
| F8 | the F8 rows of the table above: reference-mode export to a fresh directory; every file the reference wrote is compared, and no other file may be written | byte-exact |
| F9 | the F9 rows of the table above: reference-mode save-as, compared with the reference's output | byte-exact |
| F7 | waits on O-12 | — |
| F11 | `FileLogger2` takes T0 privately at construction (BT 4.9.0 `src/loggers/bt_file_logger_v2.cpp:78,107`), so the harness records each transition's uid, status and **absolute** timestamp through a second, absolute-mode status logger on the same tree. Both loggers are notified with the same time point. The harness then reads T0 from the file. coppice's decode must give the same uid and status, and offsets equal to the harness's absolute time minus T0 | exact |
| F12 | from scratch: coppice writes from the harness's XML, the T0 read from the `FileLogger2` file, and the harness's transition list (offsets computed as for F11), with no decoded record involved; the output must equal the `FileLogger2` file. Plus write-then-read | byte-exact |
| F13, F15 | session against the BT.CPP harness publisher. The harness ticks only on command and is quiescent while the client samples. Tree XML equals the harness's own `WriteTreeToXML` output; blackboard equals the harness's dump; replies to every v1 tag are checked against §5.3 | exact |
| F13 (`btview mock`) | connect, FULLTREE, STATUS and BLACKBOARD parse with no error, and the decoded tree matches the mock's served XML | exact on what the mock serves |
| F14 | the harness drives scripted transitions (RUNNING halted to IDLE, RUNNING to SUCCESS to IDLE, FAILURE to IDLE, SKIPPED to IDLE), and coppice decodes 11, 12, 13 and 14 as idle-after-RUNNING, -SUCCESS, -FAILURE and -SKIPPED, not as plain IDLE. Unit tests on synthetic buffers cover 10 and out-of-range bytes, which no conformant publisher emits | exact |
| F16 | live recording from the harness compared with a `FileLogger2` run on the same tree and the same scripted ticks; plus a test that drains exactly 1000 records and expects the possible-gap report | same uid/status sequence; absolute times (T0 + offset) within one tick |

Reference re-baselining against Groot2 v1.10.0 itself happens on the
analysis side and returns as updated public fixtures. CI never needs the
reference product.
