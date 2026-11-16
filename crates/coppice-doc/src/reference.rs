//! The reference writer (F6, F7, F9): a tree file written byte for byte as
//! the reference application writes it (spec §3.2, §3.4, §3.5).
//!
//! The writer is a canonical serialization: nothing lexical survives from
//! the source. Where the record does not say what the reference writes,
//! the writer refuses rather than guesses (spec §3.7), so its output is
//! either the reference's bytes or nothing.

use std::collections::BTreeMap;

use crate::tree::{
    Diagnostic, DiagnosticKind, ModelKind, ModelLookup, NodeModel, Port, PortDirection,
    TreeDocument, builtin_ports, is_builtin, is_port_name, model_lookup,
};
use crate::{Document, Element, Node};

/// The comment the reference writes before the model block (spec §3.2
/// rule 4). A file-format element, written in reference mode only.
const MODELS_COMMENT: &str = "<!-- Description of Node Models (used by Groot) -->";

/// The dialect mode the reference writer runs in (spec §2.5, §3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteMode {
    /// V4: the root carries the version stamp.
    #[default]
    V4,
    /// V3 compatibility: no stamp; undeclared custom nodes get the
    /// model-importer's default model.
    V3,
}

/// Settings of the reference writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceOptions {
    /// The target version written as `BTCPP_version` in V4 mode (spec
    /// §3.2, *Version stamp*). Default `4.10`, the reference's.
    pub target: String,
    /// The dialect mode.
    pub mode: WriteMode,
    /// Write every port of a declared model, unset ones as `""` (spec §3.2
    /// rule 6). Default on, as the reference is configured.
    pub write_unset_ports: bool,
}

impl Default for ReferenceOptions {
    fn default() -> Self {
        ReferenceOptions {
            target: "4.10".to_owned(),
            mode: WriteMode::V4,
            write_unset_ports: true,
        }
    }
}

/// A written tree file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The file's bytes.
    pub text: String,
    /// What the write changed that the reference would have announced in
    /// a prompt: a raised version stamp, a model created by the V3
    /// importer defaults.
    pub notes: Vec<Diagnostic>,
    /// The `ID`s of the models written into the model block, sorted: the
    /// declared models the tree uses, and any the V3 importer created.
    pub models: Vec<String>,
}

/// The V3-mode node types the record shows getting no model (spec §3.5).
const V3_BUILTINS_SEEN: &[&str] = &["Sequence", "SetBlackboard", "SubTree"];
/// The V3-mode node types the record shows are not built in (spec §3.5).
const V3_NOT_BUILTIN_SEEN: &[&str] = &["Script"];

/// Write `tree` as the reference writes a tree file, with the node models
/// `models` (a project's merged list, or the file's own for a standalone
/// save). Refuses, with the reasons, any input the reference refuses to
/// load and any case the record does not cover (spec §3.7).
pub fn write_tree(
    tree: &TreeDocument,
    models: &[NodeModel],
    options: &ReferenceOptions,
) -> Result<Written, Vec<Diagnostic>> {
    write_parts(tree, models, options).map_err(|r| {
        let mut all = r.tree;
        all.extend(r.models.into_iter().map(|(_, d)| d));
        all.sort_by_key(|d| d.offset);
        all
    })
}

/// Why a write was refused, split by where each reason's offset points:
/// into the tree document, or into the document that declared a model
/// (named by its `ID`), which for a project save may be another one.
pub(crate) struct Refused {
    pub(crate) tree: Vec<Diagnostic>,
    pub(crate) models: Vec<(String, Diagnostic)>,
}

/// [`write_tree`], with the refusals about declared models kept apart.
pub(crate) fn write_parts(
    tree: &TreeDocument,
    models: &[NodeModel],
    options: &ReferenceOptions,
) -> Result<Written, Refused> {
    let mut refusals = tree.diagnostics_with_models(models);
    let mut notes = Vec::new();
    let lookup = model_lookup(models);
    let root = tree.document().root_element();

    // Root attributes (rule 2, *Version stamp*).
    let format = root.attribute("BTCPP_format");
    let mut stamp: Option<String> = None;
    for a in &root.attributes {
        match a.name.as_str() {
            "BTCPP_format" | "project_name" => {}
            "BTCPP_version" => {}
            _ => refusals.push(uncovered(
                a.name_span.start,
                format!("the root attribute {} (spec §7, O-17 c)", a.name),
            )),
        }
    }
    match (options.mode, format) {
        (_, None) => {} // MissingFormat is already a diagnostic.
        (WriteMode::V4, Some("4")) | (WriteMode::V3, Some("3")) => {}
        (_, Some(f)) => refusals.push(uncovered(
            root.span.start,
            format!("BTCPP_format=\"{f}\" in {:?} mode", options.mode),
        )),
    }
    let version = root.attribute("BTCPP_version");
    if options.mode == WriteMode::V4 && parse_version(&options.target).is_none() {
        refusals.push(uncovered(
            root.span.start,
            format!(
                "the target \"{}\", which is not MAJOR.MINOR",
                options.target
            ),
        ));
    }
    match options.mode {
        WriteMode::V3 => {
            if version.is_some() {
                refusals.push(uncovered(
                    root.span.start,
                    "a BTCPP_version stamp in V3 mode".to_owned(),
                ));
            }
        }
        WriteMode::V4 => match stamp_order(version, &options.target) {
            Ok(StampChange::Added | StampChange::Kept) => {
                stamp = Some(options.target.clone());
            }
            Ok(StampChange::Raised(from)) => {
                notes.push(Diagnostic {
                    kind: DiagnosticKind::StampRaised,
                    offset: root.span.start,
                    message: format!(
                        "BTCPP_version {from} is written as the target {}",
                        options.target
                    ),
                });
                stamp = Some(options.target.clone());
            }
            Err((kind, message)) => refusals.push(Diagnostic {
                kind,
                offset: root.span.start,
                message,
            }),
        },
    }

    // Document-level content the record does not cover (rule 5 covers
    // comments only).
    for node in tree.document().nodes() {
        let what = match node {
            Node::DocType(_) => Some("a DOCTYPE"),
            Node::ProcessingInstruction(_) => Some("a processing instruction"),
            Node::ByteOrderMark(_) => Some("a byte-order mark"),
            _ => None,
        };
        if let Some(what) = what {
            refusals.push(uncovered(node.span().start, what.to_owned()));
        }
    }
    // Inside <root>: CDATA and processing instructions anywhere, and text
    // outside a node (rule 3 covers text inside a node).
    let mut stack: Vec<(&Element, bool)> = vec![(root, false)];
    while let Some((e, in_node)) = stack.pop() {
        for child in &e.children {
            match child {
                Node::Element(c) => {
                    // Text inside a node is dropped (rule 3); anywhere else
                    // it is refused. Model entries and ports report their
                    // own text (`unrecognized`, `description`), so text is
                    // treated as inside a node below `<TreeNodesModel>`'s
                    // own level.
                    let node = in_node || e.name == "BehaviorTree" || e.name == "TreeNodesModel";
                    stack.push((c, node));
                }
                Node::CData(span) => {
                    refusals.push(uncovered(span.start, "CDATA".to_owned()));
                }
                Node::ProcessingInstruction(span) => {
                    refusals.push(uncovered(span.start, "a processing instruction".to_owned()));
                }
                Node::Text(span) if !in_node && !tree.document().text(span).trim().is_empty() => {
                    refusals.push(uncovered(span.start, "text outside a node".to_owned()));
                }
                _ => {}
            }
        }
    }
    for block in root.child_elements().filter(|e| e.name == "TreeNodesModel") {
        for a in &block.attributes {
            refusals.push(uncovered(
                a.name_span.start,
                format!("the <TreeNodesModel> attribute {}", a.name),
            ));
        }
    }

    // Root children (rules 3, 4): trees and model blocks only.
    let mut trees: Vec<&Element> = Vec::new();
    for child in root.child_elements() {
        match child.name.as_str() {
            "BehaviorTree" => trees.push(child),
            "TreeNodesModel" => {}
            other => refusals.push(uncovered(
                child.span.start,
                format!("a <{other}> element under <root> (spec §7, O-17 d)"),
            )),
        }
    }

    // A file with no tree: rules 4 and 8 presuppose one, and no saved
    // tree file in the record has none.
    if trees.is_empty() {
        refusals.push(uncovered(
            root.span.start,
            "a tree file with no <BehaviorTree>".to_owned(),
        ));
    }

    // Nodes: which models they use, and anything the writer cannot write.
    let mut used: BTreeMap<String, NodeModel> = BTreeMap::new();
    let mut tree_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for t in &trees {
        for a in &t.attributes {
            if a.name != "ID" {
                refusals.push(uncovered(
                    a.name_span.start,
                    format!("the <BehaviorTree> attribute {}", a.name),
                ));
            }
        }
        // The library refuses these (BT 4.9.0 `src/xml_parsing.cpp:571-586`);
        // what the reference does with them is not in the record. A tree
        // ID that names a built-in, which the library also refuses
        // (`:577-581`), is in the record: the reference writes it (fixture
        // `out-o4/tree.xml`, `<BehaviorTree ID="SubTree">`).
        match t.attribute("ID") {
            None | Some("") => refusals.push(uncovered(
                t.span.start,
                "a <BehaviorTree> without an ID, or with an empty one".to_owned(),
            )),
            Some(id) => {
                if !tree_ids.insert(id) {
                    refusals.push(uncovered(
                        t.span.start,
                        format!("a second <BehaviorTree> with the ID {id}"),
                    ));
                }
            }
        }
        if t.child_elements().count() != 1 {
            refusals.push(uncovered(
                t.span.start,
                "a <BehaviorTree> without exactly one child node".to_owned(),
            ));
        }
        // Document order: the importer's model comes from a node type's
        // first use.
        let mut stack: Vec<&Element> = t.child_elements().collect();
        stack.reverse();
        while let Some(e) = stack.pop() {
            let mut children: Vec<&Element> = e.child_elements().collect();
            children.reverse();
            stack.extend(children);
            let before = refusals.len();
            classify(e, &lookup, options, &mut used, &mut refusals, &mut notes);
            // The writer checks exactly what it will write: every source
            // attribute must come out with its value (rule 6 drops none),
            // whatever the diagnostics decided about the node.
            if refusals.len() == before {
                let written = attributes_of(e, false, &lookup, options, &used);
                for a in &e.attributes {
                    let already = refusals.iter().any(|r| r.offset == a.name_span.start);
                    if !already && !written.iter().any(|(k, v)| *k == a.name && *v == a.value) {
                        refusals.push(uncovered(
                            a.name_span.start,
                            format!(
                                "the attribute {} of <{}>, which no rule writes",
                                a.name, e.name
                            ),
                        ));
                    }
                }
            }
        }
    }

    // The declared models must themselves be writable (rule 9).
    let mut model_refusals: Vec<(String, Diagnostic)> = Vec::new();
    for m in used.values() {
        let mut here = Vec::new();
        check_model(m, &mut here);
        model_refusals.extend(here.into_iter().map(|d| (m.id.clone(), d)));
    }
    // Attribute values the record shows no escaping for.
    let mut stack: Vec<&Element> = vec![root];
    while let Some(e) = stack.pop() {
        stack.extend(e.child_elements());
        for a in &e.attributes {
            if a.value.contains(['\t', '\n', '\r']) {
                refusals.push(uncovered(
                    a.value_span.start,
                    "a tab or line break in an attribute value".to_owned(),
                ));
            }
        }
    }

    if !refusals.is_empty() || !model_refusals.is_empty() {
        refusals.sort_by_key(|d| d.offset);
        refusals.dedup();
        model_refusals.dedup();
        return Err(Refused {
            tree: refusals,
            models: model_refusals,
        });
    }

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let mut root_attrs: Vec<(&str, String)> = Vec::new();
    if let Some(f) = format {
        root_attrs.push(("BTCPP_format", f.to_owned()));
    }
    if let Some(v) = &stamp {
        root_attrs.push(("BTCPP_version", v.clone()));
    }
    write_open(&mut out, 0, "root", &root_attrs, true);
    for (i, t) in trees.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        write_element(&mut out, 2, t, &lookup, options, &used);
    }
    out.push('\n');
    out.push_str("  ");
    out.push_str(MODELS_COMMENT);
    out.push('\n');
    if used.is_empty() {
        out.push_str("  <TreeNodesModel/>\n");
    } else {
        out.push_str("  <TreeNodesModel>\n");
        for m in used.values() {
            write_model(&mut out, 4, m);
        }
        out.push_str("  </TreeNodesModel>\n");
    }
    out.push('\n');
    out.push_str("</root>\n");
    // A last check, not a rule: the refusals above are meant to make this
    // unreachable, and no known input reaches it. A writer bug that
    // produced malformed XML is refused here rather than written.
    if let Err(e) = Document::parse(out.as_bytes()) {
        return Err(Refused {
            tree: vec![Diagnostic {
                kind: DiagnosticKind::NotInRecord,
                offset: 0,
                message: format!("the written output is not well-formed ({e}); not written"),
            }],
            models: Vec::new(),
        });
    }
    notes.sort_by_key(|d| d.offset);
    Ok(Written {
        text: out,
        notes,
        models: used.keys().cloned().collect(),
    })
}

enum StampChange {
    Added,
    Kept,
    Raised(String),
}

/// Compare a source stamp with the target (spec §3.2, *Version stamp*).
fn stamp_order(
    source: Option<&str>,
    target: &str,
) -> Result<StampChange, (DiagnosticKind, String)> {
    let Some(source) = source else {
        return Ok(StampChange::Added);
    };
    let (Some(s), Some(t)) = (parse_version(source), parse_version(target)) else {
        return Err((
            DiagnosticKind::NotInRecord,
            format!(
                "the reference's output for BTCPP_version \"{source}\" with the target \"{target}\" is not in the record (not both MAJOR.MINOR)"
            ),
        ));
    };
    match s.cmp(&t) {
        std::cmp::Ordering::Equal => Ok(StampChange::Kept),
        std::cmp::Ordering::Less => Ok(StampChange::Raised(source.to_owned())),
        std::cmp::Ordering::Greater => Err((
            DiagnosticKind::NewerStamp,
            format!(
                "BTCPP_version {source} is newer than the target {target}; set the target to {source} to write it"
            ),
        )),
    }
}

fn parse_version(v: &str) -> Option<(u32, u32)> {
    let (major, minor) = v.split_once('.')?;
    // Canonical decimal numbers only: "4.010" is not "4.10".
    let digits = |s: &str| {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'))
    };
    if !digits(major) || !digits(minor) {
        return None;
    }
    Some((major.parse().ok()?, minor.parse().ok()?))
}

fn uncovered(offset: usize, what: String) -> Diagnostic {
    Diagnostic {
        kind: DiagnosticKind::NotInRecord,
        offset,
        message: format!("the reference's output for {what} is not in the record"),
    }
}

/// Decide how node `e` is written, record the model it uses, and refuse
/// what the writer cannot write (rule 6, §3.5).
fn classify(
    e: &Element,
    lookup: &ModelLookup<'_>,
    options: &ReferenceOptions,
    used: &mut BTreeMap<String, NodeModel>,
    refusals: &mut Vec<Diagnostic>,
    notes: &mut Vec<Diagnostic>,
) {
    let name = e.name.as_str();
    if matches!(name, "Action" | "Condition" | "Control" | "Decorator") {
        refusals.push(uncovered(
            e.span.start,
            format!("the explicit <{name} ID=\"…\"> form (spec §7, O-17 b)"),
        ));
        return;
    }
    if name == "BehaviorTree" || name == "TreeNodesModel" {
        refusals.push(uncovered(
            e.span.start,
            format!("a <{name}> used as a node inside a tree"),
        ));
        return;
    }
    if is_subtree_node(name, options.mode) && lookup.contains_key(name) {
        refusals.push(uncovered(
            e.span.start,
            format!("a declared model {name} that is also a subtree node (spec §7, O-17 i)"),
        ));
        return;
    }
    if is_subtree_node(name, options.mode) {
        match e.attribute("ID") {
            None | Some("") => refusals.push(uncovered(
                e.span.start,
                format!("a <{name}> without an ID, or with an empty one"),
            )),
            // The library refuses these (BT 4.9.0 `src/xml_parsing.cpp:588-600`).
            Some(id) if is_builtin(id) => refusals.push(uncovered(
                e.span.start,
                format!("a <{name}> whose ID is the built-in node {id}"),
            )),
            Some(id) => {
                if let Some(why) = invalid_model_name(id) {
                    refusals.push(uncovered(
                        e.span.start,
                        format!("a <{name}> whose ID {why}"),
                    ));
                }
            }
        }
        check_children(e, Children::Exactly(0), refusals);
        for a in &e.attributes {
            if a.name != "ID" {
                refusals.push(uncovered(
                    a.name_span.start,
                    format!("the <{name}> attribute {} (spec §7, O-17 a, b)", a.name),
                ));
            }
        }
        if let Some(id) = e.attribute("ID")
            && let Some(m) = lookup.get(id)
            && m.kind == ModelKind::SubTree
        {
            refusals.push(uncovered(
                e.span.start,
                format!("a subtree with a declared SubTree model {id} (spec §7, O-17 f)"),
            ));
        }
        return;
    }
    // Attributes that are neither `name` nor a port.
    for a in &e.attributes {
        if a.name != "name" && !is_port_name(&a.name) {
            refusals.push(uncovered(
                a.name_span.start,
                format!("the attribute {} (spec §7, O-17 b)", a.name),
            ));
        }
    }
    // Which node types are built in depends on the mode (§3.5): in V3
    // mode only those the record shows getting no model.
    let builtin = match options.mode {
        WriteMode::V4 => is_builtin(name),
        WriteMode::V3 => V3_BUILTINS_SEEN.contains(&name),
    };
    if builtin {
        if lookup.contains_key(name) {
            refusals.push(uncovered(
                e.span.start,
                format!("a declared model {name} that is also a built-in (spec §7, O-17 i)"),
            ));
        } else {
            check_children(e, builtin_shape(name), refusals);
            // The library refuses a ReactiveSequence with more than one
            // async child: a control registered under one of four names
            // (BT 4.9.0 `src/xml_parsing.cpp:642-668`). `AsyncSequence` is a
            // built-in control; the other three can only be declared ones.
            let async_child = |c: &&Element| {
                c.name == "AsyncSequence"
                    || (matches!(
                        c.name.as_str(),
                        "ThreadedAction" | "StatefulActionNode" | "CoroActionNode"
                    ) && lookup
                        .get(c.name.as_str())
                        .is_some_and(|m| m.kind == ModelKind::Control))
            };
            if name == "ReactiveSequence" && e.child_elements().filter(async_child).count() > 1 {
                refusals.push(uncovered(
                    e.span.start,
                    "a <ReactiveSequence> with more than one async child, which the library refuses".to_owned(),
                ));
            }
        }
        return;
    }
    if options.mode == WriteMode::V3 && is_builtin(name) && !V3_NOT_BUILTIN_SEEN.contains(&name) {
        refusals.push(uncovered(
            e.span.start,
            format!("whether V3 mode treats {name} as built-in (spec §7, O-17 h)"),
        ));
        return;
    }
    // A custom node type: the library validates its name (BT 4.9.0
    // `src/xml_parsing.cpp:604-610`, `:109-127`).
    if let Some(why) = invalid_model_name(name) {
        refusals.push(uncovered(
            e.span.start,
            format!("a node type name that {why}"),
        ));
    }
    if let Some(m) = lookup.get(name) {
        if m.kind == ModelKind::SubTree {
            refusals.push(uncovered(
                e.span.start,
                format!("a node named after the declared SubTree model {name} (spec §7, O-17 f)"),
            ));
            return;
        }
        let shape = match m.kind {
            ModelKind::Control => Children::AtLeast(1),
            ModelKind::Decorator => Children::Exactly(1),
            _ => Children::Any,
        };
        check_children(e, shape, refusals);
        used.entry(m.id.clone()).or_insert_with(|| (*m).clone());
        return;
    }
    match options.mode {
        WriteMode::V4 => refusals.push(Diagnostic {
            kind: DiagnosticKind::UndeclaredNode,
            offset: e.span.start,
            message: format!("node type {name} has no declared model"),
        }),
        WriteMode::V3 => import_model(e, used, refusals, notes),
    }
}

/// Whether `name` is written as a subtree node (`ID` and nothing else):
/// `SubTree`, and `SubTreePlus` in V3 mode only. Both `classify` and
/// `attributes_of` decide by this, so a node is written as it was checked.
fn is_subtree_node(name: &str, mode: WriteMode) -> bool {
    name == "SubTree" || (name == "SubTreePlus" && mode == WriteMode::V3)
}

/// The V3 model importer's defaults (spec §3.5): an Action, editable,
/// whose ports are the node's attributes other than `name`, untyped input
/// ports in source order.
fn import_model(
    e: &Element,
    used: &mut BTreeMap<String, NodeModel>,
    refusals: &mut Vec<Diagnostic>,
    notes: &mut Vec<Diagnostic>,
) {
    let ports: Vec<&str> = e
        .attributes
        .iter()
        .map(|a| a.name.as_str())
        .filter(|n| *n != "name")
        .collect();
    if e.child_elements().next().is_some() {
        refusals.push(uncovered(
            e.span.start,
            format!(
                "the importer's model for {}, which has children (spec §7, O-17 h)",
                e.name
            ),
        ));
        return;
    }
    if let Some(existing) = used.get(&e.name) {
        let same = existing.ports.len() == ports.len()
            && existing.ports.iter().zip(&ports).all(|(p, n)| p.name == *n);
        if !same {
            refusals.push(uncovered(
                e.span.start,
                format!(
                    "the importer's model for {}, used with different attributes (spec §7, O-17 h)",
                    e.name
                ),
            ));
        }
        return;
    }
    notes.push(Diagnostic {
        kind: DiagnosticKind::ModelCreated,
        offset: e.span.start,
        message: format!(
            "node type {} has no declared model; an Action model with ports [{}] is written for it",
            e.name,
            ports.join(", ")
        ),
    });
    used.insert(
        e.name.clone(),
        NodeModel {
            kind: ModelKind::Action,
            id: e.name.clone(),
            ports: ports
                .iter()
                .map(|n| Port {
                    direction: PortDirection::Input,
                    name: (*n).to_owned(),
                    type_name: None,
                    default: None,
                    description: String::new(),
                    unrecognized: Vec::new(),
                })
                .collect(),
            editable: true,
            has_description: false,
            metadata: Vec::new(),
            offset: e.span.start,
            unrecognized: Vec::new(),
        },
    );
}

/// How many child nodes a node type takes, as the library's loader checks
/// it (BT 4.9.0 `src/xml_parsing.cpp:612-640`; leaves are checked as
/// `LeafWithChildren`).
#[derive(Clone, Copy)]
enum Children {
    Any,
    Exactly(usize),
    AtLeast(usize),
}

/// The built-in decorators: one child each (BT 4.9.0 `src/bt_factory.cpp`
/// registrations, `include/behaviortree_cpp/decorators/`).
const BUILTIN_DECORATORS: &[&str] = &[
    "Inverter",
    "RetryUntilSuccessful",
    "KeepRunningUntilFailure",
    "Repeat",
    "Timeout",
    "Delay",
    "RunOnce",
    "ForceSuccess",
    "ForceFailure",
    "Precondition",
    "LoopInt",
    "LoopBool",
    "LoopDouble",
    "LoopString",
    "SkipUnlessUpdated",
    "WaitValueUpdate",
];

fn builtin_shape(name: &str) -> Children {
    if crate::tree::BUILTIN_LEAVES.contains(&name) {
        Children::Any // LeafWithChildren covers leaves.
    } else if BUILTIN_DECORATORS.contains(&name) {
        Children::Exactly(1)
    } else if name == "TryCatch" {
        Children::AtLeast(2)
    } else if name == "SubTree" {
        Children::Exactly(0)
    } else {
        Children::AtLeast(1) // the controls
    }
}

/// Refuse a node whose child count the library's loader refuses; what the
/// reference writes for it is not in the record.
fn check_children(e: &Element, shape: Children, refusals: &mut Vec<Diagnostic>) {
    let n = e.child_elements().count();
    let ok = match shape {
        Children::Any => true,
        Children::Exactly(k) => n == k,
        Children::AtLeast(k) => n >= k,
    };
    if !ok {
        refusals.push(uncovered(
            e.span.start,
            format!(
                "a <{}> with {n} child nodes, which the library refuses",
                e.name
            ),
        ));
    }
}

/// Why the library's `validateModelName` refuses `name`, if it does (BT
/// 4.9.0 `src/xml_parsing.cpp:109-127`, `src/basic_types.cpp:524-550`).
fn invalid_model_name(name: &str) -> Option<&'static str> {
    const FORBIDDEN: &[char] = &[
        ' ', '\t', '\n', '\r', '<', '>', '&', '"', '\'', '/', '\\', ':', '*', '?', '|', '.',
    ];
    if name.is_empty() {
        Some("is empty")
    } else if name == "Root" || name == "root" {
        Some("is the reserved name Root")
    } else if name
        .chars()
        .any(|c| FORBIDDEN.contains(&c) || (c.is_ascii_control()))
    {
        Some("contains a character the library forbids")
    } else {
        None
    }
}

/// Refuse a model entry the writer cannot write (rule 9).
pub(crate) fn check_model(m: &NodeModel, refusals: &mut Vec<Diagnostic>) {
    let mut seen: Vec<&str> = Vec::new();
    for p in &m.ports {
        if seen.contains(&p.name.as_str()) {
            refusals.push(uncovered(
                m.offset,
                format!("model {}'s port {} declared twice", m.id, p.name),
            ));
        }
        seen.push(&p.name);
    }
    let fields = std::iter::once(m.id.as_str())
        .chain(m.ports.iter().map(|p| p.name.as_str()))
        .chain(m.ports.iter().filter_map(|p| p.type_name.as_deref()));
    for f in fields {
        if f.contains(['\t', '\n', '\r']) {
            refusals.push(uncovered(
                m.offset,
                format!("model {}: a tab or line break in an attribute value", m.id),
            ));
        }
    }
    for what in &m.unrecognized {
        refusals.push(uncovered(m.offset, format!("model {}'s {what}", m.id)));
    }
    for p in &m.ports {
        if !is_port_name(&p.name) {
            // `name` and the other reserved names would collide with the
            // node's own attributes.
            refusals.push(uncovered(
                m.offset,
                format!("model {}'s port {}, which is not a port name", m.id, p.name),
            ));
        }
        for what in &p.unrecognized {
            refusals.push(uncovered(
                m.offset,
                format!("model {}'s port {} {what}", m.id, p.name),
            ));
        }
    }
    for p in &m.ports {
        if p.default.is_some() {
            refusals.push(uncovered(
                m.offset,
                format!("model {}'s port {} default (spec §7, O-17 e)", m.id, p.name),
            ));
        }
        if !p.description.trim().is_empty() {
            refusals.push(uncovered(
                m.offset,
                format!(
                    "model {}'s port {} description (spec §7, O-17 e)",
                    m.id, p.name
                ),
            ));
        }
    }
}

/// Write node `e` and its children at `indent`.
fn write_element(
    out: &mut String,
    indent: usize,
    e: &Element,
    lookup: &ModelLookup<'_>,
    options: &ReferenceOptions,
    used: &BTreeMap<String, NodeModel>,
) {
    // Iterative: an explicit stack of (element, indent, closing?). The
    // element at `top_indent` is the `<BehaviorTree>` itself.
    let top_indent = indent;
    let mut stack: Vec<(&Element, usize, bool)> = vec![(e, indent, false)];
    while let Some((e, indent, closing)) = stack.pop() {
        if closing {
            push_indent(out, indent);
            out.push_str("</");
            out.push_str(&e.name);
            out.push_str(">\n");
            continue;
        }
        let attrs = attributes_of(e, indent == top_indent, lookup, options, used);
        let children: Vec<&Element> = e.child_elements().collect();
        write_open(out, indent, &e.name, &attrs, !children.is_empty());
        if !children.is_empty() {
            stack.push((e, indent, true));
            for c in children.iter().rev() {
                stack.push((c, indent + 2, false));
            }
        }
    }
}

/// The attributes node `e` is written with (rule 6). `classify` has
/// already refused everything this does not cover.
fn attributes_of(
    e: &Element,
    tree: bool,
    lookup: &ModelLookup<'_>,
    options: &ReferenceOptions,
    used: &BTreeMap<String, NodeModel>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if tree || is_subtree_node(&e.name, options.mode) {
        if let Some(id) = e.attribute("ID") {
            out.push(("ID".to_owned(), id.to_owned()));
        }
        return out;
    }
    if let Some(n) = e.attribute("name") {
        out.push(("name".to_owned(), n.to_owned()));
    }
    let declared = used
        .get(&e.name)
        .or_else(|| lookup.get(e.name.as_str()).copied());
    match declared {
        Some(m) => {
            for p in &m.ports {
                match e.attribute(&p.name) {
                    Some(v) => out.push((p.name.clone(), v.to_owned())),
                    None if options.write_unset_ports => out.push((p.name.clone(), String::new())),
                    None => {}
                }
            }
        }
        None => {
            for p in builtin_ports(&e.name) {
                if let Some(v) = e.attribute(p) {
                    out.push(((*p).to_owned(), v.to_owned()));
                }
            }
        }
    }
    out
}

/// Write one model entry at `indent` (rule 9).
fn write_model(out: &mut String, indent: usize, m: &NodeModel) {
    let kind = match m.kind {
        ModelKind::Action => "Action",
        ModelKind::Condition => "Condition",
        ModelKind::Control => "Control",
        ModelKind::Decorator => "Decorator",
        ModelKind::SubTree => "SubTree",
    };
    let mut attrs: Vec<(&str, String)> = vec![("ID", m.id.clone())];
    if m.editable {
        attrs.push(("editable", "true".to_owned()));
    }
    write_open(out, indent, kind, &attrs, !m.ports.is_empty());
    if m.ports.is_empty() {
        return;
    }
    for p in &m.ports {
        let tag = match p.direction {
            PortDirection::Input => "input_port",
            PortDirection::Output => "output_port",
            PortDirection::InOut => "inout_port",
        };
        let mut pa: Vec<(&str, String)> = vec![("name", p.name.clone())];
        if let Some(t) = &p.type_name {
            pa.push(("type", t.clone()));
        }
        write_open(out, indent + 2, tag, &pa, false);
    }
    push_indent(out, indent);
    out.push_str("</");
    out.push_str(kind);
    out.push_str(">\n");
}

/// Write a start tag (self-closing when `open` is false) at `indent`, one
/// attribute per line when there are two or more (rule 7).
fn write_open<S: AsRef<str>>(
    out: &mut String,
    indent: usize,
    name: &str,
    attrs: &[(S, String)],
    open: bool,
) {
    push_indent(out, indent);
    out.push('<');
    out.push_str(name);
    let column = indent + 1 + name.len() + 1;
    for (i, (k, v)) in attrs.iter().enumerate() {
        if i == 0 {
            out.push(' ');
        } else {
            out.push('\n');
            push_indent(out, column);
        }
        out.push_str(k.as_ref());
        out.push_str("=\"");
        escape_into(out, v);
        out.push('"');
    }
    out.push_str(if open { ">\n" } else { "/>\n" });
}

fn push_indent(out: &mut String, n: usize) {
    out.extend(std::iter::repeat_n(' ', n));
}

/// Attribute value escaping (rule 5).
fn escape_into(out: &mut String, v: &str) {
    for c in v.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
}
