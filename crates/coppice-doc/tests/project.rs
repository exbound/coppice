//! Project loading (F3; spec §2.3, §2.3.1) and the checks that need every
//! document of a project (F10), against the self-authored fixtures in
//! `tests/fixtures/project/` and in-memory file sets.

use std::collections::HashMap;
use std::io;
use std::path::{Component, Path, PathBuf};

use coppice_doc::{DiagnosticKind, Document, Files, LoadError, Project, TreeDocument};

/// An in-memory file set. Identity is the lexically normalized path, after
/// following `links` (alias -> target), so a test can stand in for a
/// symbolic link.
#[derive(Default)]
struct Mem {
    files: HashMap<PathBuf, Vec<u8>>,
    links: HashMap<PathBuf, PathBuf>,
    opened: Vec<PathBuf>,
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

impl Mem {
    fn with(files: &[(&str, &str)]) -> Mem {
        let mut m = Mem::default();
        for (p, s) in files {
            m.files.insert(PathBuf::from(p), s.as_bytes().to_vec());
        }
        m
    }
}

impl Files for Mem {
    fn open(&mut self, path: &Path) -> io::Result<(PathBuf, Vec<u8>)> {
        self.opened.push(path.to_owned());
        let n = normalize(path);
        let n = self.links.get(&n).cloned().unwrap_or(n);
        match self.files.get(&n) {
            Some(b) => Ok((n, b.clone())),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "no such file")),
        }
    }
}

fn load(m: &mut Mem, start: &str) -> Project {
    Project::load_from(Path::new(start), m).unwrap()
}

fn paths(p: &Project) -> Vec<String> {
    p.documents()
        .iter()
        .map(|d| d.path.display().to_string())
        .collect()
}

/// (document index, kind, offset) of every diagnostic, in reported order.
fn diags(p: &Project) -> Vec<(usize, DiagnosticKind, usize)> {
    p.diagnostics()
        .iter()
        .map(|d| (d.document, d.diagnostic.kind, d.diagnostic.offset))
        .collect()
}

fn at(src: &str, needle: &str) -> usize {
    assert_eq!(src.matches(needle).count(), 1, "{needle} must occur once");
    src.find(needle).unwrap()
}

fn root(body: &str) -> String {
    format!("<root BTCPP_format=\"4\">{body}</root>")
}

fn tree(id: &str) -> String {
    root(&format!(
        "<BehaviorTree ID=\"{id}\"><AlwaysSuccess/></BehaviorTree>"
    ))
}

#[test]
fn fixture_project_loads_from_disk_with_nested_and_parent_includes() {
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/project/app/main.btproj");
    let p = Project::load(&start).unwrap();
    let canon: Vec<_> = p
        .documents()
        .iter()
        .map(|d| d.canonical_path.clone())
        .collect();
    let fixtures = std::fs::canonicalize(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/project"),
    )
    .unwrap();
    assert_eq!(
        canon,
        [
            fixtures.join("app/main.btproj"),
            fixtures.join("app/trees/a.xml"),
            // Relative to a.xml's directory, not the project's.
            fixtures.join("app/trees/sub/b.xml"),
            fixtures.join("shared/common.xml"),
        ]
    );
    let ids: Vec<_> = p.trees().iter().map(|t| (t.id, t.document)).collect();
    assert_eq!(ids, [("A", 1), ("B", 2), ("Common", 3)]);
    assert_eq!(p.main_tree(), Some("A"));
    assert_eq!(p.models().len(), 1);

    // The one problem: a port the project file's model does not declare,
    // on a node in an included file.
    let a = String::from_utf8(std::fs::read(&canon[1]).unwrap()).unwrap();
    assert_eq!(
        diags(&p),
        [(1, DiagnosticKind::UnknownPort, at(&a, "whom"))]
    );
}

#[test]
fn includes_resolve_relative_to_the_including_document_as_reached() {
    let mut m = Mem::with(&[
        (
            "/p/main.btproj",
            &root(r#"<include path="t/a.xml"/><include path="../x/c.xml"/>"#),
        ),
        ("/p/t/a.xml", &root(r#"<include path="./s/b.xml"/>"#)),
        ("/p/t/s/b.xml", &tree("B")),
        ("/x/c.xml", &tree("C")),
    ]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(
        paths(&p),
        [
            "/p/main.btproj",
            "/p/t/a.xml",
            "/p/t/./s/b.xml",
            "/p/../x/c.xml"
        ]
    );
    assert_eq!(diags(&p), []);
}

#[test]
fn a_nested_base_is_the_path_as_reached_not_its_canonical_form() {
    // /p/link/a.xml is a symbolic link to /q/real/a.xml. Its include
    // resolves against the directory it was reached by, /p/link, so
    // ../b.xml is /p/b.xml (as the library joins it), not /q/b.xml.
    let mut m = Mem::with(&[
        ("/p/main.btproj", &root(r#"<include path="link/a.xml"/>"#)),
        ("/q/real/a.xml", &root(r#"<include path="../b.xml"/>"#)),
        ("/p/b.xml", &tree("FromP")),
        ("/q/b.xml", &tree("FromQ")),
    ]);
    m.links.insert(
        PathBuf::from("/p/link/a.xml"),
        PathBuf::from("/q/real/a.xml"),
    );
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(p.documents()[1].canonical_path, Path::new("/q/real/a.xml"));
    let ids: Vec<_> = p.trees().iter().map(|t| t.id).collect();
    assert_eq!(ids, ["FromP"]);
}

#[test]
fn an_absolute_include_is_used_as_given() {
    let mut m = Mem::with(&[
        (
            "/p/main.btproj",
            &root(r#"<include path="/elsewhere/t.xml"/>"#),
        ),
        ("/elsewhere/t.xml", &tree("T")),
    ]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(paths(&p), ["/p/main.btproj", "/elsewhere/t.xml"]);
}

#[test]
fn ros_pkg_with_a_relative_path_is_reported_and_with_an_absolute_one_ignored() {
    let main = root(concat!(
        r#"<include ros_pkg="pkg" path="trees/t.xml"/>"#,
        r#"<include ros_pkg="pkg" path="/abs/t.xml"/>"#,
    ));
    let mut m = Mem::with(&[
        ("/p/main.btproj", &main),
        ("/p/trees/t.xml", &tree("NotLoaded")),
        ("/abs/t.xml", &tree("Abs")),
    ]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(paths(&p), ["/p/main.btproj", "/abs/t.xml"]);
    assert_eq!(
        diags(&p),
        [(
            0,
            DiagnosticKind::RosPackageInclude,
            at(&main, r#"<include ros_pkg="pkg" path="trees"#)
        )]
    );
    // Nothing tried to open the ROS-relative file.
    assert!(!m.opened.iter().any(|o| o.ends_with("trees/t.xml")));
}

#[test]
fn an_include_without_a_path_or_with_an_empty_one_is_reported() {
    let main = root(r#"<include/><include path=""/><include ros_pkg="p"/>"#);
    let mut m = Mem::with(&[("/p/main.btproj", &main)]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(
        diags(&p),
        [
            (
                0,
                DiagnosticKind::IncompleteInclude,
                at(&main, "<include/>")
            ),
            (
                0,
                DiagnosticKind::IncompleteInclude,
                at(&main, r#"<include path="""#)
            ),
            (
                0,
                DiagnosticKind::IncompleteInclude,
                at(&main, r#"<include ros_pkg"#)
            ),
        ]
    );
    assert_eq!(p.documents().len(), 1);
}

#[test]
fn unreadable_and_malformed_includes_are_reported_and_skipped() {
    let main = root(concat!(
        r#"<include path="missing.xml"/>"#,
        r#"<include path="broken.xml"/>"#,
        r#"<include path="other.xml"/>"#,
        r#"<include path="good.xml"/>"#,
    ));
    let mut m = Mem::with(&[
        ("/p/main.btproj", &main),
        ("/p/broken.xml", "<root><unclosed></root>"),
        ("/p/other.xml", "<notroot/>"),
        ("/p/good.xml", &tree("Good")),
    ]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(paths(&p), ["/p/main.btproj", "/p/good.xml"]);
    assert_eq!(
        diags(&p),
        [
            (
                0,
                DiagnosticKind::UnreadableInclude,
                at(&main, "<include path=\"missing")
            ),
            (
                0,
                DiagnosticKind::MalformedInclude,
                at(&main, "<include path=\"broken")
            ),
            (
                0,
                DiagnosticKind::MalformedInclude,
                at(&main, "<include path=\"other")
            ),
        ]
    );
}

#[test]
fn a_cycle_is_reported_at_the_include_that_closes_it() {
    let a = root(r#"<include path="b.xml"/><BehaviorTree ID="A"><AlwaysSuccess/></BehaviorTree>"#);
    let b = root(r#"<include path="a.xml"/><include path="b.xml"/>"#);
    let mut m = Mem::with(&[("/p/a.xml", &a), ("/p/b.xml", &b)]);
    let p = load(&mut m, "/p/a.xml");
    assert_eq!(paths(&p), ["/p/a.xml", "/p/b.xml"]);
    assert_eq!(
        diags(&p),
        [
            (
                1,
                DiagnosticKind::IncludeCycle,
                at(&b, r#"<include path="a.xml"/>"#)
            ),
            (
                1,
                DiagnosticKind::IncludeCycle,
                at(&b, r#"<include path="b.xml"/>"#)
            ),
        ]
    );
}

#[test]
fn a_file_reached_twice_without_a_cycle_loads_once_unreported() {
    // A diamond: main includes l and r; both include shared.
    let mut m = Mem::with(&[
        (
            "/p/main.btproj",
            &root(r#"<include path="l/l.xml"/><include path="r/r.xml"/>"#),
        ),
        ("/p/l/l.xml", &root(r#"<include path="../shared.xml"/>"#)),
        ("/p/r/r.xml", &root(r#"<include path="../shared.xml"/>"#)),
        ("/p/shared.xml", &tree("Shared")),
    ]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(
        paths(&p),
        [
            "/p/main.btproj",
            "/p/l/l.xml",
            "/p/l/../shared.xml",
            "/p/r/r.xml"
        ]
    );
    assert_eq!(diags(&p), []);
}

#[test]
fn an_alias_of_a_loaded_file_is_the_same_file() {
    let mut m = Mem::with(&[
        (
            "/p/main.btproj",
            &root(r#"<include path="t.xml"/><include path="alias.xml"/>"#),
        ),
        ("/p/t.xml", &tree("T")),
    ]);
    m.links
        .insert(PathBuf::from("/p/alias.xml"), PathBuf::from("/p/t.xml"));
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(paths(&p), ["/p/main.btproj", "/p/t.xml"]);
    assert_eq!(diags(&p), []);
}

#[test]
fn a_documents_own_trees_replace_its_includes_trees() {
    let main =
        root(r#"<include path="t.xml"/><BehaviorTree ID="T"><AlwaysFailure/></BehaviorTree>"#);
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &tree("T"))]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(p.tree("T").unwrap().document, 0);
    assert_eq!(
        diags(&p),
        [(0, DiagnosticKind::DuplicateTree, at(&main, "<BehaviorTree"))]
    );
}

#[test]
fn a_later_sibling_include_replaces_an_earlier_ones_tree() {
    let mut m = Mem::with(&[
        (
            "/p/main.btproj",
            &root(r#"<include path="x.xml"/><include path="y.xml"/>"#),
        ),
        ("/p/x.xml", &tree("T")),
        ("/p/y.xml", &tree("T")),
    ]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(p.tree("T").unwrap().document, 2);
    assert_eq!(
        diags(&p),
        [(
            2,
            DiagnosticKind::DuplicateTree,
            tree("T").find("<BehaviorTree").unwrap()
        )]
    );
}

#[test]
fn a_repeated_tree_id_within_one_document_keeps_the_later_tree() {
    let src = root(concat!(
        r#"<BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree>"#,
        r#"<BehaviorTree ID="T"><AlwaysFailure/></BehaviorTree>"#,
    ));
    let mut m = Mem::with(&[("/p/t.xml", &src)]);
    let p = load(&mut m, "/p/t.xml");
    let second = src.rfind("<BehaviorTree").unwrap();
    assert_eq!(p.tree("T").unwrap().element.span.start, second);
    assert_eq!(diags(&p), [(0, DiagnosticKind::DuplicateTree, second)]);
}

#[test]
fn only_the_starting_documents_main_tree_is_checked() {
    let main =
        r#"<root BTCPP_format="4" main_tree_to_execute="Nope"><include path="t.xml"/></root>"#;
    let inc = r#"<root BTCPP_format="4" main_tree_to_execute="AlsoNope"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#;
    let mut m = Mem::with(&[("/p/main.btproj", main), ("/p/t.xml", inc)]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(p.main_tree(), Some("Nope"));
    assert_eq!(
        diags(&p),
        [(
            0,
            DiagnosticKind::UnknownMainTree,
            at(main, "main_tree_to_execute")
        )]
    );

    let ok = r#"<root BTCPP_format="4" main_tree_to_execute="T"><include path="t.xml"/></root>"#;
    let mut m = Mem::with(&[("/p/main.btproj", ok), ("/p/t.xml", &tree("T"))]);
    assert_eq!(diags(&load(&mut m, "/p/main.btproj")), []);

    // A valid starting document; an included one names a missing tree.
    let inc = r#"<root project_name="x" BTCPP_format="4" main_tree_to_execute="Nope"><BehaviorTree ID="T"><AlwaysSuccess/></BehaviorTree></root>"#;
    let mut m = Mem::with(&[("/p/main.btproj", ok), ("/p/t.xml", inc)]);
    assert_eq!(diags(&load(&mut m, "/p/main.btproj")), []);
}

#[test]
fn a_subtree_naming_no_tree_of_the_project_is_reported() {
    let main = root(concat!(
        r#"<include path="t.xml"/>"#,
        r#"<BehaviorTree ID="Main"><Sequence>"#,
        r#"<SubTree ID="T"/><SubTree ID="Gone"/><SubTreePlus ID="AlsoGone"/>"#,
        r#"</Sequence></BehaviorTree>"#,
    ));
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &tree("T"))]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(
        diags(&p),
        [
            (
                0,
                DiagnosticKind::UnknownSubTree,
                at(&main, r#"<SubTree ID="Gone""#)
            ),
            (0, DiagnosticKind::UnknownSubTree, at(&main, "<SubTreePlus")),
        ]
    );
}

#[test]
fn a_project_files_models_apply_to_trees_in_included_files() {
    let main = root(concat!(
        r#"<include path="t.xml"/>"#,
        r#"<TreeNodesModel><Action ID="Act"><input_port name="p"/></Action></TreeNodesModel>"#,
    ));
    let t = root(concat!(
        r#"<BehaviorTree ID="T"><Sequence>"#,
        r#"<Act p="1" q="2"/><Act><AlwaysSuccess/></Act>"#,
        r#"</Sequence></BehaviorTree>"#,
    ));
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &t)]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(
        diags(&p),
        [
            (1, DiagnosticKind::UnknownPort, at(&t, "q=")),
            (1, DiagnosticKind::LeafWithChildren, at(&t, "<Act>")),
        ]
    );
    // On its own, the included file knows no models, so reports nothing.
    let alone = TreeDocument::new(Document::parse(t.as_bytes()).unwrap()).unwrap();
    assert_eq!(alone.diagnostics(), []);
}

#[test]
fn a_model_declared_again_in_a_later_document_is_reported_and_the_first_used() {
    let main = root(concat!(
        r#"<include path="t.xml"/>"#,
        r#"<TreeNodesModel><Action ID="M"><input_port name="first"/></Action></TreeNodesModel>"#,
    ));
    let t = root(
        r#"<TreeNodesModel><Condition ID="M"><input_port name="second"/></Condition></TreeNodesModel>"#,
    );
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &t)]);
    let p = load(&mut m, "/p/main.btproj");
    let models = p.models();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].ports[0].name, "first");
    assert_eq!(models[0].offset, at(&main, "<Action"));
    assert_eq!(
        diags(&p),
        [(1, DiagnosticKind::DuplicateModel, at(&t, "<Condition"))]
    );
}

#[test]
fn diagnostics_are_sorted_by_document_then_offset() {
    let main = root(concat!(
        r#"<include path="t.xml"/>"#,
        r#"<BehaviorTree ID="M"><SubTree ID="Gone"/></BehaviorTree>"#,
        r#"<include path="missing.xml"/>"#,
    ));
    let t = r#"<root><BehaviorTree ID="T"><AlwaysSuccess><AlwaysSuccess/></AlwaysSuccess></BehaviorTree></root>"#;
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", t)]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(
        diags(&p),
        [
            (0, DiagnosticKind::UnknownSubTree, at(&main, "<SubTree")),
            (
                0,
                DiagnosticKind::UnreadableInclude,
                at(&main, "<include path=\"missing")
            ),
            (1, DiagnosticKind::MissingFormat, 0),
            (
                1,
                DiagnosticKind::LeafWithChildren,
                at(t, "<AlwaysSuccess>")
            ),
        ]
    );
}

#[test]
fn a_starting_document_that_cannot_load_is_an_error() {
    let mut m = Mem::with(&[("/p/bad.xml", "<root>"), ("/p/other.xml", "<x/>")]);
    assert!(matches!(
        Project::load_from(Path::new("/p/none.xml"), &mut m),
        Err(LoadError::Read { .. })
    ));
    assert!(matches!(
        Project::load_from(Path::new("/p/bad.xml"), &mut m),
        Err(LoadError::Parse { .. })
    ));
    assert!(matches!(
        Project::load_from(Path::new("/p/other.xml"), &mut m),
        Err(LoadError::NotATree { .. })
    ));
}

#[test]
fn a_relative_starting_path_is_made_absolute() {
    let cwd = std::env::current_dir().unwrap();
    let mut m = Mem::default();
    m.files
        .insert(normalize(&cwd.join("main.btproj")), tree("T").into_bytes());
    let p = load(&mut m, "main.btproj");
    assert_eq!(p.documents()[0].path, cwd.join("main.btproj"));
}

#[test]
fn a_long_chain_of_includes_does_not_exhaust_the_stack() {
    const N: usize = 20_000;
    let mut m = Mem::default();
    for i in 0..N {
        let body = if i + 1 < N {
            format!(r#"<include path="{}.xml"/>"#, i + 1)
        } else {
            String::new()
        };
        m.files.insert(
            PathBuf::from(format!("/c/{i}.xml")),
            root(&body).into_bytes(),
        );
    }
    let p = load(&mut m, "/c/0.xml");
    assert_eq!(p.documents().len(), N);
}

#[test]
fn a_model_repeated_within_a_later_document_is_reported_once_per_declaration() {
    let main = root(r#"<include path="t.xml"/><TreeNodesModel><Action ID="M"/></TreeNodesModel>"#);
    let t = root(
        r#"<TreeNodesModel><Action ID="M" editable="true"/><Condition ID="M"/></TreeNodesModel>"#,
    );
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &t)]);
    let p = load(&mut m, "/p/main.btproj");
    // The first M in t.xml repeats the project file's; the second repeats
    // the first within t.xml. One diagnostic each.
    assert_eq!(
        diags(&p),
        [
            (1, DiagnosticKind::DuplicateModel, at(&t, "<Action")),
            (1, DiagnosticKind::DuplicateModel, at(&t, "<Condition")),
        ]
    );
}

#[test]
fn diagnostics_scale_linearly_with_documents_models_and_nodes() {
    // Each file declares a model, uses the previous file's model and
    // includes the next one. Rebuilding the model lookup per document
    // takes minutes here; the shared lookup, well under a second. (Not
    // every quadratic pass is caught: one over the 20,000 models alone
    // still finishes inside the bound.)
    const N: usize = 20_000;
    let mut m = Mem::default();
    for i in 0..N {
        let mut body = format!(
            r#"<TreeNodesModel><Action ID="M{i}"><input_port name="p"/></Action></TreeNodesModel><BehaviorTree ID="T{i}"><M{prev} p="1"/></BehaviorTree>"#,
            prev = i.saturating_sub(1)
        );
        if i + 1 < N {
            body.push_str(&format!(r#"<include path="{}.xml"/>"#, i + 1));
        }
        m.files.insert(
            PathBuf::from(format!("/c/{i}.xml")),
            root(&body).into_bytes(),
        );
    }
    let p = load(&mut m, "/c/0.xml");
    let started = std::time::Instant::now();
    let d = p.diagnostics();
    let took = started.elapsed();
    assert_eq!(d, []);
    assert_eq!(p.models().len(), N);
    assert!(took.as_secs() < 20, "diagnostics took {took:?}");
}

#[test]
fn tree_lookups_do_not_rebuild_the_registry() {
    // 20,000 trees, each looked up by name: a lookup that rebuilt the
    // registry each time would take minutes in a debug build.
    const N: usize = 20_000;
    let body: String = (0..N)
        .map(|i| format!(r#"<BehaviorTree ID="T{i}"><AlwaysSuccess/></BehaviorTree>"#))
        .collect();
    let mut m = Mem::with(&[("/p/t.xml", &root(&body))]);
    let p = load(&mut m, "/p/t.xml");
    let started = std::time::Instant::now();
    for i in 0..N {
        let t = p.tree(&format!("T{i}")).unwrap();
        assert_eq!(t.id, format!("T{i}"));
    }
    assert!(p.tree("absent").is_none());
    assert_eq!(p.trees().len(), N);
    let took = started.elapsed();
    assert!(took.as_secs() < 5, "lookups took {took:?}");
}

#[test]
fn an_identical_redeclaration_in_a_later_document_is_not_reported() {
    let model = r#"<Action ID="M" editable="true"><input_port name="a" type="int">the a</input_port></Action>"#;
    let main = root(&format!(
        r#"<include path="t.xml"/><TreeNodesModel>{model}</TreeNodesModel>"#
    ));
    // Same declaration, laid out differently.
    let t = root(
        r#"<TreeNodesModel>
  <Action ID="M"
          editable="true">
    <input_port name="a" type="int">the a</input_port>
  </Action>
</TreeNodesModel>"#,
    );
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &t)]);
    let p = load(&mut m, "/p/main.btproj");
    assert_eq!(diags(&p), []);
    assert_eq!(p.models()[0].offset, at(&main, "<Action"));
}

#[test]
fn any_difference_in_a_redeclaration_is_reported() {
    let main = root(concat!(
        r#"<include path="t.xml"/><TreeNodesModel>"#,
        r#"<Action ID="M" editable="true"><input_port name="a" type="int">d</input_port></Action>"#,
        r#"</TreeNodesModel>"#,
    ));
    for other in [
        r#"<Condition ID="M" editable="true"><input_port name="a" type="int">d</input_port></Condition>"#,
        r#"<Action ID="M"><input_port name="a" type="int">d</input_port></Action>"#,
        r#"<Action ID="M" editable="true"><output_port name="a" type="int">d</output_port></Action>"#,
        r#"<Action ID="M" editable="true"><input_port name="b" type="int">d</input_port></Action>"#,
        r#"<Action ID="M" editable="true"><input_port name="a" type="bool">d</input_port></Action>"#,
        r#"<Action ID="M" editable="true"><input_port name="a" type="int" default="1">d</input_port></Action>"#,
        r#"<Action ID="M" editable="true"><input_port name="a" type="int">e</input_port></Action>"#,
        r#"<Action ID="M" editable="true"><input_port name="a" type="int">d</input_port><input_port name="c"/></Action>"#,
        r#"<Action ID="M" editable="true"><input_port name="a" type="int">d</input_port><MetadataFields><Metadata k="v"/></MetadataFields></Action>"#,
        r#"<Action ID="M" editable="true"><description>x</description><input_port name="a" type="int">d</input_port></Action>"#,
    ] {
        let t = root(&format!("<TreeNodesModel>{other}</TreeNodesModel>"));
        let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &t)]);
        let p = load(&mut m, "/p/main.btproj");
        let kinds: Vec<_> = diags(&p).into_iter().map(|(d, k, _)| (d, k)).collect();
        assert!(
            kinds.contains(&(1, DiagnosticKind::DuplicateModel)),
            "{other}: {kinds:?}"
        );
    }
}

fn duplicates_between(first: &str, later: &str) -> Vec<DiagnosticKind> {
    let main = root(&format!(
        r#"<include path="t.xml"/><TreeNodesModel>{first}</TreeNodesModel>"#
    ));
    let t = root(&format!("<TreeNodesModel>{later}</TreeNodesModel>"));
    let mut m = Mem::with(&[("/p/main.btproj", &main), ("/p/t.xml", &t)]);
    let p = load(&mut m, "/p/main.btproj");
    diags(&p)
        .into_iter()
        .filter(|(d, k, _)| *d == 1 && *k == DiagnosticKind::DuplicateModel)
        .map(|(_, k, _)| k)
        .collect()
}

#[test]
fn the_written_copy_of_a_model_with_a_description_is_not_reported() {
    // The reference writer drops the <description> (spec §2.4).
    assert_eq!(
        duplicates_between(
            r#"<Action ID="M"><description>moves the arm</description><input_port name="a"/></Action>"#,
            r#"<Action ID="M"><input_port name="a"/></Action>"#,
        ),
        []
    );
}

#[test]
fn content_recorded_only_by_name_is_always_reported() {
    for (first, later) in [
        // Description texts differ, and the view keeps neither.
        (
            r#"<Action ID="M"><description>moves the arm</description></Action>"#,
            r#"<Action ID="M"><description>deletes the disk</description></Action>"#,
        ),
        // A description only in the later one: not a written copy.
        (
            r#"<Action ID="M"/>"#,
            r#"<Action ID="M"><description>x</description></Action>"#,
        ),
        // Unrecognized attribute values differ.
        (r#"<Action ID="M" foo="1"/>"#, r#"<Action ID="M" foo="2"/>"#),
        // Even when they are the same.
        (r#"<Action ID="M" foo="1"/>"#, r#"<Action ID="M" foo="1"/>"#),
        (
            r#"<Action ID="M"><input_port name="a" foo="1"/></Action>"#,
            r#"<Action ID="M"><input_port name="a" foo="2"/></Action>"#,
        ),
        (
            r#"<Action ID="M"><MetadataFields><Metadata k="1"/></MetadataFields></Action>"#,
            r#"<Action ID="M"><MetadataFields><Metadata k="1"/></MetadataFields></Action>"#,
        ),
    ] {
        assert_eq!(
            duplicates_between(first, later),
            [DiagnosticKind::DuplicateModel],
            "{first} / {later}"
        );
    }
}
