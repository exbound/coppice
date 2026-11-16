//! The parser: source bytes to a span-annotated [`Document`].
//!
//! A small hand-written tokenizer. Its job is to find where each construct
//! starts and ends, so every span is exact by construction, and to reject
//! input that is not well-formed XML 1.0 (fifth edition) with the byte
//! offset of the problem. Nothing is normalized: text and markup are
//! recorded as byte ranges into the untouched source.
//!
//! Stated limits (reported as [`ParseError::Unsupported`], never as
//! malformed): coppice does not read DTDs, so a DOCTYPE with an external
//! identifier or an internal subset is refused (a bare `<!DOCTYPE name>` is
//! accepted), any declared encoding other than UTF-8 is refused, and so is
//! element nesting deeper than [`MAX_DEPTH`]. Behavior-tree files use none of
//! these.

use std::fmt;

use crate::{Attribute, Document, Element, Node};

/// Why a source could not be parsed. Every variant names the byte offset
/// in the source where the problem was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The source is not valid UTF-8.
    NotUtf8 {
        /// Byte offset of the first invalid sequence.
        offset: usize,
    },
    /// The source is not well-formed XML.
    Malformed {
        /// Byte offset where the problem was detected.
        offset: usize,
        /// What the problem is, in words.
        message: String,
    },
    /// Well-formed XML that uses a feature coppice does not support.
    Unsupported {
        /// Byte offset of the unsupported construct.
        offset: usize,
        /// What is unsupported, in words.
        message: String,
    },
    /// An end tag does not match the open element.
    MismatchedEndTag {
        /// Byte offset of the end tag.
        offset: usize,
        /// The open element's name.
        expected: String,
        /// The end tag's name.
        found: String,
    },
    /// The source ended with elements still open.
    UnclosedElement {
        /// The source length.
        offset: usize,
        /// The innermost open element's name.
        open: String,
    },
    /// No top-level element, or more than one.
    TopLevelElementCount {
        /// Byte offset of the second top-level element, or the source
        /// length when there is none.
        offset: usize,
        /// 0 when the source has no top-level element; 2 when a second one
        /// starts (parsing stops there, so later ones are not counted).
        count: usize,
    },
}

impl ParseError {
    /// The byte offset the error names.
    pub fn offset(&self) -> usize {
        match self {
            ParseError::NotUtf8 { offset }
            | ParseError::Malformed { offset, .. }
            | ParseError::Unsupported { offset, .. }
            | ParseError::MismatchedEndTag { offset, .. }
            | ParseError::UnclosedElement { offset, .. }
            | ParseError::TopLevelElementCount { offset, .. } => *offset,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NotUtf8 { offset } => write!(f, "not valid UTF-8 at byte {offset}"),
            ParseError::Malformed { offset, message } => {
                write!(f, "malformed XML at byte {offset}: {message}")
            }
            ParseError::Unsupported { offset, message } => {
                write!(f, "unsupported XML at byte {offset}: {message}")
            }
            ParseError::MismatchedEndTag {
                offset,
                expected,
                found,
            } => write!(
                f,
                "end tag </{found}> at byte {offset} does not close <{expected}>"
            ),
            ParseError::UnclosedElement { offset, open } => {
                write!(f, "source ends at byte {offset} with <{open}> still open")
            }
            ParseError::TopLevelElementCount { offset, count } => write!(
                f,
                "expected exactly one top-level element, found {count} (byte {offset})"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// The deepest element nesting accepted. BehaviorTree.CPP itself refuses
/// trees nested deeper than 256 (BT 4.9.0 `src/xml_parsing.cpp:541-548`);
/// the cap here is four times that, and keeps every recursive walk over the
/// model well inside the stack.
pub const MAX_DEPTH: usize = 1024;

fn malformed(offset: usize, message: impl Into<String>) -> ParseError {
    ParseError::Malformed {
        offset,
        message: message.into(),
    }
}

fn unsupported(offset: usize, message: impl Into<String>) -> ParseError {
    ParseError::Unsupported {
        offset,
        message: message.into(),
    }
}

/// XML whitespace: space, tab, carriage return, line feed. Nothing else.
fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

/// The `Char` production of XML 1.0.
fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

/// The `NameStartChar` production of XML 1.0 (fifth edition).
fn is_name_start(c: char) -> bool {
    matches!(c,
        ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

/// The `NameChar` production of XML 1.0 (fifth edition).
fn is_name_char(c: char) -> bool {
    is_name_start(c)
        || matches!(c, '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Document, ParseError> {
    let source = std::str::from_utf8(bytes)
        .map_err(|e| ParseError::NotUtf8 {
            offset: e.valid_up_to(),
        })?
        .to_owned();
    if let Some((i, c)) = source.char_indices().find(|&(_, c)| !is_xml_char(c)) {
        return Err(malformed(
            i,
            format!("character U+{:04X} is not allowed in XML", c as u32),
        ));
    }
    let nodes = Parser::new(&source).run()?;
    Ok(Document { source, nodes })
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    /// Elements whose start tag has been read but whose end tag has not.
    open: Vec<Element>,
    top: Vec<Node>,
    top_elements: usize,
    seen_doctype: bool,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            src,
            pos: 0,
            open: Vec::new(),
            top: Vec::new(),
            top_elements: 0,
            seen_doctype: false,
        }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn run(mut self) -> Result<Vec<Node>, ParseError> {
        if self.src.starts_with('\u{feff}') {
            self.top.push(Node::ByteOrderMark(0..3));
            self.pos = 3;
        }
        while self.pos < self.src.len() {
            let start = self.pos;
            let rest = self.rest();
            if !rest.starts_with('<') {
                self.text(start)?;
            } else if rest.starts_with("<!--") {
                let end = self.comment_end(start)?;
                self.emit(Node::Comment(start..end));
            } else if rest.starts_with("<![CDATA[") {
                if self.open.is_empty() {
                    return Err(malformed(start, "CDATA outside the top-level element"));
                }
                let end = self.find_from(start, start + 9, "]]>", "CDATA section")? + 3;
                self.emit(Node::CData(start..end));
            } else if rest.starts_with("<!DOCTYPE") {
                let end = self.doctype(start)?;
                self.emit(Node::DocType(start..end));
            } else if rest.starts_with("<!") {
                return Err(malformed(start, "unknown markup declaration"));
            } else if rest.starts_with("<?") {
                self.processing_instruction(start)?;
            } else if rest.starts_with("</") {
                self.end_tag(start)?;
            } else {
                self.start_tag(start)?;
            }
        }
        if let Some(e) = self.open.last() {
            return Err(ParseError::UnclosedElement {
                offset: self.src.len(),
                open: e.name.clone(),
            });
        }
        if self.top_elements == 0 {
            return Err(ParseError::TopLevelElementCount {
                offset: self.src.len(),
                count: 0,
            });
        }
        Ok(self.top)
    }

    /// Add a finished node to the innermost open element, or the top level.
    fn emit(&mut self, node: Node) {
        self.pos = node.span().end;
        match self.open.last_mut() {
            Some(parent) => parent.children.push(node),
            None => self.top.push(node),
        }
    }

    /// Byte offset of `needle` searching from `from`; if absent, an
    /// "unterminated" error reported at `construct_start`.
    fn find_from(
        &self,
        construct_start: usize,
        from: usize,
        needle: &str,
        what: &str,
    ) -> Result<usize, ParseError> {
        self.src[from..]
            .find(needle)
            .map(|i| from + i)
            .ok_or_else(|| malformed(construct_start, format!("unterminated {what}")))
    }

    fn comment_end(&self, start: usize) -> Result<usize, ParseError> {
        let close = self.find_from(start, start + 4, "-->", "comment")?;
        let body = &self.src[start + 4..close];
        if let Some(i) = body.find("--") {
            return Err(malformed(
                start + 4 + i,
                "'--' is not allowed inside a comment",
            ));
        }
        if body.ends_with('-') {
            return Err(malformed(close - 1, "a comment cannot end with '--->'"));
        }
        Ok(close + 3)
    }

    fn text(&mut self, start: usize) -> Result<(), ParseError> {
        let end = self.rest().find('<').map_or(self.src.len(), |i| start + i);
        let raw = &self.src[start..end];
        if self.open.is_empty() {
            if let Some(i) = raw.find(|c: char| !is_xml_space(c)) {
                return Err(malformed(start + i, "text outside the top-level element"));
            }
        } else {
            if let Some(i) = raw.find("]]>") {
                return Err(malformed(start + i, "']]>' is not allowed in text"));
            }
            decode(raw, start, false)?;
        }
        self.emit(Node::Text(start..end));
        Ok(())
    }

    /// A `<!DOCTYPE …>` at `start`; returns the offset just past its `>`.
    fn doctype(&mut self, start: usize) -> Result<usize, ParseError> {
        if !self.open.is_empty() || self.top_elements > 0 {
            return Err(malformed(
                start,
                "DOCTYPE after the top-level element starts",
            ));
        }
        if self.seen_doctype {
            return Err(malformed(start, "a second DOCTYPE"));
        }
        self.seen_doctype = true;
        let mut at = start + "<!DOCTYPE".len();
        let after_ws = self.skip_whitespace(at);
        if after_ws == at {
            return Err(malformed(at, "expected whitespace after <!DOCTYPE"));
        }
        let (_, name_end) = self.name_at(after_ws)?;
        at = self.skip_whitespace(name_end);
        let rest = &self.src[at..];
        if rest.starts_with('>') {
            Ok(at + 1)
        } else if at > name_end
            && (rest.starts_with("SYSTEM") || rest.starts_with("PUBLIC"))
            && rest[6..].starts_with(|c: char| is_xml_space(c) || c == '"' || c == '\'')
        {
            Err(unsupported(at, "external DTDs are not read"))
        } else if rest.starts_with('[') {
            Err(unsupported(at, "DTD internal subsets are not read"))
        } else if rest.is_empty() {
            Err(malformed(start, "unterminated DOCTYPE"))
        } else {
            Err(malformed(at, "unexpected content in DOCTYPE"))
        }
    }

    /// A quoted literal starting at `q`; returns the offset past its close.
    fn quoted_end(&self, q: usize) -> Result<usize, ParseError> {
        let quote = match self.src[q..].chars().next() {
            Some(c @ ('"' | '\'')) => c,
            _ => return Err(malformed(q, "expected a quoted literal")),
        };
        self.src[q + 1..]
            .find(quote)
            .map(|i| q + 1 + i + 1)
            .ok_or_else(|| malformed(q, "unterminated quoted literal"))
    }

    fn processing_instruction(&mut self, start: usize) -> Result<(), ParseError> {
        let close = self.find_from(start, start + 2, "?>", "processing instruction")?;
        let end = close + 2;
        let (target, target_end) = self.name_at(start + 2)?;
        if target_end != close && !self.src[target_end..].starts_with(is_xml_space) {
            return Err(malformed(target_end, "invalid character after the target"));
        }
        if target == "xml" {
            let first = self.top.is_empty()
                || (self.top.len() == 1 && matches!(self.top[0], Node::ByteOrderMark(_)));
            if !(first && self.open.is_empty()) {
                return Err(malformed(
                    start,
                    "the XML declaration must come first in the document",
                ));
            }
            self.declaration(target_end, close)?;
            self.emit(Node::Declaration(start..end));
        } else if target.eq_ignore_ascii_case("xml") {
            return Err(malformed(
                start,
                "processing instruction target is reserved",
            ));
        } else {
            self.emit(Node::ProcessingInstruction(start..end));
        }
        Ok(())
    }

    /// Check `version`, `encoding` and `standalone` in the declaration body
    /// between `at` and `close` (the offset of `?>`).
    fn declaration(&self, mut at: usize, close: usize) -> Result<(), ParseError> {
        let mut seen: Vec<&str> = Vec::new();
        loop {
            let ws = self.skip_whitespace(at);
            if ws >= close {
                break;
            }
            if ws == at {
                return Err(malformed(at, "expected whitespace in the XML declaration"));
            }
            let (name, name_end) = self.name_at(ws)?;
            let eq = self.skip_whitespace(name_end);
            if !self.src[eq..].starts_with('=') {
                return Err(malformed(eq, "expected '=' in the XML declaration"));
            }
            let q = self.skip_whitespace(eq + 1);
            let value_end = self.quoted_end(q)?;
            let value = &self.src[q + 1..value_end - 1];
            let order = ["version", "encoding", "standalone"];
            let ok_order = match order.iter().position(|&n| n == name) {
                None => return Err(malformed(ws, format!("unknown declaration field {name:?}"))),
                Some(p) => seen
                    .iter()
                    .all(|s| order.iter().position(|n| n == s) < Some(p)),
            };
            if !ok_order || seen.contains(&name) {
                return Err(malformed(ws, format!("{name} is out of order or repeated")));
            }
            match name {
                "version" => {
                    let digits = value.strip_prefix("1.").unwrap_or("");
                    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                        return Err(malformed(q + 1, format!("invalid version {value:?}")));
                    }
                }
                "encoding" => {
                    if value.is_empty() {
                        return Err(malformed(q + 1, "empty encoding name"));
                    }
                    if !value.eq_ignore_ascii_case("UTF-8") {
                        return Err(unsupported(
                            q + 1,
                            format!("declared encoding {value:?}; only UTF-8 is supported"),
                        ));
                    }
                }
                _ => {
                    if value != "yes" && value != "no" {
                        return Err(malformed(q + 1, format!("invalid standalone {value:?}")));
                    }
                }
            }
            seen.push(name);
            at = value_end;
        }
        if seen.first() != Some(&"version") {
            return Err(malformed(at, "the XML declaration needs a version"));
        }
        Ok(())
    }

    /// The name starting at `at`, and where it ends. The character after
    /// the name is not checked here; callers check what may follow.
    fn name_at(&self, at: usize) -> Result<(&'a str, usize), ParseError> {
        let s = &self.src[at..];
        match s.chars().next() {
            Some(c) if is_name_start(c) => {}
            Some(c) => return Err(malformed(at, format!("a name cannot start with {c:?}"))),
            None => return Err(malformed(at, "expected a name")),
        }
        let len = s.find(|c: char| !is_name_char(c)).unwrap_or(s.len());
        Ok((&s[..len], at + len))
    }

    fn skip_whitespace(&self, at: usize) -> usize {
        let s = &self.src[at..];
        at + s.find(|c: char| !is_xml_space(c)).unwrap_or(s.len())
    }

    fn start_tag(&mut self, start: usize) -> Result<(), ParseError> {
        if self.open.len() >= MAX_DEPTH {
            return Err(unsupported(
                start,
                format!("elements nested deeper than {MAX_DEPTH}"),
            ));
        }
        let (name, mut at) = self.name_at(start + 1)?;
        let mut attributes: Vec<Attribute> = Vec::new();
        let self_closing = loop {
            let after_ws = self.skip_whitespace(at);
            let rest = &self.src[after_ws..];
            if rest.starts_with("/>") {
                at = after_ws + 2;
                break true;
            }
            if rest.starts_with('>') {
                at = after_ws + 1;
                break false;
            }
            if rest.is_empty() {
                return Err(malformed(start, format!("unterminated start tag <{name}")));
            }
            if rest.starts_with('/') {
                return Err(malformed(after_ws, "expected '/>' or '>'"));
            }
            if after_ws == at {
                return Err(malformed(at, "expected whitespace, '>' or '/>'"));
            }
            let attr = self.attribute(after_ws)?;
            if attributes.iter().any(|a| a.name == attr.name) {
                return Err(malformed(
                    after_ws,
                    format!("duplicate attribute {:?}", attr.name),
                ));
            }
            at = attr.value_span.end + 1;
            attributes.push(attr);
        };

        if self.open.is_empty() {
            if self.top_elements == 1 {
                return Err(ParseError::TopLevelElementCount {
                    offset: start,
                    count: 2,
                });
            }
            self.top_elements += 1;
        }
        let element = Element {
            name: name.to_owned(),
            span: start..at,
            start_tag: start..at,
            end_tag: None,
            attributes,
            children: Vec::new(),
        };
        if self_closing {
            self.emit(Node::Element(element));
        } else {
            self.pos = at;
            self.open.push(element);
        }
        Ok(())
    }

    fn attribute(&self, at: usize) -> Result<Attribute, ParseError> {
        let (name, name_end) = self.name_at(at)?;
        let eq = self.skip_whitespace(name_end);
        if !self.src[eq..].starts_with('=') {
            return Err(malformed(
                eq,
                format!("expected '=' after attribute {name:?}"),
            ));
        }
        let open_quote = self.skip_whitespace(eq + 1);
        let quote = match self.src[open_quote..].chars().next() {
            Some(q @ ('"' | '\'')) => q,
            _ => return Err(malformed(open_quote, "expected a quoted attribute value")),
        };
        let value_start = open_quote + 1;
        let value_end = self.src[value_start..]
            .find(quote)
            .map(|i| value_start + i)
            .ok_or_else(|| malformed(open_quote, "unterminated attribute value"))?;
        let raw = &self.src[value_start..value_end];
        if let Some(i) = raw.find('<') {
            return Err(malformed(
                value_start + i,
                "'<' is not allowed in an attribute value",
            ));
        }
        Ok(Attribute {
            name: name.to_owned(),
            value: decode(raw, value_start, true)?,
            name_span: at..name_end,
            value_span: value_start..value_end,
            quote,
        })
    }

    fn end_tag(&mut self, start: usize) -> Result<(), ParseError> {
        let (name, name_end) = self.name_at(start + 2)?;
        let close = self.skip_whitespace(name_end);
        if !self.src[close..].starts_with('>') {
            return Err(malformed(close, format!("expected '>' to end </{name}")));
        }
        let end = close + 1;
        let Some(mut element) = self.open.pop() else {
            return Err(malformed(
                start,
                format!("end tag </{name}> with no open element"),
            ));
        };
        if element.name != name {
            return Err(ParseError::MismatchedEndTag {
                offset: start,
                expected: element.name,
                found: name.to_owned(),
            });
        }
        element.end_tag = Some(start..end);
        element.span = element.start_tag.start..end;
        self.emit(Node::Element(element));
        Ok(())
    }
}

/// Decode entity and character references in `raw`, which starts at byte
/// `base` of the source. Undefined or malformed references are errors.
/// With `line_ends`, literal CR-LF and lone CR become LF (XML 1.0 §2.11);
/// a CR written as `&#13;` is kept.
pub(crate) fn decode(raw: &str, base: usize, line_ends: bool) -> Result<String, ParseError> {
    let literal = |out: &mut String, text: &str| {
        if line_ends {
            out.push_str(&text.replace("\r\n", "\n").replace('\r', "\n"));
        } else {
            out.push_str(text);
        }
    };
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut offset = base;
    while let Some(amp) = rest.find('&') {
        literal(&mut out, &rest[..amp]);
        let at = offset + amp;
        let after = &rest[amp + 1..];
        let semi = after
            .find(';')
            .ok_or_else(|| malformed(at, "'&' without a terminating ';'"))?;
        let entity = &after[..semi];
        let ch = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => char_reference(entity).ok_or_else(|| {
                malformed(at, format!("undefined or invalid reference &{entity};"))
            })?,
        };
        out.push(ch);
        let consumed = amp + 1 + semi + 1;
        rest = &rest[consumed..];
        offset += consumed;
    }
    literal(&mut out, rest);
    Ok(out)
}

/// `#123` or `#x7B` to its character, if it is a legal XML character;
/// `None` for anything else.
fn char_reference(entity: &str) -> Option<char> {
    let digits = entity.strip_prefix('#')?;
    let value = match digits.strip_prefix('x') {
        Some(hex) if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
            u32::from_str_radix(hex, 16).ok()?
        }
        Some(_) => return None,
        None if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) => {
            digits.parse().ok()?
        }
        None => return None,
    };
    char::from_u32(value).filter(|&c| is_xml_char(c))
}
