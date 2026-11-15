//! The behavior-tree view (rest of F1; F2, F4, F10), against the
//! self-authored fixtures in `tests/fixtures/doc/`, the reference fixture
//! set in `tests/fixtures/reference/o-answers/`, and inline inputs.

use std::path::PathBuf;

use coppice_doc::{
    DiagnosticKind, Dialect, Document, ModelKind, NotATreeDocument, PortDirection, TreeDocument,
};

fn fixture(rel: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn view(bytes: &[u8]) -> TreeDocument {
    TreeDocument::new(Document::parse(bytes).unwrap()).unwrap()
}

fn kinds(t: &TreeDocument) -> Vec<DiagnosticKind> {
    t.diagnostics().iter().map(|d| d.kind).collect()
}

#[test]
fn root_attributes_and_trees() {
    let t = view(&fixture("doc/awkward-v4.xml"));
    assert_eq!(t.format(), Some("4"));
    assert_eq!(t.version(), None);
    assert_eq!(t.project_name(), Some("a & b"));
    let ids: Vec<_> = t
        .behavior_trees()
        .iter()
        .map(|e| e.attribute("ID").unwrap())
        .collect();
    assert_eq!(ids, ["Main", "Sub"]);
    assert_eq!(t.suggested_dialect(), Dialect::V4);
}

#[test]
fn includes_keep_path_and_ros_pkg_in_order() {
    let src = fixture("doc/project.btproj");
    let t = view(&src);
    let inc = t.includes();
    let paths: Vec<_> = inc.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(
        paths,
        ["trees/main.xml", "../shared/common.xml", "bt/extra.xml"]
    );
    assert_eq!(inc[0].ros_pkg, None);
    assert_eq!(inc[2].ros_pkg.as_deref(), Some("some_pkg"));
    for i in &inc {
        assert!(t.document().source()[i.offset..].starts_with("<include"));
    }
    assert!(t.behavior_trees().is_empty(), "a project may hold no trees");
    assert_eq!(t.project_name(), Some("Demo"));
}

#[test]
fn models_with_ports_metadata_and_extensions() {
    let t = view(&fixture("doc/awkward-v4.xml"));
    let m = t.models();
    assert_eq!(m.len(), 1);
    let say = &m[0];
    assert_eq!((say.kind, say.id.as_str()), (ModelKind::Action, "SayHi"));
    assert!(say.editable);
    assert!(say.has_description);
    assert_eq!(say.ports.len(), 1);
    let p = &say.ports[0];
    assert_eq!(p.direction, PortDirection::Input);
    assert_eq!(p.name, "msg");
    assert_eq!(p.type_name.as_deref(), Some("std::string"));
    assert_eq!(p.default.as_deref(), Some("hi"));
    assert_eq!(p.description, "the \"greeting\"");

    let src = br#"<root BTCPP_format="4"><TreeNodesModel>
      <Decorator ID="D"><output_port name="o"/><inout_port name="io" default=""/>
        <MetadataFields><Metadata a="1"/><Metadata b="2"/></MetadataFields>
      </Decorator>
      <Control ID="C"/><Condition ID="Q"/><SubTree ID="S"><input_port name="frame" default="world"/></SubTree>
    </TreeNodesModel></root>"#;
    let m = view(src).models();
    let kinds: Vec<_> = m.iter().map(|x| (x.kind, x.id.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (ModelKind::Decorator, "D"),
            (ModelKind::Control, "C"),
            (ModelKind::Condition, "Q"),
            (ModelKind::SubTree, "S"),
        ]
    );
    let d = &m[0];
    assert_eq!(d.ports[0].direction, PortDirection::Output);
    assert_eq!(d.ports[1].direction, PortDirection::InOut);
    assert_eq!(d.ports[1].default, None, "an empty default is absent");
    assert_eq!(d.ports[0].description, "");
    assert_eq!(
        d.metadata,
        [
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "2".to_owned())
        ]
    );
    assert!(!d.editable && !d.has_description);
    assert_eq!(m[3].ports[0].default.as_deref(), Some("world"));
}

#[test]
fn dialect_suggestion_follows_spec_2_5() {
    assert_eq!(
        view(&fixture("doc/v3-dialect.xml")).suggested_dialect(),
        Dialect::V3
    );
    assert_eq!(
        view(b"<root BTCPP_format=\"4\"><BehaviorTree ID=\"T\"><SubTreePlus ID=\"S\"/></BehaviorTree></root>")
            .suggested_dialect(),
        Dialect::V4,
        "BTCPP_format decides when present"
    );
    assert_eq!(
        view(b"<root><BehaviorTree ID=\"T\"><Sequence><SubTreePlus ID=\"S\"/></Sequence></BehaviorTree></root>")
            .suggested_dialect(),
        Dialect::V3,
        "SubTreePlus anywhere suggests v3 without BTCPP_format"
    );
    assert_eq!(
        view(b"<root><BehaviorTree ID=\"T\"><SetBlackboard output_key=\"k\" value=\"1\"/></BehaviorTree></root>")
            .suggested_dialect(),
        Dialect::V4,
        "SetBlackboard is not a v3 marker"
    );
    assert_eq!(
        view(&fixture("doc/no-format.xml")).suggested_dialect(),
        Dialect::V4
    );
}

#[test]
fn missing_format_is_reported_at_the_root() {
    let t = view(&fixture("doc/no-format.xml"));
    let d = t.diagnostics();
    assert_eq!(kinds(&t), [DiagnosticKind::MissingFormat]);
    assert_eq!(d[0].offset, 0);
    assert!(kinds(&view(&fixture("doc/awkward-v4.xml"))).is_empty());
}

#[test]
fn leaf_with_children_built_in_and_declared() {
    // The reference fixture: an AlwaysSuccess wrapping a Script.
    let src = fixture("reference/o-answers/in-o1-proj/tree.xml");
    let t = view(&src);
    let d: Vec<_> = t
        .diagnostics()
        .into_iter()
        .filter(|d| d.kind == DiagnosticKind::LeafWithChildren)
        .collect();
    assert_eq!(d.len(), 1);
    assert!(t.document().source()[d[0].offset..].starts_with("<AlwaysSuccess"));
    assert!(d[0].message.contains("AlwaysSuccess"));

    // Its corrected sibling has no such diagnostic.
    let ok = view(&fixture(
        "reference/o-answers/in-o1-proj-corrected/tree.xml",
    ));
    assert!(!kinds(&ok).contains(&DiagnosticKind::LeafWithChildren));

    // A declared Condition with a child, and a declared Control with one (fine).
    let src = br#"<root BTCPP_format="4"><BehaviorTree ID="T"><MyCtl><IsOk><A/></IsOk></MyCtl></BehaviorTree>
      <TreeNodesModel><Condition ID="IsOk"/><Control ID="MyCtl"/></TreeNodesModel></root>"#;
    let t = view(src);
    let leaf: Vec<_> = t
        .diagnostics()
        .into_iter()
        .filter(|d| d.kind == DiagnosticKind::LeafWithChildren)
        .collect();
    assert_eq!(leaf.len(), 1);
    assert!(t.document().source()[leaf[0].offset..].starts_with("<IsOk"));

    // Text or a comment inside a leaf is not a child element.
    let t = view(b"<root BTCPP_format=\"4\"><BehaviorTree ID=\"T\"><Script code=\"x\"><!-- c --> </Script></BehaviorTree></root>");
    assert!(kinds(&t).is_empty());
}

#[test]
fn unknown_port_on_a_declared_model() {
    let src = br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Sequence>
        <Say msg="hi" name="n" _skipIf="x" _uid="3" tone="loud"/>
        <SubTree ID="S" anything="goes"/>
        <Undeclared whatever="1"/>
      </Sequence></BehaviorTree>
      <TreeNodesModel><Action ID="Say"><input_port name="msg"/></Action></TreeNodesModel></root>"#;
    let t = view(src);
    let d = t.diagnostics();
    assert_eq!(kinds(&t), [DiagnosticKind::UnknownPort]);
    assert!(t.document().source()[d[0].offset..].starts_with("tone="));
    assert!(d[0].message.contains("tone") && d[0].message.contains("Say"));

    // The reference fixture's EditableModel uses its declared port: clean.
    let tree = view(&fixture("reference/o-answers/out-o6/tree.xml"));
    assert!(!kinds(&tree).contains(&DiagnosticKind::UnknownPort));
}

#[test]
fn malformed_model_entries_are_reported_and_skipped() {
    let src = br#"<root BTCPP_format="4">
      <TreeNodesModel><Widget ID="W"/><Action/><Action ID="A"><input_port/></Action></TreeNodesModel>
      <TreeNodesModel><Condition ID="C"/></TreeNodesModel></root>"#;
    let t = view(src);
    assert_eq!(
        kinds(&t),
        [
            DiagnosticKind::UnknownModelKind,
            DiagnosticKind::IncompleteModel,
            DiagnosticKind::IncompleteModel,
            DiagnosticKind::DuplicateModelsBlock,
        ]
    );
    let offsets: Vec<_> = t.diagnostics().iter().map(|d| d.offset).collect();
    assert!(offsets.windows(2).all(|w| w[0] <= w[1]), "sorted by offset");
    let ids: Vec<_> = t.models().iter().map(|m| m.id.clone()).collect();
    assert_eq!(
        ids,
        ["A", "C"],
        "unknown kinds and ID-less entries are skipped; blocks merge"
    );
    assert!(
        t.models()[0].ports.is_empty(),
        "a port without a name is skipped"
    );
}

#[test]
fn a_document_whose_top_element_is_not_root_is_refused() {
    let err = TreeDocument::new(Document::parse(b"<!-- x --><tree/>").unwrap()).unwrap_err();
    assert_eq!(
        err,
        NotATreeDocument {
            offset: 10,
            found: "tree".into()
        }
    );
}

#[test]
fn every_fixture_views_without_panicking_and_stays_lossless() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut stack = vec![base.join("doc"), base.join("reference")];
    let mut n = 0;
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let bytes = std::fs::read(&p).unwrap();
            let t = view(&bytes);
            let _ = (
                t.includes(),
                t.behavior_trees(),
                t.models(),
                t.diagnostics(),
            );
            let _ = t.suggested_dialect();
            assert_eq!(t.document().render().as_bytes(), bytes.as_slice());
            n += 1;
        }
    }
    assert!(n >= 51, "fixture set went missing: {n}");
}

#[test]
fn port_details_edge_cases() {
    let src = br#"<root BTCPP_format="4"><TreeNodesModel>
      <Action ID="A" editable="false"><input_port name="p">x &amp; <![CDATA[&amp; <raw>]]> y</input_port>
        <input_port name=""/><output_port/></Action>
    </TreeNodesModel></root>"#;
    let t = view(src);
    let m = &t.models()[0];
    assert!(!m.editable, "only editable=\"true\" counts");
    assert_eq!(m.ports.len(), 1, "empty and missing port names are skipped");
    assert_eq!(
        m.ports[0].description, "x & &amp; <raw> y",
        "CDATA stays literal"
    );
    let incomplete: Vec<_> = t
        .diagnostics()
        .into_iter()
        .filter(|d| d.kind == DiagnosticKind::IncompleteModel)
        .collect();
    assert_eq!(
        incomplete.len(),
        2,
        "both the empty and the missing name are reported"
    );
    let s = t.document().source();
    assert!(s[incomplete[0].offset..].starts_with("<input_port name=\"\""));
    assert!(s[incomplete[1].offset..].starts_with("<output_port"));
    assert!(incomplete[0].message.contains("model A"));
}

#[test]
fn subtree_spellings_are_never_checked_even_if_declared() {
    let src = br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Sequence>
        <SubTreePlus ID="S" remapped="{x}"><A/></SubTreePlus>
      </Sequence></BehaviorTree>
      <TreeNodesModel><Action ID="SubTreePlus"><input_port name="only"/></Action></TreeNodesModel></root>"#;
    assert!(kinds(&view(src)).is_empty());
}

fn only(t: &TreeDocument, k: DiagnosticKind) -> Vec<coppice_doc::Diagnostic> {
    t.diagnostics()
        .into_iter()
        .filter(|d| d.kind == k)
        .collect()
}

#[test]
fn explicit_node_form_is_looked_up_by_id() {
    let t = view(br#"<root BTCPP_format="3"><BehaviorTree ID="T"><Action ID="AlwaysSuccess"><Action ID="Script" code="x"/></Action></BehaviorTree></root>"#);
    let leaf = only(&t, DiagnosticKind::LeafWithChildren);
    assert_eq!(leaf.len(), 1);
    assert!(t.document().source()[leaf[0].offset..].starts_with("<Action ID=\"AlwaysSuccess\""));

    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Action ID="Say" msg="a" tone="x"/></BehaviorTree>
      <TreeNodesModel><Action ID="Say"><input_port name="msg"/></Action></TreeNodesModel></root>"#);
    let d = t.diagnostics();
    assert_eq!(
        kinds(&t),
        [DiagnosticKind::UnknownPort],
        "ID selects the type and is not a port"
    );
    assert!(t.document().source()[d[0].offset..].starts_with("tone="));

    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Action/></BehaviorTree></root>"#);
    assert_eq!(kinds(&t), [DiagnosticKind::MisplacedNodeId]);
}

#[test]
fn built_in_kinds_win_over_declared_models() {
    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><AlwaysSuccess><Script code="x"/></AlwaysSuccess></BehaviorTree>
      <TreeNodesModel><Control ID="AlwaysSuccess"/></TreeNodesModel></root>"#);
    assert_eq!(
        kinds(&t),
        [
            DiagnosticKind::LeafWithChildren,
            DiagnosticKind::ModelShadowsBuiltin
        ]
    );
    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Sequence><Script code="x"/></Sequence></BehaviorTree>
      <TreeNodesModel><Action ID="Sequence"/></TreeNodesModel></root>"#);
    assert_eq!(
        kinds(&t),
        [DiagnosticKind::ModelShadowsBuiltin],
        "Sequence stays a Control"
    );
}

#[test]
fn only_port_shaped_attribute_names_are_checked() {
    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Say msg="a" _description="d" __shared_blackboard="1" _foo="z" a.b="q" _uid="3" tone="t"/></BehaviorTree>
      <TreeNodesModel><Action ID="Say"><input_port name="msg"/></Action></TreeNodesModel></root>"#);
    let d = t.diagnostics();
    assert_eq!(kinds(&t), [DiagnosticKind::UnknownPort]);
    assert!(t.document().source()[d[0].offset..].starts_with("tone="));
    assert!(coppice_doc::is_port_name("tone"));
    for n in ["_while", "name", "ID", "a.b", "a:b", "1x", "_x", ""] {
        assert!(!coppice_doc::is_port_name(n), "{n:?}");
    }

    let t = view(
        br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Say ID="x" msg="a"/></BehaviorTree>
      <TreeNodesModel><Action ID="Say"><input_port name="msg"/></Action></TreeNodesModel></root>"#,
    );
    let d = t.diagnostics();
    assert_eq!(kinds(&t), [DiagnosticKind::MisplacedNodeId]);
    assert!(t.document().source()[d[0].offset..].starts_with("ID="));
}

#[test]
fn diagnostics_are_sorted_by_offset_across_sources() {
    // Tree before the model block: unsorted output would list the model
    // diagnostic first.
    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><AlwaysSuccess><A/></AlwaysSuccess></BehaviorTree>
      <TreeNodesModel><Widget ID="W"/></TreeNodesModel></root>"#);
    assert_eq!(
        kinds(&t),
        [
            DiagnosticKind::LeafWithChildren,
            DiagnosticKind::UnknownModelKind
        ]
    );
}

#[test]
fn duplicate_model_ids_are_reported_and_the_first_wins() {
    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Foo a="1"><X/></Foo></BehaviorTree>
      <TreeNodesModel><Control ID="Foo"><input_port name="a"/></Control><Action ID="Foo"/></TreeNodesModel></root>"#);
    let d = t.diagnostics();
    assert_eq!(kinds(&t), [DiagnosticKind::DuplicateModel]);
    assert!(t.document().source()[d[0].offset..].starts_with("<Action ID=\"Foo\""));
}

#[test]
fn view_details_the_first_tests_did_not_pin() {
    let t = view(&fixture("reference/o-answers/out-o5/in-o5-single.xml"));
    assert_eq!(t.version(), Some("4.10"));

    let t = view(br#"<root BTCPP_format="4"><BehaviorTree ID="T"><Dec><A/></Dec></BehaviorTree>
      <TreeNodesModel><Decorator ID="Dec"><MetadataFields><Other k="v"/><Metadata a="1"/></MetadataFields></Decorator></TreeNodesModel>
      <Models><Action ID="Stray"/></Models></root>"#);
    assert!(kinds(&t).is_empty(), "a Decorator is not a leaf");
    let m = t.models();
    assert_eq!(m.len(), 1, "models come only from <TreeNodesModel>");
    assert!(
        !m[0].has_description,
        "only <description> sets has_description"
    );
    assert_eq!(
        m[0].metadata,
        [("a".to_owned(), "1".to_owned())],
        "only <Metadata> children"
    );

    for leaf in coppice_doc::BUILTIN_LEAVES {
        let src = format!(
            r#"<root BTCPP_format="4"><BehaviorTree ID="T"><{leaf}><A/></{leaf}></BehaviorTree></root>"#
        );
        assert_eq!(
            kinds(&view(src.as_bytes())),
            [DiagnosticKind::LeafWithChildren],
            "{leaf}"
        );
    }
    for branch in coppice_doc::BUILTIN_BRANCHES {
        if *branch == "SubTree" {
            continue;
        }
        let src = format!(
            r#"<root BTCPP_format="4"><BehaviorTree ID="T"><{branch}><A/></{branch}></BehaviorTree></root>"#
        );
        assert!(kinds(&view(src.as_bytes())).is_empty(), "{branch}");
    }

    let t = view(b"<root BTCPP_format=\"4\"><TreeNodesModel><Action ID=\"A\"><input_port name=\"p\">x\r\ny</input_port></Action></TreeNodesModel></root>");
    assert_eq!(
        t.models()[0].ports[0].description,
        "x\ny",
        "line ends normalized"
    );
}

/// The built-in lists, pinned to BehaviorTree.CPP 4.9.0's registrations
/// (`src/bt_factory.cpp:118-173`), so an entry cannot go missing unnoticed.
#[test]
fn built_in_lists_match_the_library_registrations() {
    let mut leaves = coppice_doc::BUILTIN_LEAVES.to_vec();
    leaves.sort_unstable();
    assert_eq!(
        leaves,
        [
            "AlwaysFailure",
            "AlwaysSuccess",
            "Script",
            "ScriptCondition",
            "SetBlackboard",
            "Sleep",
            "UnsetBlackboard",
            "WasEntryUpdated",
        ]
    );
    let mut branches = coppice_doc::BUILTIN_BRANCHES.to_vec();
    branches.sort_unstable();
    assert_eq!(
        branches,
        [
            "AsyncFallback",
            "AsyncSequence",
            "Delay",
            "Fallback",
            "ForceFailure",
            "ForceSuccess",
            "IfThenElse",
            "Inverter",
            "KeepRunningUntilFailure",
            "LoopBool",
            "LoopDouble",
            "LoopInt",
            "LoopString",
            "Parallel",
            "ParallelAll",
            "Precondition",
            "ReactiveFallback",
            "ReactiveSequence",
            "Repeat",
            "RetryUntilSuccessful",
            "RunOnce",
            "Sequence",
            "SequenceWithMemory",
            "SkipUnlessUpdated",
            "SubTree",
            "Switch2",
            "Switch3",
            "Switch4",
            "Switch5",
            "Switch6",
            "Timeout",
            "TryCatch",
            "WaitValueUpdate",
            "WhileDoElse",
        ]
    );
}

#[test]
fn library_written_models_of_built_ins_are_not_reported() {
    // The shape BehaviorTree.CPP writes into monitor and log XML: built-in
    // models declared with their own kinds, and runtime metadata.
    let t = view(
        br#"<root BTCPP_format="4"><BehaviorTree ID="T" _fullpath="">
        <Sequence name="s" _uid="1"><AlwaysSuccess _uid="2"/><Script code="x" _uid="3"/></Sequence>
      </BehaviorTree>
      <TreeNodesModel><Control ID="Sequence"/><Action ID="AlwaysSuccess"/>
        <Action ID="Script"><input_port name="code"/></Action></TreeNodesModel></root>"#,
    );
    assert!(kinds(&t).is_empty(), "{:?}", t.diagnostics());
    // Port names: only an ASCII letter may start one, as in the library.
    assert!(!coppice_doc::is_port_name("ébc"));
    assert!(coppice_doc::is_port_name("aé"));
}
