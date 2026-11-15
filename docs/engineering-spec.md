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
- **Research record (the only sources of this spec):** the Groot2 dossier
  (`dossiers/groot2.md`) and the compatibility-surface study
  (`docs/groot2-compat-surface.md`) in
  [exbound/research](https://github.com/exbound/research), at commit
  `bc319c2`. Section references below are written `dossier §…` and
  `surface §…`.
- **Capabilities in scope here:** C2 data (`.btlog`), C3 document (projects
  and tree XML, both dialects), C6 programmability (the live-monitor client).
  C8 (the graphical application) is out of scope for v1.

Conventions: **ESTABLISHED** = stated in the research record, with its
section; **HYPOTHESIS** = a reading of the record that a test must confirm;
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
| F1 | Parse tree XML, v4 dialect, all elements and attributes | C3 | yes (surface §5) | none | none | §2.1 |
| F2 | Parse tree XML, v3 dialect | C3 | yes (dossier G-5) | none | none | §2.2 |
| F3 | Parse `.btproj` projects and resolve `<include>` | C3 | yes (surface §3, dossier WP1) | none | none | §2.3 |
| F4 | Parse `<TreeNodesModel>`, including tolerated extensions | C3 | yes (surface §3) | none | none | §2.4 |
| F5 | Lossless round-trip: load then save unmodified, byte-identical | C3 | yes (dossier G-1) | none | none | §3.1 |
| F6 | Reference save, V4: add the `BTCPP_version` stamp | C3 | partial — placement bytes OPEN | none | none | §3.2 |
| F7 | Reference save, V3: add nothing; the TreeNodesModel drop | C3 | partial — drop rule OPEN | none | none | §3.3 |
| F8 | Save-as: rewrite include paths relative to the new location | C3 | partial — algorithm OPEN | none | none | §3.4 |
| F9 | Save-as of a single tree: wrap as a one-include project | C3 | yes (dossier G-1) | none | none | §3.5 |
| F10 | Dialect detection and port-versus-model diagnostics | C3 | yes (dossier WP1, WP3) | none | none | §2.5 |
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
| L3 | Editing operations on the document model | C3 | n/a | must preserve F5 for untouched regions |
| L4 | Graphical application (canvas, monitor, replay, blackboard) | C8 | partial | after a GUI spike; not specified here |
| L5 | v3 binary wire translator (legacy publisher to v4 client) | C6 | partial (surface §2 cross-check) | the v3 wire is not needed by the reference client (dossier G-5) |

Explicitly **not** reproduced: the free-tier node cap and feature gating
(dossier G-6: a product decision, not a protocol constraint), licensing
behavior, and vendor UI text.

## 2. Documents — parsing

### 2.1 Tree XML, v4 dialect (F1) — ESTABLISHED, surface §5

- A document is `<root …>` containing, in any order: zero or more
  `<include path="…"/>`, one or more `<BehaviorTree ID="…">`, and at most one
  `<TreeNodesModel>`.
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
- The parser accepts any well-formed XML in this shape, including unknown
  elements and attributes, and keeps them (F5 depends on it).

### 2.2 Tree XML, v3 dialect (F2) — ESTABLISHED, dossier G-5

The v3 dialect uses the same document shape with: `SetBlackboard` as a node,
`SubTree` and `SubTreePlus` for subtrees, and `BTCPP_format="3"` (accepted,
not enforced). The parser must treat `SubTree` and `SubTreePlus` as distinct
spellings and never translate one into the other.

### 2.3 Projects (F3) — ESTABLISHED, surface §3, dossier WP1/WP3

- A `.btproj` is a tree-XML document whose `<root>` carries `project_name`
  and which typically holds `<include>` references and a `<TreeNodesModel>`.
- `<include path>` is resolved relative to the including file's directory.
  All 15 files of the reference's own sample corpus resolve this way
  (dossier WP1).
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
- Tolerated extensions written by the reference: an `editable="true"`
  attribute on model elements, and a `<description>` child element. Both
  must parse and must survive F5. Whether the reference preserves them on
  its own save is OPEN (§7, O-6).

### 2.5 Dialect detection and diagnostics (F10)

- **Dialect** is decided from content, since `BTCPP_format` is advisory:
  `BTCPP_format="3"`, or a v3-only node (`SetBlackboard`, `SubTreePlus`),
  means v3; otherwise v4. HYPOTHESIS: this agrees with the reference's own
  mismatch detection (dossier WP3 records that it offers a mode switch on
  mismatch, not the rule it applies).
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

The governing requirement: **coppice's save output for a given input is the
reference's save output for that input, byte for byte.** Two behaviors follow
from the record.

### 3.1 Lossless round-trip (F5) — ESTABLISHED, dossier G-1

Outside the specific changes in §3.2–3.4, the reference round-trips
**byte-exact**: the `<TreeNodesModel>` block, comments, attribute order,
indentation and XML escaping. A V3 in-place round-trip is byte-identical to
its input. So:

- Loading then saving an unmodified document with no stamp change must
  reproduce the input bytes exactly: attribute order and quoting,
  whitespace inside and between tags, comments, the XML declaration,
  character references and entity spelling, line endings, and the presence
  or absence of a trailing newline.
- **Test:** byte equality of the output against the input (or against the
  reference's saved form where §3.2–3.4 apply). A normalized or semantic
  comparison does not pass.
- Node layout is **not** a document property: the reference stores no canvas
  coordinates in either file type (dossier G-1). coppice must not write any.

### 3.2 Reference save, V4 (F6) — ESTABLISHED in part, dossier G-1

- On save in V4 the reference **adds `BTCPP_version="4.10"`** to `<root>`,
  unconditionally.
- **Placement differs by file type:** in a `.btproj` the attribute goes
  **inline**, on the same line as `<root>`'s existing attributes; in a tree
  XML file it goes on **its own indented continuation line**.
- OPEN: the exact bytes of both placements (separator, indentation width
  and character, position relative to the other attributes), and the
  behavior when `BTCPP_version` is already present, with the same or a
  different value (§7, O-1, O-2).
- coppice also offers a **lossless save** (F5 with no stamp), as an explicit
  option. The reference-compatible save is the default.

### 3.3 Reference save, V3 (F7) — ESTABLISHED in part, dossier G-1/G-5

- V3 adds **nothing** to `<root>`; an in-place round-trip is byte-identical.
- A **fresh** V3 save declares `BTCPP_format="3"`, preserves `SubTree`
  spelling, and embeds **no** built-in node models in `<TreeNodesModel>`.
  The record adds that "only models loaded from included files get
  embedded". OPEN: the exact rule, as an input-to-output mapping (§7, O-3).
- "Fresh save" means saving a document created in the editor, so it
  belongs with L3. F7's v1 obligation is the byte-identical in-place
  round-trip.

### 3.4 Save-as and include rewriting (F8) — ESTABLISHED in part, dossier G-1

- On save-as, each `<include path>` is rewritten **relative to the saved
  file's own location**: a bare filename when the included file sits in the
  same directory, a relative path otherwise.
- OPEN: `..` segments versus other forms, the path separator, and whether
  an absolute input path stays absolute (§7, O-4).

### 3.5 Single-tree save-as (F9) — ESTABLISHED, dossier G-1

- The reference saves only **projects**: save-as on a single opened tree
  file does not rewrite that tree; it writes a new `.btproj` with one
  `<include>` pointing to the tree.
- A bare tree file is rewritten only when its content was modified.
- OPEN: the bytes of the wrapper project the reference creates (§7, O-5).

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
- **Atomic output:** the file appears complete or not at all. That matches
  the reference recorder, which buffers the whole recording and writes it
  when recording stops (dossier G-2).

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
  - coppice decodes 0–4 and 11–14. A 10, or any other value, decodes as
    IDLE with a diagnostic. This follows the open decoder note (surface §2),
    not the reference client's stricter rejection (dossier G-4); it accepts
    everything the reference accepts.

### 5.5 Blackboard (F15)

- The request's frame 2 lists subtree names separated by `;` (a name is an
  instance name or a tree ID). The reply is **MessagePack** of a JSON
  object mapping each name to its blackboard dump. HYPOTHESIS, confirmed by
  the F15 oracle: unknown names are omitted from the reply rather than
  raising an error.
- Values are arbitrary JSON: nested objects, arrays and numbers. coppice
  keeps them as a generic JSON value.
- 4.11: a subtree whose blackboard is backed by a distinct root blackboard
  is exported under the reserved name **`ROOT`**, deduplicated, and an
  ambiguous request gets an error reply (surface, version-delta note).
  coppice must accept `ROOT` as a key.

### 5.6 Recording a live session (F16)

- `r` with `start` begins server-side capture and returns T0 (epoch µs,
  decimal string). `t` drains the captured 9-byte records. The server's
  buffer is capped at 1000 records, so the client must drain faster than
  the tree fills it.
- The client writes a `.btlog` (§4.2) from: the FULLTREE XML, T0 from the
  start reply, and the drained records in order. `r` with `stop` ends the
  capture.
- **Record flow** (dossier G-2): the destination path is chosen *before*
  recording starts, and the file is written atomically when recording
  stops. An interrupted recording leaves no partial file. A missed drain
  (buffer overflow) is a reported gap, never silent.
- Draining with `t` clears the shared buffer (surface §2), so a
  second client reading the same publisher loses records. coppice warns
  when it detects this (a `start` reply while capture is already active is
  the HYPOTHESIS signal).

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
| O-1 | Exact bytes of the `BTCPP_version` stamp: inline in `.btproj`, and as a continuation line in tree XML (separator, indentation, position among attributes) | F6 | a self-authored input and the reference's saved output, both files |
| O-2 | Stamp behavior when `BTCPP_version` is already present, with the same and with a different value | F6 | as O-1 |
| O-3 | V3 TreeNodesModel rule: which models are embedded on save and which are dropped | F7 | input/output pairs covering built-in, declared-custom and included models |
| O-4 | Include rewriting: `..` paths, separators, absolute inputs, nested directories | F8 | input/output pairs |
| O-5 | The wrapper project written by save-as of a single tree | F9 | an input/output pair |
| O-6 | Whether the reference preserves `editable` and `<description>` in models on save | F4/F5 | an input/output pair |
| O-7 | The reference's reconnect back-off after a publisher restart | F13 | a timing description; non-blocking (coppice's own policy stands) |

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
2. **Reference saves (O-1 to O-6):** self-authored inputs, plus the output
   the reference produced when saving each one. They are captured on the
   analysis side, published in the research record, then copied here with
   their provenance.
3. **Logs:** `.btlog` files generated in CI by BehaviorTree.CPP 4.9.0's own
   `FileLogger2`, running self-authored trees.
4. **Monitor:** a live BehaviorTree.CPP 4.9.0 `Groot2Publisher`, built in CI
   from source and running a self-authored tree; and `btview mock`
   (Apache-2.0) as a second, independent publisher.

### Expected results and tolerances

| Feature | Oracle | Tolerance |
|---------|--------|-----------|
| F1–F4, F10 | parse of fixture set 1; diagnostics as a reviewed snapshot | exact |
| F5 | load then save, compared with the input | byte-exact |
| F6–F9 | save, compared with the reference's output (fixture set 2) | byte-exact |
| F11 | decode compared with the producing program's own record of what it wrote (uid, status, offset per transition) | exact |
| F12 | write a decoded log back and compare with the original | byte-exact |
| F13–F15 | session against both publishers: tree XML equals the publisher's own `WriteTreeToXML` output; status and blackboard equal the publisher-side state | exact, sampled per tick |
| F16 | live recording compared with a `FileLogger2` run on the same tree and ticks | same uid/status sequence; offsets within one tick |

Reference re-baselining against Groot2 v1.10.0 itself happens on the
analysis side and returns as updated public fixtures. CI never needs the
reference product.
