//! The reference writer (F6, F7, F9; spec §3.2, §3.4, §3.5, §3.7) against
//! the reference application's own output: every save, save-as and
//! per-file-save row of the spec's §8 oracle table, byte for byte, plus
//! the writer's refusals.

use std::path::PathBuf;

use coppice_doc::{
    DiagnosticKind, Document, Project, ReferenceOptions, TreeDocument, WriteMode, write_tree,
};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/reference")
}

fn read(rel: &str) -> String {
    let p = fixtures().join(rel);
    String::from_utf8(std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn view(src: &str) -> TreeDocument {
    TreeDocument::new(Document::parse(src.as_bytes()).unwrap()).unwrap()
}

/// Compare, showing the first differing line on failure.
fn assert_bytes(got: &str, want: &str, what: &str) {
    if got != want {
        let line = got
            .lines()
            .zip(want.lines())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| got.lines().count().min(want.lines().count()));
        panic!(
            "{what}: output differs at line {}\n--- got ---\n{got}\n--- want ---\n{want}",
            line + 1
        );
    }
}

/// Standalone save-as (F9): the file's own models.
fn save_as(input: &str, output: &str) {
    let t = view(&read(input));
    let w = write_tree(&t, &t.models(), &ReferenceOptions::default())
        .unwrap_or_else(|e| panic!("{input}: refused: {e:#?}"));
    assert_bytes(&w.text, &read(output), input);
}

/// Per-file save through a project (F6, F7): the project's merged models.
fn project_save(project: &str, tree: &str, output: &str, options: &ReferenceOptions) {
    let p = Project::load(&fixtures().join(project)).unwrap();
    let want = std::fs::canonicalize(fixtures().join(tree)).unwrap();
    let d = p
        .documents()
        .iter()
        .find(|d| d.canonical_path == want)
        .unwrap_or_else(|| panic!("{tree} is not in {project}"));
    let w = write_tree(&d.tree, &p.models(), options)
        .unwrap_or_else(|e| panic!("{tree}: refused: {e:#?}"));
    assert_bytes(&w.text, &read(output), tree);
}

// F6 rows.

#[test]
fn f6_unmodified_tree_in_a_project_with_an_unused_model() {
    project_save(
        "o-answers/in-o2-same-corrected/proj.btproj",
        "o-answers/in-o2-same-corrected/tree.xml",
        "o-answers/in-o1-proj-corrected/tree.xml",
        &ReferenceOptions::default(),
    );
}

#[test]
fn f6_the_o4_trees_as_single_trees() {
    for (input, output) in [
        (
            "o-answers/in-o4-paths/sub/tree.xml",
            "o-answers/out-o4/tree.xml",
        ),
        (
            "o-answers/in-o4-paths/deep/nest/tree2.xml",
            "o-answers/out-o4/tree2.xml",
        ),
        ("o-answers/in-o4-paths/abs.xml", "o-answers/out-o4/abs.xml"),
        ("o-answers/outside.xml", "o-answers/out-o4/outside.xml"),
    ] {
        save_as(input, output);
    }
}

#[test]
fn f6_every_declared_port_is_written_and_two_or_more_attributes_wrap() {
    project_save(
        "o-answers-2/out-o10-corrected/proj.btproj",
        "o-answers-2/out-o10-corrected/tree.xml",
        "o-answers-2/out-o10/tree.xml",
        &ReferenceOptions::default(),
    );
}

#[test]
fn f6_the_model_block_holds_the_models_the_tree_uses() {
    for (tree, output) in [
        (
            "o-answers-2/in-o14-models/tree.xml",
            "o-answers-2/out-o14/tree.xml",
        ),
        (
            "o-answers-2/in-o14-models/unused.xml",
            "o-answers-2/out-o14/unused.xml",
        ),
    ] {
        project_save(
            "o-answers-2/in-o14-models/proj.btproj",
            tree,
            output,
            &ReferenceOptions::default(),
        );
    }
}

#[test]
fn f6_an_older_stamp_is_raised_and_the_root_normalized() {
    for (tree, output) in [
        (
            "o-answers-2/in-o16-stamps/stamped.xml",
            "o-answers-2/out-o16b/stamped.xml",
        ),
        (
            "o-answers-2/in-o16-stamps/ordered.xml",
            "o-answers-2/out-o16b/ordered.xml",
        ),
    ] {
        project_save(
            "o-answers-2/in-o16-stamps/proj-stamp.btproj",
            tree,
            output,
            &ReferenceOptions::default(),
        );
    }
}

#[test]
fn f6_a_newer_stamp_is_written_once_the_target_is_switched() {
    project_save(
        "o-answers-2/in-o16-stamps/proj-411.btproj",
        "o-answers-2/in-o16-stamps/t411.xml",
        "o-answers-2/out-o16a/t411.xml",
        &ReferenceOptions {
            target: "4.11".to_owned(),
            ..ReferenceOptions::default()
        },
    );
}

// F7 row.

#[test]
fn f7_a_v3_mode_save_uses_the_same_writer_without_a_stamp() {
    project_save(
        "o-answers-2/in-o12-v3/proj.btproj",
        "o-answers-2/in-o12-v3/tree.xml",
        "o-answers-2/out-o12/tree.xml",
        &ReferenceOptions {
            mode: WriteMode::V3,
            ..ReferenceOptions::default()
        },
    );
}

// F9 rows.

#[test]
fn f9_standalone_save_as() {
    for (input, output) in [
        (
            "o-answers/in-o5-single.xml",
            "o-answers/out-o5/in-o5-single.xml",
        ),
        (
            "o-answers-2/in-o13-escape/content.xml",
            "o-answers-2/out-o13/content.xml",
        ),
        (
            "o-answers-2/in-o13-escape/two-trees-tab.xml",
            "o-answers-2/out-o13/two-trees-tab.xml",
        ),
        (
            "o-answers-2/out-o13-corrected/attr-escapes.xml",
            "o-answers-2/out-o13/attr-escapes.xml",
        ),
        (
            "o-answers-2/out-o13-corrected/mixed-forms.xml",
            "o-answers-2/out-o13/mixed-forms.xml",
        ),
        (
            "o-answers-2/out-o13-corrected/two-trees-blank.xml",
            "o-answers-2/out-o13/two-trees-blank.xml",
        ),
    ] {
        save_as(input, output);
    }
}

#[test]
fn f9_the_awkward_set_round_trips_to_the_reference_bytes() {
    for name in [
        "altquote", "charrefs", "comments", "crlf", "nodecl", "notrail",
    ] {
        save_as(
            &format!("o-answers/in-o9-awkward/{name}.xml"),
            &format!("o-answers/in-o9-awkward/{name}-rt.xml"),
        );
    }
}

// Refusals and notes (spec §3.7).

fn v4(body: &str) -> String {
    format!("<root BTCPP_format=\"4\">{body}</root>")
}

fn tree_of(nodes: &str) -> String {
    v4(&format!("<BehaviorTree ID=\"T\">{nodes}</BehaviorTree>"))
}

/// (kind, offset) of each refusal, writing `src` standalone.
fn refusals(src: &str, options: &ReferenceOptions) -> Vec<(DiagnosticKind, usize)> {
    let t = view(src);
    match write_tree(&t, &t.models(), options) {
        Ok(w) => panic!("expected a refusal, wrote:\n{}", w.text),
        Err(d) => d.iter().map(|d| (d.kind, d.offset)).collect(),
    }
}

fn at(src: &str, needle: &str) -> usize {
    assert_eq!(src.matches(needle).count(), 1, "{needle}");
    src.find(needle).unwrap()
}

#[test]
fn cases_the_record_does_not_cover_are_refused_where_they_occur() {
    let d = ReferenceOptions::default();
    let not = DiagnosticKind::NotInRecord;
    for (src, needle) in [
        (tree_of(r#"<Action ID="AlwaysSuccess"/>"#), "<Action"),
        (tree_of(r#"<Script code="x" _skipIf="y"/>"#), "_skipIf"),
        (tree_of(r#"<SubTree ID="T" name="n"/>"#), "name="),
        (tree_of(r#"<SubTree ID="T" goal="{g}"/>"#), "goal="),
        (tree_of(r#"<Script code="a&#10;b"/>"#), "\"a&#10;b\""),
        (
            r#"<root BTCPP_format="4" main_tree_to_execute="T"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#.to_owned(),
            "main_tree_to_execute",
        ),
        (v4(r#"<include path="x.xml"/>"#), "<include"),
        (v4(r#"<Unknown/>"#), "<Unknown"),
        (v4(r#"<BehaviorTree ID="T" extra="1"><AlwaysSuccess/></BehaviorTree>"#), "extra"),
        (v4(r#"<BehaviorTree><AlwaysSuccess/></BehaviorTree>"#), "<BehaviorTree"),
        (
            r#"<root BTCPP_format="4" BTCPP_version="next"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#.to_owned(),
            "<root",
        ),
        (
            r#"<root BTCPP_format="5"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#.to_owned(),
            "<root",
        ),
    ] {
        let offset = if needle.starts_with('"') {
            at(&src, needle) + 1
        } else {
            at(&src, needle)
        };
        assert_eq!(refusals(&src, &d), [(not, offset)], "{src}");
    }
}

#[test]
fn a_declared_model_the_writer_cannot_write_is_refused() {
    let d = ReferenceOptions::default();
    for model in [
        r#"<Action ID="M"><input_port name="p" default="1"/></Action>"#,
        r#"<Action ID="M"><input_port name="p">a description</input_port></Action>"#,
        r#"<Action ID="M"><input_port name="p"/><MetadataFields><Metadata k="v"/></MetadataFields></Action>"#,
    ] {
        let src = v4(&format!(
            r#"<BehaviorTree ID="T"><M p="1"/></BehaviorTree><TreeNodesModel>{model}</TreeNodesModel>"#
        ));
        assert_eq!(
            refusals(&src, &d),
            [(DiagnosticKind::NotInRecord, at(&src, r#"<Action ID="M""#))],
            "{model}"
        );
    }
    // A declared model of a built-in, used: what the reference writes is
    // not in the record.
    let src = v4(
        r#"<BehaviorTree ID="T"><Script code="1"/></BehaviorTree><TreeNodesModel><Action ID="Script"><input_port name="code"/></Action></TreeNodesModel>"#,
    );
    assert_eq!(
        refusals(&src, &d),
        [(DiagnosticKind::NotInRecord, at(&src, "<Script code"))]
    );
}

#[test]
fn inputs_with_a_diagnostic_are_refused_with_it() {
    let d = ReferenceOptions::default();
    let src = tree_of("<AlwaysSuccess><AlwaysFailure/></AlwaysSuccess>");
    assert_eq!(
        refusals(&src, &d),
        [(
            DiagnosticKind::LeafWithChildren,
            at(&src, "<AlwaysSuccess>")
        )]
    );
    let src = tree_of(r#"<Script code="1" note="2"/>"#);
    assert_eq!(
        refusals(&src, &d),
        [(DiagnosticKind::UnknownPort, at(&src, "note"))]
    );
    let src = "<root><BehaviorTree ID=\"T\"><AlwaysSuccess/></BehaviorTree></root>";
    assert_eq!(refusals(src, &d), [(DiagnosticKind::MissingFormat, 0)]);
}

#[test]
fn stamps_newer_than_the_target_are_refused_and_older_ones_raised_with_a_note() {
    let src = r#"<root BTCPP_format="4" BTCPP_version="4.11"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#;
    assert_eq!(
        refusals(src, &ReferenceOptions::default()),
        [(DiagnosticKind::NewerStamp, 0)]
    );
    // Numeric, not textual: 4.9 is older than 4.10.
    let src = r#"<root BTCPP_format="4" BTCPP_version="4.9"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#;
    let t = view(src);
    let w = write_tree(&t, &[], &ReferenceOptions::default()).unwrap();
    let notes: Vec<_> = w.notes.iter().map(|n| (n.kind, n.offset)).collect();
    assert_eq!(notes, [(DiagnosticKind::StampRaised, 0)]);
    assert!(w.text.contains("BTCPP_version=\"4.10\""));
    // Equal or absent: no note.
    for src in [
        r#"<root BTCPP_format="4" BTCPP_version="4.10"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#,
        r#"<root BTCPP_format="4"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#,
    ] {
        let t = view(src);
        assert_eq!(
            write_tree(&t, &[], &ReferenceOptions::default())
                .unwrap()
                .notes,
            []
        );
    }
}

#[test]
fn v4_refuses_an_undeclared_custom_node() {
    let src = tree_of(r#"<Custom p="1"/>"#);
    assert_eq!(
        refusals(&src, &ReferenceOptions::default()),
        [(DiagnosticKind::UndeclaredNode, at(&src, "<Custom"))]
    );
}

fn v3() -> ReferenceOptions {
    ReferenceOptions {
        mode: WriteMode::V3,
        ..ReferenceOptions::default()
    }
}

fn v3_tree(nodes: &str) -> String {
    format!("<root BTCPP_format=\"3\"><BehaviorTree ID=\"T\">{nodes}</BehaviorTree></root>")
}

#[test]
fn v3_creates_the_importers_model_and_notes_it() {
    let src = v3_tree(
        r#"<Sequence><Custom b="1" name="n" a="2"/><Custom b="3" a="4"/><SubTreePlus ID="T"/></Sequence>"#,
    );
    let t = view(&src);
    let w = write_tree(&t, &[], &v3()).unwrap();
    let notes: Vec<_> = w.notes.iter().map(|n| (n.kind, n.offset)).collect();
    assert_eq!(
        notes,
        [(DiagnosticKind::ModelCreated, at(&src, r#"<Custom b="1""#))]
    );
    assert!(w.text.contains(
        "    <Action ID=\"Custom\"\n            editable=\"true\">\n      <input_port name=\"b\"/>\n      <input_port name=\"a\"/>\n    </Action>\n"
    ), "{}", w.text);
    assert!(w.text.contains("<SubTreePlus ID=\"T\"/>"), "{}", w.text);
}

#[test]
fn v3_refuses_what_the_importer_case_does_not_cover() {
    // A built-in the record does not place in V3 mode.
    let src = v3_tree("<Fallback><AlwaysSuccess/></Fallback>");
    let r = refusals(&src, &v3());
    assert!(
        r.contains(&(DiagnosticKind::NotInRecord, at(&src, "<Fallback"))),
        "{r:?}"
    );
    // The same undeclared node with different attributes.
    let src = v3_tree(r#"<Sequence><Custom a="1"/><Custom b="2"/></Sequence>"#);
    assert_eq!(
        refusals(&src, &v3()),
        [(DiagnosticKind::NotInRecord, at(&src, r#"<Custom b"#))]
    );
    // ...also when the uses are direct children of the tree.
    let src = v3_tree(r#"<Custom a="1"/><Custom b="2"/>"#);
    assert_eq!(
        refusals(&src, &v3()),
        [(DiagnosticKind::NotInRecord, at(&src, r#"<Custom b"#))]
    );
    // An undeclared node with children.
    let src = v3_tree(r#"<Custom><Script code="1"/></Custom>"#);
    assert_eq!(
        refusals(&src, &v3()),
        [(DiagnosticKind::NotInRecord, at(&src, "<Custom"))]
    );
    // A stamp in V3 mode, and a v4 file in V3 mode.
    let src = r#"<root BTCPP_format="3" BTCPP_version="4.10"><BehaviorTree ID="T"><Script code="1"/></BehaviorTree></root>"#;
    assert_eq!(refusals(src, &v3()), [(DiagnosticKind::NotInRecord, 0)]);
    let src = tree_of(r#"<Script code="1"/>"#);
    assert_eq!(refusals(&src, &v3()), [(DiagnosticKind::NotInRecord, 0)]);
}

#[test]
fn unset_declared_ports_can_be_left_out() {
    let src = v4(
        r#"<BehaviorTree ID="T"><M b="1"/></BehaviorTree><TreeNodesModel><Action ID="M"><input_port name="a"/><input_port name="b"/></Action></TreeNodesModel>"#,
    );
    let t = view(&src);
    let all = write_tree(&t, &t.models(), &ReferenceOptions::default()).unwrap();
    assert!(
        all.text.contains("<M a=\"\"\n       b=\"1\"/>"),
        "{}",
        all.text
    );
    let set_only = write_tree(
        &t,
        &t.models(),
        &ReferenceOptions {
            write_unset_ports: false,
            ..ReferenceOptions::default()
        },
    )
    .unwrap();
    assert!(set_only.text.contains("<M b=\"1\"/>"), "{}", set_only.text);
}

#[test]
fn built_in_ports_are_written_in_library_source_order() {
    let src = tree_of(
        r#"<Parallel failure_count="1" name="p" success_count="2"><AlwaysSuccess/></Parallel>"#,
    );
    let t = view(&src);
    let w = write_tree(&t, &[], &ReferenceOptions::default()).unwrap();
    assert!(
        w.text.contains("<Parallel name=\"p\"\n              success_count=\"2\"\n              failure_count=\"1\">"),
        "{}",
        w.text
    );
}

#[test]
fn the_reference_load_refusals_are_diagnostics_on_load() {
    // §8 F10 rows of the second set: a Script attribute that is not its port.
    for (rel, attr) in [
        ("o-answers-2/in-o10-wrap/tree.xml", "x="),
        ("o-answers-2/in-o13-escape/attr-escapes.xml", "note="),
    ] {
        let src = read(rel);
        let kinds: Vec<_> = view(&src).diagnostics().iter().map(|d| d.kind).collect();
        assert!(
            kinds.contains(&DiagnosticKind::UnknownPort),
            "{rel}: {kinds:?}"
        );
        let offsets: Vec<_> = view(&src)
            .diagnostics()
            .iter()
            .filter(|d| d.kind == DiagnosticKind::UnknownPort)
            .map(|d| d.offset)
            .collect();
        assert!(
            offsets.contains(&src.find(attr).unwrap()),
            "{rel}: {offsets:?}"
        );
    }
    // ...and the leaf-with-child ones.
    for rel in [
        "o-answers-2/in-o11-collide/b/tree.xml",
        "o-answers-2/in-o13-escape/mixed-forms.xml",
        "o-answers-2/in-o13-escape/two-trees-blank.xml",
    ] {
        let kinds: Vec<_> = view(&read(rel))
            .diagnostics()
            .iter()
            .map(|d| d.kind)
            .collect();
        assert!(
            kinds.contains(&DiagnosticKind::LeafWithChildren),
            "{rel}: {kinds:?}"
        );
    }
}
