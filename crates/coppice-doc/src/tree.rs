//! The behavior-tree view of a parsed [`Document`]: root attributes,
//! includes, trees, node models, the dialect suggestion and diagnostics.
//!
//! Implements the rest of F1 and F2, F4 and F10 of
//! `docs/engineering-spec.md` (§2.1, §2.2, §2.4, §2.5, §3.6). The view reads
//! the span model and never changes it, so the lossless round-trip (F5) is
//! unaffected by anything here.

use crate::parse::decode;
use crate::{Document, Element, Node};

/// The XML dialect a document is written in (spec §2.2, §2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// BehaviorTree.CPP 3.x.
    V3,
    /// BehaviorTree.CPP 4.x.
    V4,
}

/// The node type a model declares (spec §2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    /// `<Action>`.
    Action,
    /// `<Condition>`.
    Condition,
    /// `<Control>`.
    Control,
    /// `<Decorator>`.
    Decorator,
    /// `<SubTree>`.
    SubTree,
}

impl ModelKind {
    /// Whether nodes of this kind are leaves, which may not have children
    /// (BT 4.9.0 `src/xml_parsing.cpp:671-677`).
    pub fn is_leaf(self) -> bool {
        matches!(self, ModelKind::Action | ModelKind::Condition)
    }
}

/// A port's direction (spec §2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    /// `<input_port>`.
    Input,
    /// `<output_port>`.
    Output,
    /// `<inout_port>`.
    InOut,
}

/// One port of a node model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    /// The direction, from the element name.
    pub direction: PortDirection,
    /// The `name` attribute.
    pub name: String,
    /// The `type` attribute, absent for an untyped port.
    pub type_name: Option<String>,
    /// The `default` attribute, absent when empty or missing.
    pub default: Option<String>,
    /// The element's text content with references decoded: the port's
    /// description. Empty when there is none.
    pub description: String,
}

/// One entry of a `<TreeNodesModel>` block (spec §2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeModel {
    /// The node type, from the element name.
    pub kind: ModelKind,
    /// The `ID` attribute: the registration name nodes use as their tag.
    pub id: String,
    /// Ports in source order.
    pub ports: Vec<Port>,
    /// Whether the entry carries `editable="true"`.
    pub editable: bool,
    /// Whether the entry has a `<description>` child element.
    pub has_description: bool,
    /// The `<MetadataFields>` key/value pairs, in source order.
    pub metadata: Vec<(String, String)>,
}

/// One `<include>` of a document (spec §2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Include {
    /// The `path` attribute as written (decoded).
    pub path: String,
    /// The `ros_pkg` attribute, if present.
    pub ros_pkg: Option<String>,
    /// Byte offset of the `<include>` element in the source.
    pub offset: usize,
}

/// What a diagnostic is about. Diagnostics report a condition without
/// refusing the input (design decision D7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// `<root>` has no `BTCPP_format` attribute (spec §2.1).
    MissingFormat,
    /// A leaf node (an Action or Condition, built-in or declared) has a
    /// child element (spec §3.6).
    LeafWithChildren,
    /// A node of a declared model carries an attribute that is neither a
    /// declared port nor a reserved attribute (spec §2.5).
    UnknownPort,
    /// A `<TreeNodesModel>` child whose element name is not a model kind.
    UnknownModelKind,
    /// A model entry without an `ID`, or a port without a `name`.
    IncompleteModel,
    /// More than one `<TreeNodesModel>` block; their entries are merged.
    DuplicateModelsBlock,
    /// A model entry whose `ID` was already declared; the first one is used.
    DuplicateModel,
    /// A model entry names a built-in node of BehaviorTree.CPP but declares
    /// it a leaf when the built-in is not, or the reverse. The library uses
    /// its built-in. A declaration that agrees (as the library itself writes
    /// into monitor and log XML) is not reported.
    ModelShadowsBuiltin,
    /// An `<Action|Condition|Control|Decorator>` node without an `ID`, or a
    /// node written with its own tag that carries an `ID` attribute; the
    /// library refuses both (BT 4.9.0 `src/xml_parsing.cpp:560-563,763-766`).
    MisplacedNodeId,
}

/// A condition worth reporting, at a byte offset in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What it is about.
    pub kind: DiagnosticKind,
    /// Byte offset of the element or attribute concerned.
    pub offset: usize,
    /// A human-readable explanation naming the element or attribute.
    pub message: String,
}

/// The top-level element is not `<root>`, so this is not a behavior-tree
/// document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotATreeDocument {
    /// Byte offset of the top-level element.
    pub offset: usize,
    /// Its name.
    pub found: String,
}

impl std::fmt::Display for NotATreeDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "top-level element <{}> at byte {} is not <root>",
            self.found, self.offset
        )
    }
}

impl std::error::Error for NotATreeDocument {}

/// The built-in leaf nodes BehaviorTree.CPP 4.9.0 registers: its Actions
/// and Conditions (BT 4.9.0 `src/bt_factory.cpp:148-154,171`).
pub const BUILTIN_LEAVES: &[&str] = &[
    "AlwaysSuccess",
    "AlwaysFailure",
    "Script",
    "ScriptCondition",
    "SetBlackboard",
    "Sleep",
    "UnsetBlackboard",
    "WasEntryUpdated",
];

/// The other node types BehaviorTree.CPP 4.9.0 registers by default: its
/// Controls and Decorators, which may have children (BT 4.9.0
/// `src/bt_factory.cpp:118-173`). `SequenceStar` is left out: the library
/// registers it only when built with its v3 old-names option, which is off
/// by default (`src/bt_factory.cpp:123-126`, `CMakeLists.txt`).
pub const BUILTIN_BRANCHES: &[&str] = &[
    "Fallback",
    "AsyncFallback",
    "Sequence",
    "AsyncSequence",
    "SequenceWithMemory",
    "Parallel",
    "ParallelAll",
    "ReactiveSequence",
    "ReactiveFallback",
    "IfThenElse",
    "WhileDoElse",
    "TryCatch",
    "Inverter",
    "RetryUntilSuccessful",
    "KeepRunningUntilFailure",
    "Repeat",
    "Timeout",
    "Delay",
    "RunOnce",
    "ForceSuccess",
    "ForceFailure",
    "SubTree",
    "Precondition",
    "Switch2",
    "Switch3",
    "Switch4",
    "Switch5",
    "Switch6",
    "LoopInt",
    "LoopBool",
    "LoopDouble",
    "LoopString",
    "SkipUnlessUpdated",
    "WaitValueUpdate",
];

/// The attribute names BehaviorTree.CPP reserves: never ports (BT 4.9.0
/// `include/behaviortree_cpp/tree_node.h:78-94`,
/// `src/basic_types.cpp:505-522`).
pub const RESERVED_ATTRIBUTES: &[&str] = &[
    "name",
    "ID",
    "_autoremap",
    "_failureIf",
    "_successIf",
    "_skipIf",
    "_while",
    "_onHalted",
    "_onFailure",
    "_onSuccess",
    "_post",
];

/// Whether BehaviorTree.CPP would treat an attribute name as a port name:
/// it starts with an ASCII letter, contains none of the library's forbidden
/// characters, and is not reserved (BT 4.9.0 `src/basic_types.cpp:485-530`).
/// Any other attribute (`_uid`, `_fullpath`, `__shared_blackboard`, `a.b`, …)
/// is kept by the library without a port check, so coppice does not check it
/// either.
pub fn is_port_name(name: &str) -> bool {
    const FORBIDDEN: &[char] = &[
        ' ', '\t', '\n', '\r', '<', '>', '&', '"', '\'', '/', '\\', ':', '*', '?', '|', '.',
    ];
    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && !name.contains(FORBIDDEN)
        && !RESERVED_ATTRIBUTES.contains(&name)
}

/// The element names under which BehaviorTree.CPP reads the node type from
/// the `ID` attribute instead of the element name (BT 4.9.0
/// `src/xml_parsing.cpp:557-559,605`).
const EXPLICIT_FORMS: &[&str] = &["Action", "Condition", "Control", "Decorator"];

/// A behavior-tree document: a [`Document`] whose top-level element is
/// `<root>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeDocument {
    doc: Document,
}

impl TreeDocument {
    /// View a parsed document as a behavior-tree document.
    pub fn new(doc: Document) -> Result<TreeDocument, NotATreeDocument> {
        let root = doc.root_element();
        if root.name != "root" {
            return Err(NotATreeDocument {
                offset: root.span.start,
                found: root.name.clone(),
            });
        }
        Ok(TreeDocument { doc })
    }

    /// The underlying lossless document.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    fn root(&self) -> &Element {
        self.doc.root_element()
    }

    /// The `BTCPP_format` attribute of `<root>`, if present.
    pub fn format(&self) -> Option<&str> {
        self.root().attribute("BTCPP_format")
    }

    /// The `BTCPP_version` attribute of `<root>`, if present.
    pub fn version(&self) -> Option<&str> {
        self.root().attribute("BTCPP_version")
    }

    /// The `project_name` attribute of `<root>`, if present.
    pub fn project_name(&self) -> Option<&str> {
        self.root().attribute("project_name")
    }

    /// The `<include>` children of `<root>`, in source order.
    pub fn includes(&self) -> Vec<Include> {
        let mut out = Vec::new();
        for e in self.root().child_elements() {
            if e.name == "include" {
                out.push(Include {
                    path: e.attribute("path").unwrap_or("").to_owned(),
                    ros_pkg: e.attribute("ros_pkg").map(str::to_owned),
                    offset: e.span.start,
                });
            }
        }
        out
    }

    /// The `<BehaviorTree>` children of `<root>`, in source order.
    pub fn behavior_trees(&self) -> Vec<&Element> {
        self.root()
            .child_elements()
            .filter(|e| e.name == "BehaviorTree")
            .collect()
    }

    /// The entries of every `<TreeNodesModel>` child of `<root>`, merged in
    /// source order. Entries that are not a model kind, or have no `ID`,
    /// are skipped (and reported by [`TreeDocument::diagnostics`]).
    pub fn models(&self) -> Vec<NodeModel> {
        let doc = &self.doc;
        let root = self.root();
        let mut out = Vec::new();
        for block in root.child_elements().filter(|e| e.name == "TreeNodesModel") {
            for entry in block.child_elements() {
                let Some(kind) = model_kind(&entry.name) else {
                    continue;
                };
                let Some(id) = entry.attribute("ID").filter(|s| !s.is_empty()) else {
                    continue;
                };
                let mut ports = Vec::new();
                for port in entry.child_elements() {
                    let Some(direction) = (match port.name.as_str() {
                        "input_port" => Some(PortDirection::Input),
                        "output_port" => Some(PortDirection::Output),
                        "inout_port" => Some(PortDirection::InOut),
                        _ => None,
                    }) else {
                        continue;
                    };
                    let Some(name) = port.attribute("name").filter(|s| !s.is_empty()) else {
                        continue;
                    };
                    ports.push(Port {
                        direction,
                        name: name.to_owned(),
                        type_name: port.attribute("type").map(str::to_owned),
                        default: port
                            .attribute("default")
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned),
                        description: port_description(doc, port),
                    });
                }
                out.push(NodeModel {
                    kind,
                    id: id.to_owned(),
                    ports,
                    editable: entry.attribute("editable") == Some("true"),
                    has_description: entry.child_elements().any(|c| c.name == "description"),
                    metadata: metadata_pairs(entry),
                });
            }
        }
        out
    }

    /// The dialect this document suggests (spec §2.5): `BTCPP_format`
    /// decides when present ("3" for v3, anything else v4); without it, a
    /// `SubTreePlus` element anywhere suggests v3; otherwise v4.
    pub fn suggested_dialect(&self) -> Dialect {
        match self.format() {
            Some(format) if format != "3" => Dialect::V4,
            Some(_) => Dialect::V3,
            None => {
                if self.contains_element("SubTreePlus") {
                    Dialect::V3
                } else {
                    Dialect::V4
                }
            }
        }
    }

    /// Conditions worth reporting, sorted by offset.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let root = self.root();
        let mut out = Vec::new();
        if self.format().is_none() {
            out.push(Diagnostic {
                kind: DiagnosticKind::MissingFormat,
                offset: root.span.start,
                message: "the <root> element has no BTCPP_format attribute".to_owned(),
            });
        }
        for (i, block) in root
            .child_elements()
            .filter(|e| e.name == "TreeNodesModel")
            .enumerate()
        {
            if i > 0 {
                out.push(Diagnostic {
                    kind: DiagnosticKind::DuplicateModelsBlock,
                    offset: block.span.start,
                    message: "a second <TreeNodesModel> block; entries of both blocks are merged"
                        .to_owned(),
                });
            }
            for entry in block.child_elements() {
                let unknown_kind = model_kind(&entry.name).is_none();
                let id = entry.attribute("ID").filter(|s| !s.is_empty());
                if unknown_kind {
                    out.push(Diagnostic {
                        kind: DiagnosticKind::UnknownModelKind,
                        offset: entry.span.start,
                        message: format!(
                            "<{}> is not a node model kind (Action, Condition, Control, Decorator or SubTree)",
                            entry.name
                        ),
                    });
                    continue;
                }
                if id.is_none() {
                    out.push(Diagnostic {
                        kind: DiagnosticKind::IncompleteModel,
                        offset: entry.span.start,
                        message: format!(
                            "the <{}> model entry has no ID, or an empty one",
                            entry.name
                        ),
                    });
                    continue;
                }
                for port in entry.child_elements() {
                    if matches!(
                        port.name.as_str(),
                        "input_port" | "output_port" | "inout_port"
                    ) && port.attribute("name").is_none_or(str::is_empty)
                    {
                        out.push(Diagnostic {
                            kind: DiagnosticKind::IncompleteModel,
                            offset: port.span.start,
                            message: format!(
                                "a <{}> of model {} has no name",
                                port.name,
                                id.unwrap_or_default()
                            ),
                        });
                    }
                }
            }
        }
        // Duplicate and shadowing declarations, from the parsed models.
        let models = self.models();
        let mut entries = root
            .child_elements()
            .filter(|e| e.name == "TreeNodesModel")
            .flat_map(|b| b.child_elements())
            .filter(|e| {
                model_kind(&e.name).is_some() && e.attribute("ID").is_some_and(|i| !i.is_empty())
            });
        for (n, m) in models.iter().enumerate() {
            let Some(entry) = entries.next() else { break };
            if models[..n].iter().any(|earlier| earlier.id == m.id) {
                out.push(Diagnostic {
                    kind: DiagnosticKind::DuplicateModel,
                    offset: entry.span.start,
                    message: format!(
                        "model {} is declared more than once; the first is used",
                        m.id
                    ),
                });
            }
            if is_builtin(&m.id) && m.kind.is_leaf() != BUILTIN_LEAVES.contains(&m.id.as_str()) {
                out.push(Diagnostic {
                    kind: DiagnosticKind::ModelShadowsBuiltin,
                    offset: entry.span.start,
                    message: format!(
                        "model {} declares the built-in node {} as {}; the library uses its built-in",
                        m.id,
                        if BUILTIN_LEAVES.contains(&m.id.as_str()) { "a leaf" } else { "not a leaf" },
                        if m.kind.is_leaf() { "a leaf" } else { "not a leaf" },
                    ),
                });
            }
        }
        // Node-level checks, inside every <BehaviorTree> child of <root>.
        for tree in self.behavior_trees() {
            walk(tree, &models, &mut out);
        }
        out.sort_by_key(|d| d.offset);
        out
    }

    fn contains_element(&self, name: &str) -> bool {
        let mut stack = vec![self.doc.root_element()];
        while let Some(e) = stack.pop() {
            if e.name == name {
                return true;
            }
            stack.extend(e.child_elements());
        }
        false
    }
}

/// The model kind an element name declares, by its element name.
fn model_kind(name: &str) -> Option<ModelKind> {
    match name {
        "Action" => Some(ModelKind::Action),
        "Condition" => Some(ModelKind::Condition),
        "Control" => Some(ModelKind::Control),
        "Decorator" => Some(ModelKind::Decorator),
        "SubTree" => Some(ModelKind::SubTree),
        _ => None,
    }
}

/// The port description: the element's text content with references
/// decoded and line ends normalized — its raw `Text` spans concatenated and
/// decoded, and any
/// `CData` span with its `<![CDATA[ … ]]>` delimiters stripped and kept
/// literally. Comments and child elements are not port text. The parser has
/// already validated every reference in the text, so decoding cannot fail.
fn port_description(doc: &Document, port: &Element) -> String {
    let mut out = String::new();
    for child in &port.children {
        let span = match child {
            Node::Text(s) => Some(s),
            Node::CData(s) => Some(s),
            _ => None,
        };
        let Some(span) = span else { continue };
        let raw = doc.text(span);
        if let Some(inner) = raw
            .strip_prefix("<![CDATA[")
            .and_then(|r| r.strip_suffix("]]>"))
        {
            // CDATA is literal: no references inside it.
            out.push_str(inner);
        } else if let Ok(decoded) = decode(raw, span.start, true) {
            out.push_str(&decoded);
        }
    }
    out
}

/// The `<MetadataFields>` key/value pairs, in source order.
fn metadata_pairs(entry: &Element) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for fields in entry
        .child_elements()
        .filter(|c| c.name == "MetadataFields")
    {
        for meta in fields.child_elements().filter(|c| c.name == "Metadata") {
            for a in &meta.attributes {
                out.push((a.name.clone(), a.value.clone()));
            }
        }
    }
    out
}

fn is_builtin(name: &str) -> bool {
    BUILTIN_LEAVES.contains(&name) || BUILTIN_BRANCHES.contains(&name)
}

/// Visit `tree` and every element below it, iteratively (no recursion, so
/// nesting depth cannot exhaust the stack), reporting `LeafWithChildren`,
/// `UnknownPort` and `MisplacedNodeId` as the library's loader would decide
/// them (spec §2.5, §3.6; BT 4.9.0 `src/xml_parsing.cpp:557-677,763-850`).
fn walk(tree: &Element, models: &[NodeModel], out: &mut Vec<Diagnostic>) {
    let mut stack: Vec<&Element> = tree.child_elements().collect();
    stack.reverse();
    while let Some(element) = stack.pop() {
        let mut children: Vec<&Element> = element.child_elements().collect();
        children.reverse();
        stack.extend(children);
        check_node(element, models, out);
    }
}

fn check_node(element: &Element, models: &[NodeModel], out: &mut Vec<Diagnostic>) {
    if element.name == "SubTree" || element.name == "SubTreePlus" {
        return;
    }
    let explicit = EXPLICIT_FORMS.contains(&element.name.as_str());
    let type_name = if explicit {
        match element.attribute("ID").filter(|i| !i.is_empty()) {
            Some(id) => id,
            None => {
                out.push(Diagnostic {
                    kind: DiagnosticKind::MisplacedNodeId,
                    offset: element.span.start,
                    message: format!("<{}> has no ID naming its node type", element.name),
                });
                return;
            }
        }
    } else {
        if let Some(a) = element.attributes.iter().find(|a| a.name == "ID") {
            out.push(Diagnostic {
                kind: DiagnosticKind::MisplacedNodeId,
                offset: a.name_span.start,
                message: format!("ID is not allowed on a <{}> node", element.name),
            });
        }
        element.name.as_str()
    };
    // The library decides the kind from its registered nodes; a declared
    // model only counts for a name the library does not register itself.
    let declared = if is_builtin(type_name) {
        None
    } else {
        models.iter().find(|m| m.id == type_name)
    };
    let leaf = BUILTIN_LEAVES.contains(&type_name) || declared.is_some_and(|m| m.kind.is_leaf());
    if leaf && element.child_elements().next().is_some() {
        out.push(Diagnostic {
            kind: DiagnosticKind::LeafWithChildren,
            offset: element.span.start,
            message: format!(
                "the leaf node {type_name} has a child element; leaf nodes may not have children"
            ),
        });
    }
    if let Some(m) = declared
        && m.kind != ModelKind::SubTree
    {
        for a in &element.attributes {
            if (explicit && a.name == "ID") || !is_port_name(&a.name) {
                continue;
            }
            if m.ports.iter().any(|p| p.name == a.name) {
                continue;
            }
            out.push(Diagnostic {
                kind: DiagnosticKind::UnknownPort,
                offset: a.name_span.start,
                message: format!(
                    "attribute '{}' is not a declared port of model {type_name}",
                    a.name
                ),
            });
        }
    }
}
