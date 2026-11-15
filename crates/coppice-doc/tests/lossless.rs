//! F1 (parse into the model) and F5 (lossless round-trip), against the
//! self-authored fixtures in `tests/fixtures/doc/` (provenance in
//! `tests/fixtures/README.md`).

use std::path::PathBuf;

use coppice_doc::{Document, Element, Node, ParseError};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/doc")
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(fixture_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn all_fixtures() -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<_> = std::fs::read_dir(fixture_dir())
        .expect("fixture dir")
        .map(|e| e.unwrap().path())
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read(&p).unwrap())
        })
        .collect();
    out.sort();
    assert!(out.len() >= 8, "fixture set went missing: {}", out.len());
    out
}

fn parse(name: &str) -> Document {
    Document::parse(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// First differing byte offset, for a readable failure.
fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i))
}

// ---- F5: the round-trip ----

#[test]
fn every_fixture_renders_back_byte_for_byte() {
    for (name, bytes) in all_fixtures() {
        let doc = Document::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = doc.render();
        assert_eq!(
            first_difference(out.as_bytes(), &bytes),
            None,
            "{name}: render differs from source"
        );
    }
}

#[test]
fn negative_control_one_whitespace_byte_survives() {
    let space = Document::parse(&fixture("pair-space.xml"))
        .unwrap()
        .render();
    let tab = Document::parse(&fixture("pair-tab.xml")).unwrap().render();
    let diffs: Vec<usize> = (0..space.len())
        .filter(|&i| space.as_bytes()[i] != tab.as_bytes()[i])
        .collect();
    assert_eq!(space.len(), tab.len());
    assert_eq!(diffs, vec![22]);
    assert_eq!((space.as_bytes()[22], tab.as_bytes()[22]), (b' ', b'\t'));
}

// ---- The span invariants that make the round-trip honest ----

fn check_contiguous(name: &str, what: &str, start: usize, nodes: &[Node], end: usize) {
    let mut at = start;
    for n in nodes {
        assert_eq!(
            n.span().start,
            at,
            "{name}: gap or overlap in {what} at {at}"
        );
        at = n.span().end;
    }
    assert_eq!(at, end, "{name}: {what} does not reach {end}");
}

fn check_element(name: &str, doc: &Document, e: &Element) {
    assert_eq!(
        e.start_tag.start, e.span.start,
        "{name}: <{}> start",
        e.name
    );
    let tag = doc.text(&e.start_tag);
    assert!(
        tag.starts_with(&format!("<{}", e.name)),
        "{name}: start tag {tag:?}"
    );
    assert!(tag.ends_with('>'), "{name}: start tag {tag:?}");
    match &e.end_tag {
        None => {
            assert!(tag.ends_with("/>"), "{name}: self-closing {tag:?}");
            assert_eq!(e.span, e.start_tag, "{name}: <{}/> span", e.name);
            assert!(e.children.is_empty());
        }
        Some(end) => {
            assert!(!tag.ends_with("/>"), "{name}: {tag:?} has an end tag");
            check_contiguous(name, &e.name, e.start_tag.end, &e.children, end.start);
            assert_eq!(end.end, e.span.end, "{name}: </{}> end", e.name);
            let end_text = doc.text(end);
            assert!(
                end_text.starts_with(&format!("</{}", e.name))
                    && end_text.ends_with('>')
                    && end_text[2 + e.name.len()..end_text.len() - 1]
                        .chars()
                        .all(char::is_whitespace),
                "{name}: end tag {end_text:?} for <{}>",
                e.name
            );
        }
    }
    let mut last = e.start_tag.start;
    for a in &e.attributes {
        assert!(
            a.name_span.start > last,
            "{name}: attribute order in <{}>",
            e.name
        );
        assert!(
            a.value_span.end < e.start_tag.end,
            "{name}: {} outside tag",
            a.name
        );
        assert_eq!(
            doc.text(&a.name_span),
            a.name,
            "{name}: attribute name span"
        );
        let src = doc.source().as_bytes();
        let q = a.quote as u8;
        assert!(
            a.quote == '"' || a.quote == '\'',
            "{name}: quote {:?}",
            a.quote
        );
        assert_eq!(
            src[a.value_span.start - 1],
            q,
            "{name}: {} opening quote",
            a.name
        );
        assert_eq!(src[a.value_span.end], q, "{name}: {} closing quote", a.name);
        last = a.value_span.end;
    }
    for child in &e.children {
        check_node(name, doc, child);
    }
}

fn check_node(name: &str, doc: &Document, n: &Node) {
    let t = doc.text(n.span());
    match n {
        Node::Element(e) => check_element(name, doc, e),
        Node::Text(_) => assert!(!t.is_empty() && !t.contains('<'), "{name}: text {t:?}"),
        Node::Comment(_) => assert!(t.starts_with("<!--") && t.ends_with("-->"), "{name}: {t:?}"),
        Node::CData(_) => assert!(
            t.starts_with("<![CDATA[") && t.ends_with("]]>"),
            "{name}: {t:?}"
        ),
        Node::Declaration(_) => {
            assert!(t.starts_with("<?xml") && t.ends_with("?>"), "{name}: {t:?}")
        }
        Node::ProcessingInstruction(_) => assert!(
            t.starts_with("<?") && !t.starts_with("<?xml ") && t.ends_with("?>"),
            "{name}: {t:?}"
        ),
        Node::DocType(_) => assert!(
            t.starts_with("<!DOCTYPE") && t.ends_with('>'),
            "{name}: {t:?}"
        ),
        Node::ByteOrderMark(s) => {
            assert_eq!(*s, 0..3, "{name}: BOM position");
            assert_eq!(t, "\u{feff}");
        }
    }
}

#[test]
fn spans_tile_the_source_in_every_fixture() {
    for (name, bytes) in all_fixtures() {
        let doc = Document::parse(&bytes).unwrap();
        check_contiguous(&name, "document", 0, doc.nodes(), bytes.len());
        let elements = doc
            .nodes()
            .iter()
            .filter(|n| matches!(n, Node::Element(_)))
            .count();
        assert_eq!(elements, 1, "{name}: top-level elements");
        for n in doc.nodes() {
            check_node(&name, &doc, n);
        }
    }
}

// ---- F1: what the model says ----

#[test]
fn attribute_values_are_decoded_and_raw_spans_are_kept() {
    let doc = parse("awkward-v4.xml");
    let root = doc.root_element();
    assert_eq!(root.name, "root");
    let names: Vec<&str> = root.attributes.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        names,
        ["BTCPP_format", "project_name", "main_tree_to_execute"]
    );
    assert_eq!(root.attribute("BTCPP_format"), Some("4"));
    let project = &root.attributes[1];
    assert_eq!(project.value, "a & b");
    assert_eq!(doc.text(&project.value_span), "a &amp; b");
    assert_eq!(project.quote, '\'');

    let main = root.child_elements().next().unwrap();
    let seq = main.child_elements().next().unwrap();
    assert_eq!(seq.attribute("name"), Some("seq <1>"));
    let script = seq.child_elements().next().unwrap();
    assert_eq!(script.name, "Script");
    assert_eq!(script.attribute("code"), Some(" x:='é' AB "));
    assert!(script.is_self_closing());
    let skip = seq.child_elements().nth(1).unwrap();
    assert_eq!(skip.attribute("_skipIf"), Some("a > b"));
}

#[test]
fn structure_keeps_every_kind_of_node_in_order() {
    let doc = parse("awkward-v4.xml");
    let kinds: Vec<&str> = doc
        .nodes()
        .iter()
        .map(|n| match n {
            Node::Declaration(_) => "decl",
            Node::Comment(_) => "comment",
            Node::Text(_) => "text",
            Node::Element(_) => "element",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "decl", "text", "comment", "text", "element", "text", "comment", "text"
        ]
    );

    let root = doc.root_element();
    let children: Vec<&str> = root.child_elements().map(|e| e.name.as_str()).collect();
    assert_eq!(children, ["BehaviorTree", "BehaviorTree", "TreeNodesModel"]);
    assert!(
        root.children
            .iter()
            .any(|n| matches!(n, Node::ProcessingInstruction(_)))
    );

    let seq = root
        .child_elements()
        .next()
        .unwrap()
        .child_elements()
        .next()
        .unwrap();
    let unknown = seq.child_elements().nth(2).unwrap();
    assert_eq!(unknown.name, "Unknown.Thing");
    assert!(!unknown.is_self_closing(), "<a></a> is not <a/>");
    assert!(unknown.children.is_empty());

    let fallback = root
        .child_elements()
        .nth(1)
        .unwrap()
        .child_elements()
        .next()
        .unwrap();
    assert!(
        fallback
            .children
            .iter()
            .any(|n| matches!(n, Node::CData(_)))
    );
    let text = fallback
        .children
        .iter()
        .find_map(|n| match n {
            Node::Text(s) => Some(doc.text(s)),
            _ => None,
        })
        .unwrap();
    assert_eq!(text, "text &amp; more", "text is kept raw, not decoded");
}

#[test]
fn bom_and_doctype_are_their_own_nodes() {
    let doc = parse("bom-doctype.xml");
    assert!(matches!(doc.nodes()[0], Node::ByteOrderMark(_)));
    assert!(matches!(doc.nodes()[1], Node::Declaration(_)));
    assert!(doc.nodes().iter().any(|n| matches!(n, Node::DocType(_))));
    let tree = doc.root_element().child_elements().next().unwrap();
    assert_eq!(tree.attribute("ID"), Some("Grüße"));
}

// ---- Errors ----

#[test]
fn invalid_utf8_is_reported_at_its_offset() {
    let err = Document::parse(b"<root a=\"\xff\"/>").unwrap_err();
    assert_eq!(err, ParseError::NotUtf8 { offset: 9 });
}

#[test]
fn mismatched_end_tag_names_both_tags() {
    let err = Document::parse(b"<root><a></root>").unwrap_err();
    assert_eq!(
        err,
        ParseError::MismatchedEndTag {
            offset: 9,
            expected: "a".into(),
            found: "root".into()
        }
    );
}

#[test]
fn unclosed_element_is_reported() {
    let err = Document::parse(b"<root><a/>").unwrap_err();
    assert_eq!(
        err,
        ParseError::UnclosedElement {
            offset: 10,
            open: "root".into()
        }
    );
}

#[test]
fn exactly_one_top_level_element_is_required() {
    assert_eq!(
        Document::parse(b"<root/><root/>").unwrap_err(),
        ParseError::TopLevelElementCount {
            offset: 7,
            count: 2
        }
    );
    assert_eq!(
        Document::parse(b"<!-- nothing -->\n").unwrap_err(),
        ParseError::TopLevelElementCount {
            offset: 17,
            count: 0
        }
    );
}

/// Each rule the parser enforces, with the exact offset it reports.
#[test]
fn each_wellformedness_rule_reports_its_exact_offset() {
    enum K {
        Malformed,
        Unsupported,
    }
    use K::*;
    let cases: &[(&[u8], K, usize)] = &[
        (b"<root a=\"1></root>", Malformed, 8), // unterminated attribute value
        (b"<root>&undefined;</root>", Malformed, 6), // undefined entity in text
        (b"<root a='1' a='2'/>", Malformed, 12), // duplicate attribute
        (b"<a>]]></a>", Malformed, 3),          // ]]> in text
        (b"<a b=\"<\"/>", Malformed, 6),        // < in an attribute value
        (b"<![CDATA[x]]><a/>", Malformed, 0),   // CDATA at top level
        (b" <?xml version=\"1.0\"?><a/>", Malformed, 1), // late declaration
        (b"<a b=\"1\"c=\"2\"/>", Malformed, 8), // no whitespace between attributes
        (b"<a>&#0;</a>", Malformed, 3),         // NUL by reference
        (b"<a>&#1;</a>", Malformed, 3),         // control character by reference
        (b"<a>&#xFFFE;</a>", Malformed, 3),     // non-character by reference
        (b"<a>\x01</a>", Malformed, 3),         // control character literally
        (b"<a/>x", Malformed, 4),               // text outside the top-level element
        (b"<?XmL x?><a/>", Malformed, 0),       // reserved PI target
        ("<a\u{A0}b=\"1\"/>".as_bytes(), Malformed, 2), // non-XML whitespace
        (b"<a;b/>", Malformed, 2),              // invalid name character
        ("<\u{300}a/>".as_bytes(), Malformed, 1), // combining mark cannot start a name
        (b"<a/ >", Malformed, 2),               // '/' not followed by '>'
        (b"<a><!-- a -- b --></a>", Malformed, 10), // -- inside a comment
        (b"<a><!-- x ---></a>", Malformed, 10), // comment ending --->
        (b"<a/><!DOCTYPE a>", Malformed, 4),    // DOCTYPE after the element
        (b"<!DOCTYPE a><!DOCTYPE b><a/>", Malformed, 12), // second DOCTYPE
        (b"<!DOCTYPE><a/>", Malformed, 9),      // DOCTYPE without a name
        (b"<!DOCTYPE a [ ] junk ><a/>", Malformed, 16), // junk after the subset
        (b"<?xml?><a/>", Malformed, 5),         // declaration without version
        (b"<?xml version=\"9.9\"?><a/>", Malformed, 15), // invalid version
        (
            b"<?xml version=\"1.0\" encoding=\"UTF-16\"?><a/>",
            Unsupported,
            30,
        ),
        (
            b"<!DOCTYPE a [<!ENTITY e \"x\">]><a>&e;</a>",
            Unsupported,
            13,
        ),
    ];
    for (input, kind, want) in cases {
        let shown = String::from_utf8_lossy(input);
        match (Document::parse(input), kind) {
            (Err(ParseError::Malformed { offset, .. }), Malformed)
            | (Err(ParseError::Unsupported { offset, .. }), Unsupported) => {
                assert_eq!(offset, *want, "{shown:?}")
            }
            (other, _) => panic!("{shown:?}: {other:?}"),
        }
    }
}

/// Well-formed inputs the parser must accept, and round-trip exactly.
#[test]
fn wellformed_edge_cases_are_accepted_and_round_trip() {
    let cases: &[&str] = &[
        "<!DOCTYPE a [<!ELEMENT a (#PCDATA)>]><a/>",
        "<!DOCTYPE a SYSTEM \"x[y.dtd\"><a/>",
        "<!DOCTYPE a [<!-- ] -->]><a/>",
        "<!DOCTYPE a [<!ATTLIST a b CDATA \"]>\">]><a/>",
        "<!DOCTYPE a PUBLIC \"p\" 's'><a/>",
        "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?><a/>",
        "<?xml-stylesheet href=\"x\"?><a/>",
        "<:a/>",
        "<a-b.c_d:e/>",
        "<été/>",
        "<文/>",
        "<a><!-- > --></a>",
        "<a><?pi > x?></a>",
        "<a><![CDATA[ ]] ]]></a>",
        "<a>&#x10FFFF;&#9;</a>",
        "<a\r\n\tb\r\n=\r\n'1'\r\n/>",
        "<a></a \r\n>",
    ];
    for input in cases {
        let doc = Document::parse(input.as_bytes()).unwrap_or_else(|e| panic!("{input:?}: {e}"));
        assert_eq!(doc.render(), *input);
        check_contiguous(input, "document", 0, doc.nodes(), input.len());
        for n in doc.nodes() {
            check_node(input, &doc, n);
        }
    }
}
