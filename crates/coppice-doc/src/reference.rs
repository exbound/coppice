//! The reference writer (F6, F7, F9): a tree file written byte for byte as
//! the reference application writes it (spec §3.2, §3.4, §3.5).
//!
//! The writer is a canonical serialization: nothing lexical survives from
//! the source. Where the record does not say what the reference writes,
//! the writer refuses rather than guesses (spec §3.7), so its output is
//! either the reference's bytes or nothing.

use std::collections::BTreeMap;

use crate::Element;
use crate::tree::{
    Diagnostic, DiagnosticKind, ModelKind, ModelLookup, NodeModel, Port, PortDirection,
    TreeDocument, builtin_ports, is_builtin, is_port_name, model_lookup,
};

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

    // Nodes: which models they use, and anything the writer cannot write.
    let mut used: BTreeMap<String, NodeModel> = BTreeMap::new();
    for t in &trees {
        for a in &t.attributes {
            if a.name != "ID" {
                refusals.push(uncovered(
                    a.name_span.start,
                    format!("the <BehaviorTree> attribute {}", a.name),
                ));
            }
        }
        if t.attribute("ID").is_none() {
            refusals.push(uncovered(
                t.span.start,
                "a <BehaviorTree> without an ID".to_owned(),
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
            classify(e, &lookup, options, &mut used, &mut refusals, &mut notes);
        }
    }

    // The declared models must themselves be writable (rule 9).
    for m in used.values() {
        check_model(m, &mut refusals);
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

    if !refusals.is_empty() {
        refusals.sort_by_key(|d| d.offset);
        refusals.dedup();
        return Err(refusals);
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
    notes.sort_by_key(|d| d.offset);
    Ok(Written { text: out, notes })
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
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
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
    if name == "SubTree" || name == "SubTreePlus" {
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
    let v3_builtin = match options.mode {
        WriteMode::V4 => is_builtin(name),
        WriteMode::V3 => V3_BUILTINS_SEEN.contains(&name),
    };
    if let Some(m) = lookup.get(name).filter(|_| !is_builtin(name)) {
        used.entry(m.id.clone()).or_insert_with(|| (*m).clone());
        return;
    }
    if is_builtin(name) {
        if lookup.contains_key(name) {
            refusals.push(uncovered(
                e.span.start,
                format!("a declared model {name} that is also a built-in (spec §7, O-17 i)"),
            ));
            return;
        }
        if v3_builtin {
            return;
        }
        if !(options.mode == WriteMode::V3 && V3_NOT_BUILTIN_SEEN.contains(&name)) {
            refusals.push(uncovered(
                e.span.start,
                format!("whether V3 mode treats {name} as built-in (spec §7, O-17 h)"),
            ));
            return;
        }
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
                })
                .collect(),
            editable: true,
            has_description: false,
            metadata: Vec::new(),
            offset: e.span.start,
        },
    );
}

/// Refuse a model entry the writer cannot write (rule 9).
fn check_model(m: &NodeModel, refusals: &mut Vec<Diagnostic>) {
    if !m.metadata.is_empty() {
        refusals.push(uncovered(m.offset, format!("model {}'s metadata", m.id)));
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
    // Iterative: an explicit stack of (element, indent, closing?).
    let mut stack: Vec<(&Element, usize, bool)> = vec![(e, indent, false)];
    while let Some((e, indent, closing)) = stack.pop() {
        if closing {
            push_indent(out, indent);
            out.push_str("</");
            out.push_str(&e.name);
            out.push_str(">\n");
            continue;
        }
        let attrs = attributes_of(e, lookup, options, used);
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
    lookup: &ModelLookup<'_>,
    options: &ReferenceOptions,
    used: &BTreeMap<String, NodeModel>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if e.name == "BehaviorTree" || e.name == "SubTree" || e.name == "SubTreePlus" {
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
