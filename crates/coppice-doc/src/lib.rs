//! Behavior-tree documents: projects and tree XML in both dialects.
//!
//! Implements F5 (lossless round-trip), the reading half of F1, F2 and F4,
//! project loading (F3, [`Project`]) and the F10 diagnostics of
//! `docs/engineering-spec.md`, using the design in `docs/design-decisions.md`
//! D3: a document keeps its original source, and every node of the parsed
//! tree records the byte range it came from. Output is rebuilt from those
//! ranges, so an unmodified document renders back to exactly its input, and
//! later edits (the save-time changes of F6–F9) splice at a known range
//! without disturbing anything else.

mod parse;
mod project;
mod reference;
mod tree;

use std::ops::Range;

pub use parse::{MAX_DEPTH, ParseError};
pub use project::{
    Disk, Files, LoadError, Project, ProjectDiagnostic, ProjectDocument, ProjectTree,
};
pub use reference::{ReferenceOptions, WriteMode, Written, write_tree};
pub use tree::{
    BUILTIN_BRANCHES, BUILTIN_LEAVES, BUILTIN_PORTS, Diagnostic, DiagnosticKind, Dialect, Include,
    ModelKind, NodeModel, NotATreeDocument, Port, PortDirection, RESERVED_ATTRIBUTES, TreeDocument,
    builtin_ports, is_port_name,
};

/// A byte range into [`Document::source`].
pub type Span = Range<usize>;

/// A parsed XML document that remembers exactly where everything came from.
///
/// Invariant (tested): the spans of [`Document::nodes`] are contiguous and
/// cover the whole source, from byte 0 to `source.len()`, with no gaps or
/// overlaps; and within every [`Element`], `start_tag`, the children's spans
/// in order, and `end_tag` are likewise contiguous and cover `span`. So
/// [`Document::render`] reproduces the source byte for byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    source: String,
    nodes: Vec<Node>,
}

/// One piece of XML, at any level of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// An element, with its tags, attributes and children.
    Element(Element),
    /// Character data between markup, exactly as written: whitespace,
    /// entity and character references are not decoded here.
    Text(Span),
    /// `<!-- … -->`, the whole comment including its delimiters.
    Comment(Span),
    /// `<![CDATA[ … ]]>`, including its delimiters.
    CData(Span),
    /// The XML declaration `<?xml … ?>`, including its delimiters.
    Declaration(Span),
    /// Any other processing instruction `<? … ?>`, including delimiters.
    ProcessingInstruction(Span),
    /// `<!DOCTYPE … >`, including its delimiters.
    DocType(Span),
    /// A UTF-8 byte-order mark at the very start of the source.
    ByteOrderMark(Span),
}

/// An element: `<name attr="v">children</name>` or `<name attr="v"/>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// The tag name as written, including any namespace prefix.
    pub name: String,
    /// The whole element, from `<` of the start tag to `>` of the end tag
    /// (or of the self-closing tag).
    pub span: Span,
    /// The start tag `<name …>`, or the whole self-closing tag `<name …/>`.
    pub start_tag: Span,
    /// The end tag `</name>`; `None` for a self-closing element.
    pub end_tag: Option<Span>,
    /// Attributes in source order.
    pub attributes: Vec<Attribute>,
    /// Child nodes in source order; always empty when self-closing.
    pub children: Vec<Node>,
}

/// One attribute of a start tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// The name as written.
    pub name: String,
    /// The value with entity and character references decoded
    /// (`&amp;` becomes `&`, `&#x41;` becomes `A`) and literal line ends
    /// normalized to LF (CR-LF and a lone CR become LF, XML 1.0 §2.11).
    /// Tabs and line feeds are not replaced by spaces as XML 1.0 §3.3.3
    /// describes, because the XML library BehaviorTree.CPP 4.9.0 bundles
    /// (tinyxml2 11.0, `3rdparty/tinyxml2/tinyxml2.cpp:294-316`) does not
    /// either. That library also collapses an LF-CR pair to one LF, which
    /// XML does not; coppice follows XML there.
    pub value: String,
    /// Where the name is, within the start tag.
    pub name_span: Span,
    /// Where the raw value is: the bytes between the quotes, not including
    /// them, exactly as written.
    pub value_span: Span,
    /// The quote character used: `"` or `'`.
    pub quote: char,
}

impl Document {
    /// Parse a document. The input must be well-formed XML 1.0 encoded as
    /// UTF-8 (a UTF-8 byte-order mark is allowed and kept). Exactly one
    /// top-level element is required. DTDs are not read: a DOCTYPE with an
    /// external identifier or an internal subset is refused as
    /// [`ParseError::Unsupported`] (a bare `<!DOCTYPE name>` is accepted),
    /// as are a declared encoding other than UTF-8 and element nesting
    /// deeper than [`MAX_DEPTH`].
    pub fn parse(source: &[u8]) -> Result<Document, ParseError> {
        parse::parse(source)
    }

    /// The original source, unchanged.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Top-level nodes in source order: the declaration, comments,
    /// whitespace, the one top-level element, and anything after it.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The single top-level element.
    pub fn root_element(&self) -> &Element {
        self.nodes
            .iter()
            .find_map(|n| match n {
                Node::Element(e) => Some(e),
                _ => None,
            })
            .expect("parse guarantees exactly one top-level element")
    }

    /// The text of a span of this document's source.
    pub fn text(&self, span: &Span) -> &str {
        &self.source[span.clone()]
    }

    /// Rebuild the document from the node tree's spans. For an unmodified
    /// document this equals [`Document::source`] byte for byte (feature F5).
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.source.len());
        for node in &self.nodes {
            self.render_node(node, &mut out);
        }
        out
    }

    fn render_node(&self, node: &Node, out: &mut String) {
        match node {
            Node::Element(e) => {
                out.push_str(self.text(&e.start_tag));
                for child in &e.children {
                    self.render_node(child, out);
                }
                if let Some(end) = &e.end_tag {
                    out.push_str(self.text(end));
                }
            }
            Node::Text(s)
            | Node::Comment(s)
            | Node::CData(s)
            | Node::Declaration(s)
            | Node::ProcessingInstruction(s)
            | Node::DocType(s)
            | Node::ByteOrderMark(s) => out.push_str(self.text(s)),
        }
    }
}

impl Node {
    /// The byte range this node covers in the source.
    pub fn span(&self) -> &Span {
        match self {
            Node::Element(e) => &e.span,
            Node::Text(s)
            | Node::Comment(s)
            | Node::CData(s)
            | Node::Declaration(s)
            | Node::ProcessingInstruction(s)
            | Node::DocType(s)
            | Node::ByteOrderMark(s) => s,
        }
    }
}

impl Element {
    /// The decoded value of the first attribute with this name.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Child elements, skipping text, comments and other nodes.
    pub fn child_elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// Whether this element was written self-closing (`<name/>`).
    pub fn is_self_closing(&self) -> bool {
        self.end_tag.is_none()
    }
}
