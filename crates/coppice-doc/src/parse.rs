//! The parser: source bytes to a span-annotated [`Document`].
//!
//! A small hand-written tokenizer. Its only job is to find where each
//! construct starts and ends, so every span is exact by construction, and
//! to reject input that is not well-formed. Nothing is normalized: text and
//! markup are recorded as byte ranges into the untouched source.

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
        /// How many top-level elements were found.
        count: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NotUtf8 { offset } => write!(f, "not valid UTF-8 at byte {offset}"),
            ParseError::Malformed { offset, message } => {
                write!(f, "malformed XML at byte {offset}: {message}")
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

fn malformed(offset: usize, message: impl Into<String>) -> ParseError {
    ParseError::Malformed {
        offset,
        message: message.into(),
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Document, ParseError> {
    let source = std::str::from_utf8(bytes)
        .map_err(|e| ParseError::NotUtf8 {
            offset: e.valid_up_to(),
        })?
        .to_owned();
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
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            src,
            pos: 0,
            open: Vec::new(),
            top: Vec::new(),
            top_elements: 0,
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
                let end = self.find_from(start + 4, "-->", "comment")? + 3;
                self.emit(Node::Comment(start..end));
            } else if rest.starts_with("<![CDATA[") {
                if self.open.is_empty() {
                    return Err(malformed(start, "CDATA outside the top-level element"));
                }
                let end = self.find_from(start + 9, "]]>", "CDATA section")? + 3;
                self.emit(Node::CData(start..end));
            } else if rest.starts_with("<!DOCTYPE") {
                let end = self.doctype_end(start)?;
                self.emit(Node::DocType(start..end));
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

    /// Byte offset of `needle`, searching from `from`; an error if absent.
    fn find_from(&self, from: usize, needle: &str, what: &str) -> Result<usize, ParseError> {
        self.src[from..]
            .find(needle)
            .map(|i| from + i)
            .ok_or_else(|| malformed(self.pos, format!("unterminated {what}")))
    }

    fn text(&mut self, start: usize) -> Result<(), ParseError> {
        let end = self.rest().find('<').map_or(self.src.len(), |i| start + i);
        let raw = &self.src[start..end];
        if self.open.is_empty() {
            if let Some(i) = raw.find(|c: char| !matches!(c, ' ' | '\t' | '\r' | '\n')) {
                return Err(malformed(start + i, "text outside the top-level element"));
            }
        } else {
            if let Some(i) = raw.find("]]>") {
                return Err(malformed(start + i, "']]>' is not allowed in text"));
            }
            decode(raw, start)?;
        }
        self.emit(Node::Text(start..end));
        Ok(())
    }

    fn doctype_end(&self, start: usize) -> Result<usize, ParseError> {
        // An internal subset `[...]` may contain '>' characters.
        let body = &self.src[start..];
        let gt = body.find('>');
        let bracket = body.find('[');
        let end = match (bracket, gt) {
            (Some(b), Some(g)) if b < g => {
                let close = self.find_from(start + b, "]", "DOCTYPE internal subset")?;
                self.find_from(close, ">", "DOCTYPE")?
            }
            (_, Some(g)) => start + g,
            (_, None) => return Err(malformed(start, "unterminated DOCTYPE")),
        };
        Ok(end + 1)
    }

    fn processing_instruction(&mut self, start: usize) -> Result<(), ParseError> {
        let end = self.find_from(start + 2, "?>", "processing instruction")? + 2;
        let body = &self.src[start + 2..end - 2];
        let target_len = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
        let target = &body[..target_len];
        if target.is_empty() {
            return Err(malformed(start, "processing instruction without a target"));
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

    /// The name starting at `at`, and where it ends.
    fn name_at(&self, at: usize) -> Result<(&'a str, usize), ParseError> {
        let s = &self.src[at..];
        let len = s
            .find(|c: char| c.is_whitespace() || "/>=<\"'&?!".contains(c))
            .unwrap_or(s.len());
        let name = &s[..len];
        match name.chars().next() {
            None => Err(malformed(at, "expected a name")),
            Some(c) if c.is_ascii_digit() || c == '-' || c == '.' => {
                Err(malformed(at, format!("a name cannot start with {c:?}")))
            }
            Some(_) => Ok((name, at + len)),
        }
    }

    fn skip_whitespace(&self, mut at: usize) -> usize {
        while let Some(c) = self.src[at..].chars().next() {
            if !c.is_whitespace() {
                break;
            }
            at += c.len_utf8();
        }
        at
    }

    fn start_tag(&mut self, start: usize) -> Result<(), ParseError> {
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
            if after_ws == at {
                return Err(malformed(at, "expected whitespace before an attribute"));
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
            value: decode(raw, value_start)?,
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
fn decode(raw: &str, base: usize) -> Result<String, ParseError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut offset = base;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
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
    out.push_str(rest);
    Ok(out)
}

/// `#123` or `#x7B` to its character; `None` for anything else.
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
    char::from_u32(value).filter(|&c| c != '\0')
}
